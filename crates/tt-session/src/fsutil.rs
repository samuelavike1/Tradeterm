//! File helpers for persisted state: atomic writes, corrupt-file backups and a
//! debounced background writer ([`DebouncedFile`]).

use serde::Serialize;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

/// Atomic write: a temp file in the same directory, flushed to disk, then
/// renamed over `path`. Readers see either the old or the new content, never
/// a partial file. Creates missing parent directories.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".tmp-{}", std::process::id()));
    let tmp = path.with_file_name(name);
    let result = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Moves an unparseable file aside as `<name>.corrupt-<unix-ms>` so it is never
/// overwritten. Returns the backup path.
pub fn backup_corrupt(path: &Path) -> std::io::Result<PathBuf> {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".corrupt-{ms}"));
    let backup = path.with_file_name(name);
    std::fs::rename(path, &backup)?;
    Ok(backup)
}

/// What [`DebouncedFile::poll`] wants the caller to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Poll {
    /// Nothing changed since the last save.
    Idle,
    /// Changes are pending; check again after this long.
    Wait(Duration),
    /// Changes have been stable for the debounce time: build the value and
    /// call [`DebouncedFile::save`].
    Due,
}

enum Job<T> {
    Write(T),
    /// Acknowledged once every earlier write has finished.
    Sync(mpsc::Sender<()>),
}

/// A JSON file written in the background, debounced on a generation counter.
///
/// The caller reports changes with [`Self::mark_changed`] (cheap: no snapshot,
/// no comparison) and builds the value only when [`Self::poll`] says
/// [`Poll::Due`]. Serialization, change detection (a hash of the bytes) and the
/// atomic write run on a dedicated writer thread, in order; identical content
/// is never rewritten.
pub struct DebouncedFile<T: Serialize + Send + 'static> {
    path: Option<PathBuf>,
    debounce: Duration,
    generation: u64,
    saved_generation: u64,
    last_change: Instant,
    /// A check is scheduled by the caller (from `mark_changed`'s return value).
    armed: bool,
    tx: Option<mpsc::Sender<Job<T>>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl<T: Serialize + Send + 'static> DebouncedFile<T> {
    /// `path = None` never writes. `initial` is what the file already holds
    /// (e.g. the loaded value): writing the same content again is skipped.
    pub fn new(path: Option<PathBuf>, debounce: Duration, initial: Option<&T>) -> Self {
        let mut me = Self {
            path: None,
            debounce,
            generation: 0,
            saved_generation: 0,
            last_change: Instant::now(),
            armed: false,
            tx: None,
            worker: None,
        };
        let Some(path) = path else { return me };
        let last_hash = initial.and_then(|v| serde_json::to_vec_pretty(v).ok().map(|b| hash(&b)));
        let (tx, rx) = mpsc::channel::<Job<T>>();
        let p = path.clone();
        let worker = std::thread::Builder::new()
            .name("settings-writer".into())
            .spawn(move || writer_loop(&p, rx, last_hash));
        match worker {
            Ok(w) => {
                me.path = Some(path);
                me.tx = Some(tx);
                me.worker = Some(w);
            }
            Err(e) => warn!(path = %path.display(), "could not start writer thread: {e}"),
        }
        me
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Records a change at `now`. Returns `Some(wait)` when the caller must
    /// schedule a [`Self::poll`] after `wait` (no check is pending yet).
    pub fn mark_changed(&mut self, now: Instant) -> Option<Duration> {
        self.generation += 1;
        self.last_change = now;
        (!std::mem::replace(&mut self.armed, true)).then_some(self.debounce)
    }

    /// Unsaved changes exist.
    pub fn is_dirty(&self) -> bool {
        self.generation != self.saved_generation
    }

    pub fn poll(&mut self, now: Instant) -> Poll {
        if !self.is_dirty() {
            self.armed = false;
            return Poll::Idle;
        }
        let elapsed = now.saturating_duration_since(self.last_change);
        if elapsed >= self.debounce {
            self.armed = false;
            Poll::Due
        } else {
            Poll::Wait(self.debounce - elapsed)
        }
    }

    /// Queues `value` for writing (serialized and written on the writer
    /// thread) and marks everything up to now as saved.
    pub fn save(&mut self, value: T) {
        self.saved_generation = self.generation;
        if let Some(tx) = &self.tx {
            let _ = tx.send(Job::Write(value));
        }
    }

    /// Blocks until every queued write has finished.
    pub fn wait_idle(&self) {
        if let Some(tx) = &self.tx {
            let (ack_tx, ack_rx) = mpsc::channel();
            if tx.send(Job::Sync(ack_tx)).is_ok() {
                let _ = ack_rx.recv();
            }
        }
    }

    /// Saves `value` if anything changed and waits until it is on disk
    /// (used on exit).
    pub fn flush_with(&mut self, value: impl FnOnce() -> T) {
        if self.is_dirty() {
            self.save(value());
        }
        self.wait_idle();
    }
}

impl<T: Serialize + Send + 'static> Drop for DebouncedFile<T> {
    fn drop(&mut self) {
        // Closing the channel lets the writer finish queued jobs and exit.
        self.tx = None;
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

fn hash(bytes: &[u8]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    h.finish()
}

fn writer_loop<T: Serialize>(path: &Path, rx: mpsc::Receiver<Job<T>>, mut last_hash: Option<u64>) {
    let mut write = |value: T| {
        let bytes = match serde_json::to_vec_pretty(&value) {
            Ok(b) => b,
            Err(e) => {
                warn!(path = %path.display(), "could not serialize: {e}");
                return;
            }
        };
        let h = hash(&bytes);
        if last_hash == Some(h) {
            return;
        }
        // Recorded even on failure so a broken disk doesn't cause a retry storm.
        last_hash = Some(h);
        match atomic_write(path, &bytes) {
            Ok(()) => debug!(path = %path.display(), "saved"),
            Err(e) => warn!(path = %path.display(), "could not save: {e}"),
        }
    };
    while let Ok(job) = rx.recv() {
        let mut pending = None;
        let mut acks = Vec::new();
        let mut next = Some(job);
        // Coalesce a burst: only the newest value is written, but acks wait for it.
        while let Some(job) = next {
            match job {
                Job::Write(v) => pending = Some(v),
                Job::Sync(ack) => {
                    if let Some(v) = pending.take() {
                        write(v);
                    }
                    acks.push(ack);
                }
            }
            next = rx.try_recv().ok();
        }
        if let Some(v) = pending.take() {
            write(v);
        }
        for ack in acks {
            let _ = ack.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_and_leaves_no_temp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a").join("f.json");
        atomic_write(&path, b"one").unwrap();
        atomic_write(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["f.json"]);
    }

    #[test]
    fn failed_atomic_write_keeps_target_and_cleans_temp() {
        let dir = tempfile::tempdir().unwrap();
        // The target is a non-empty directory, so the rename must fail.
        let path = dir.path().join("f.json");
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("keep"), b"x").unwrap();
        assert!(atomic_write(&path, b"new").is_err());
        assert_eq!(std::fs::read(path.join("keep")).unwrap(), b"x");
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["f.json"]);
    }

    #[test]
    fn debounce_on_generation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.json");
        let d = Duration::from_millis(100);
        let mut f = DebouncedFile::<u32>::new(Some(path.clone()), d, Some(&1));
        let t0 = Instant::now();
        assert_eq!(f.poll(t0), Poll::Idle);
        assert_eq!(f.mark_changed(t0), Some(d));
        // Already armed: no second timer.
        assert_eq!(f.mark_changed(t0 + Duration::from_millis(50)), None);
        assert_eq!(
            f.poll(t0 + Duration::from_millis(100)),
            Poll::Wait(Duration::from_millis(50))
        );
        assert_eq!(f.poll(t0 + Duration::from_millis(150)), Poll::Due);
        // Same content as the initial value: not written.
        f.save(1);
        f.wait_idle();
        assert!(!path.exists());
        assert_eq!(f.poll(t0 + Duration::from_secs(1)), Poll::Idle);
        // Re-armed after the check fired.
        assert_eq!(f.mark_changed(t0 + Duration::from_secs(1)), Some(d));
        f.flush_with(|| 2);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "2");
    }
}
