//! Asset names and logos (TradingView-style ticker logos), cached on disk.
//!
//! Layout, in the app's data directory (the settings file's directory, so
//! `$TRADETERM_SETTINGS` relocates it for tests):
//! - `assets.json`: Binance's asset list (code → full name, logo URL) with
//!   the time it was fetched; refreshed in the background at most once per
//!   [`REFRESH_EVERY`];
//! - `logos/<CODE>.png`: logo images, downloaded lazily the first time the
//!   UI asks for one ([`AssetCache::logo`]), written atomically.
//!
//! The UI queries an in-memory map (a read lock and a hash lookup, no IO)
//! and is woken when the list loads or a logo arrives; [`AssetCache::generation`]
//! tells it something changed. Offline or with the endpoint gone, the cached
//! list and logos keep working and missing logos fall back to letter badges.

use crate::tab::Wake;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::runtime::Handle;
use tracing::{debug, info, warn};
use tt_brokers::BinanceSource;

pub const ASSETS_FILE: &str = "assets.json";
pub const LOGOS_DIR: &str = "logos";
/// The asset list is re-downloaded at most this often.
pub const REFRESH_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
/// After a failed refresh, try again this much later.
const RETRY_AFTER: Duration = Duration::from_secs(60 * 60);
/// Logo downloads in flight at once (a search can ask for 100).
const LOGO_DOWNLOADS: usize = 4;

/// `assets.json`.
#[derive(Debug, Default, Serialize, Deserialize)]
struct CacheFile {
    /// When the list was downloaded (Unix ms).
    #[serde(default)]
    fetched_at_ms: Option<i64>,
    #[serde(default)]
    assets: Vec<CachedAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedAsset {
    code: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    logo_url: Option<String>,
}

/// Whether a list fetched at `fetched_at_ms` should be downloaded again.
/// A timestamp in the future (clock changed) counts as stale.
pub fn is_stale(fetched_at_ms: Option<i64>, now_ms: i64) -> bool {
    due_in(fetched_at_ms, now_ms).is_zero()
}

/// How long until the list fetched at `fetched_at_ms` is due for a refresh.
pub(crate) fn due_in(fetched_at_ms: Option<i64>, now_ms: i64) -> Duration {
    let Some(at) = fetched_at_ms else {
        return Duration::ZERO;
    };
    let age = now_ms - at;
    let every = REFRESH_EVERY.as_millis() as i64;
    if !(0..every).contains(&age) {
        return Duration::ZERO;
    }
    Duration::from_millis((every - age) as u64)
}

pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// The logo's file name for an asset code: `BTC.png`. Codes that aren't
/// plain ASCII (some assets have CJK codes) or that are reserved device
/// names on Windows are hex-encoded.
pub fn logo_file_name(code: &str) -> String {
    const RESERVED: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];
    let plain = !code.is_empty()
        && code
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    let upper = code.to_ascii_uppercase();
    let reserved = RESERVED.contains(&upper.as_str())
        || ((upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.len() == 4
            && upper.as_bytes()[3].is_ascii_digit());
    if plain && !reserved {
        format!("{code}.png")
    } else {
        let hex: String = code.bytes().map(|b| format!("{b:02x}")).collect();
        format!("x-{hex}.png")
    }
}

#[derive(Debug, Clone, Default)]
struct Entry {
    name: Option<Arc<str>>,
    logo_url: Option<Arc<str>>,
    /// The logo file, once it is on disk.
    logo: Option<Arc<Path>>,
}

struct Inner {
    /// Data directory; `None`: memory only, nothing downloaded.
    dir: Option<PathBuf>,
    entries: RwLock<HashMap<String, Entry>>,
    /// Codes whose logo is downloading, or failed this session (not retried
    /// until the next launch).
    requested: Mutex<HashSet<String>>,
    generation: AtomicU64,
    wake: Wake,
    net: Option<(Handle, Arc<BinanceSource>)>,
    downloads: tokio::sync::Semaphore,
}

/// Shared asset names and logos; cheap to clone.
#[derive(Clone)]
pub struct AssetCache(Arc<Inner>);

/// What was read from the data directory.
#[derive(Debug, Default)]
struct OnDisk {
    fetched_at_ms: Option<i64>,
    assets: Vec<CachedAsset>,
    /// File names in `logos/`.
    logos: HashSet<String>,
}

fn load_dir(dir: &Path) -> OnDisk {
    let file = match std::fs::read(dir.join(ASSETS_FILE)) {
        Ok(bytes) => serde_json::from_slice::<CacheFile>(&bytes).unwrap_or_else(|e| {
            warn!("ignoring unreadable {ASSETS_FILE}: {e}");
            CacheFile::default()
        }),
        Err(_) => CacheFile::default(),
    };
    let logos = std::fs::read_dir(dir.join(LOGOS_DIR))
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| n.ends_with(".png"))
                .collect()
        })
        .unwrap_or_default();
    OnDisk {
        fetched_at_ms: file.fetched_at_ms,
        assets: file.assets,
        logos,
    }
}

impl AssetCache {
    fn new(dir: Option<PathBuf>, wake: Wake, net: Option<(Handle, Arc<BinanceSource>)>) -> Self {
        Self(Arc::new(Inner {
            dir,
            entries: RwLock::new(HashMap::new()),
            requested: Mutex::new(HashSet::new()),
            generation: AtomicU64::new(0),
            wake,
            net,
            downloads: tokio::sync::Semaphore::new(LOGO_DOWNLOADS),
        }))
    }

    /// Empty, memory only, no network: every lookup misses (letter badges).
    pub fn disabled() -> Self {
        Self::new(None, Arc::new(|| {}), None)
    }

    /// The data directory for a settings file: its directory.
    pub fn dir_for(settings: &Path) -> Option<PathBuf> {
        settings.parent().map(Path::to_path_buf)
    }

    /// Reads the cache in `dir` synchronously, without network (tests and
    /// tools; the app uses [`Self::start`]).
    pub fn offline(dir: Option<PathBuf>) -> Self {
        let cache = Self::new(dir, Arc::new(|| {}), None);
        if let Some(dir) = &cache.0.dir {
            let disk = load_dir(dir);
            cache.install(disk.assets, &disk.logos);
        }
        cache
    }

    /// Loads the cache in `dir` on the runtime, then keeps the list fresh
    /// (downloads it when missing or older than [`REFRESH_EVERY`]). `wake`
    /// is called whenever names or logos change. Never blocks the caller.
    pub fn start(
        dir: Option<PathBuf>,
        source: Arc<BinanceSource>,
        handle: Handle,
        wake: Wake,
    ) -> Self {
        let cache = Self::new(dir, wake, Some((handle.clone(), source.clone())));
        let this = cache.clone();
        handle.spawn(async move { this.run(source).await });
        cache
    }

    async fn run(self, source: Arc<BinanceSource>) {
        let mut fetched_at = None;
        if let Some(dir) = self.0.dir.clone() {
            let disk = tokio::task::spawn_blocking(move || load_dir(&dir))
                .await
                .unwrap_or_default();
            fetched_at = disk.fetched_at_ms;
            if !disk.assets.is_empty() || !disk.logos.is_empty() {
                debug!(
                    assets = disk.assets.len(),
                    logos = disk.logos.len(),
                    "asset cache loaded"
                );
                self.install(disk.assets, &disk.logos);
                self.changed();
            }
        }
        let mut wait = due_in(fetched_at, now_ms());
        loop {
            tokio::time::sleep(wait).await;
            wait = match source.fetch_assets().await {
                Ok(list) if !list.is_empty() => {
                    info!(assets = list.len(), "asset list downloaded");
                    let assets: Vec<CachedAsset> = list
                        .into_iter()
                        .map(|a| CachedAsset {
                            code: a.code,
                            name: a.name,
                            logo_url: a.logo_url,
                        })
                        .collect();
                    let file = CacheFile {
                        fetched_at_ms: Some(now_ms()),
                        assets: assets.clone(),
                    };
                    if let Some(dir) = self.0.dir.clone() {
                        let saved = tokio::task::spawn_blocking(move || {
                            let json = serde_json::to_vec(&file).map_err(std::io::Error::other)?;
                            crate::fsutil::atomic_write(&dir.join(ASSETS_FILE), &json)
                        })
                        .await;
                        if let Ok(Err(e)) = saved {
                            warn!("could not save {ASSETS_FILE}: {e}");
                        }
                    }
                    self.install(assets, &HashSet::new());
                    self.changed();
                    REFRESH_EVERY
                }
                Ok(_) => {
                    warn!("asset list is empty; keeping the cached one");
                    RETRY_AFTER
                }
                Err(e) => {
                    warn!("asset list unavailable ({e:#}); keeping the cached one");
                    RETRY_AFTER
                }
            };
        }
    }

    /// Merges a list into the map. Logo files already known stay; `logos`
    /// (file names on disk) attach files to their codes.
    fn install(&self, assets: Vec<CachedAsset>, logos: &HashSet<String>) {
        let dir = self.0.dir.as_deref().map(|d| d.join(LOGOS_DIR));
        let path_of = |code: &str| {
            let name = logo_file_name(code);
            match &dir {
                Some(d) if logos.contains(&name) => Some(Arc::from(d.join(name))),
                _ => None,
            }
        };
        let mut map = self.0.entries.write().unwrap_or_else(|e| e.into_inner());
        for a in assets {
            let code = a.code.trim().to_uppercase();
            if code.is_empty() {
                continue;
            }
            let logo = path_of(&code);
            let e = map.entry(code).or_default();
            let name = a.name.trim();
            if !name.is_empty() {
                e.name = Some(name.into());
            }
            e.logo_url = a.logo_url.map(Into::into);
            if e.logo.is_none() {
                e.logo = logo;
            }
        }
        // Logos on disk for codes the list doesn't have (yet).
        if let Some(d) = &dir {
            for file in logos {
                if let Some(code) = file.strip_suffix(".png")
                    && logo_file_name(code) == *file
                {
                    let e = map.entry(code.to_uppercase()).or_default();
                    if e.logo.is_none() {
                        e.logo = Some(Arc::from(d.join(file)));
                    }
                }
            }
        }
    }

    fn changed(&self) {
        self.0.generation.fetch_add(1, Ordering::Release);
        (self.0.wake)();
    }

    /// Bumped whenever a name or logo changes (the UI re-reads on change).
    pub fn generation(&self) -> u64 {
        self.0.generation.load(Ordering::Acquire)
    }

    /// Full name of an asset ("Bitcoin"), if known.
    pub fn name(&self, code: &str) -> Option<Arc<str>> {
        let map = self.0.entries.read().unwrap_or_else(|e| e.into_inner());
        map.get(code)?.name.clone()
    }

    /// "Bitcoin / TetherUS" when the base asset's name is known (an unknown
    /// quote name falls back to its code).
    pub fn pair_name(&self, base: &str, quote: &str) -> Option<String> {
        let b = self.name(base)?;
        let q = self.name(quote);
        Some(format!("{b} / {}", q.as_deref().unwrap_or(quote)))
    }

    /// The asset's logo file if it is on disk. Otherwise the first call
    /// starts a background download (when the list has a logo URL) and the
    /// UI is woken when it lands; until then, and when there is no logo,
    /// `None` (draw the letter badge).
    pub fn logo(&self, code: &str) -> Option<Arc<Path>> {
        let url = {
            let map = self.0.entries.read().unwrap_or_else(|e| e.into_inner());
            let e = map.get(code)?;
            if let Some(p) = &e.logo {
                return Some(p.clone());
            }
            e.logo_url.clone()?
        };
        self.request_logo(code, url);
        None
    }

    fn request_logo(&self, code: &str, url: Arc<str>) {
        let (Some(dir), Some((handle, source))) = (&self.0.dir, &self.0.net) else {
            return;
        };
        {
            let mut req = self.0.requested.lock().unwrap_or_else(|e| e.into_inner());
            if !req.insert(code.to_owned()) {
                return;
            }
        }
        let path = dir.join(LOGOS_DIR).join(logo_file_name(code));
        let (this, source, code) = (self.clone(), source.clone(), code.to_owned());
        handle.spawn(async move {
            let Ok(_permit) = this.0.downloads.acquire().await else {
                return;
            };
            let bytes = match source.fetch_logo(&url).await {
                Ok(b) => b,
                Err(e) => {
                    debug!(%code, "no logo: {e:#}");
                    return;
                }
            };
            let p = path.clone();
            let written =
                tokio::task::spawn_blocking(move || crate::fsutil::atomic_write(&p, &bytes)).await;
            match written {
                Ok(Ok(())) => {
                    let mut map = this.0.entries.write().unwrap_or_else(|e| e.into_inner());
                    map.entry(code).or_default().logo = Some(Arc::from(path));
                    drop(map);
                    this.changed();
                }
                Ok(Err(e)) => warn!(%code, "could not save logo: {e}"),
                Err(e) => warn!(%code, "logo write task failed: {e}"),
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY_MS: i64 = 24 * 60 * 60 * 1000;

    #[test]
    fn freshness() {
        let now = 1_700_000_000_000;
        assert!(is_stale(None, now));
        assert!(!is_stale(Some(now), now));
        assert!(!is_stale(Some(now - DAY_MS + 1), now));
        assert!(is_stale(Some(now - DAY_MS), now));
        assert!(is_stale(Some(now - 10 * DAY_MS), now));
        // Clock went backwards: refresh rather than wait for days.
        assert!(is_stale(Some(now + 60_000), now));
        assert_eq!(
            due_in(Some(now - DAY_MS + 5_000), now),
            Duration::from_secs(5)
        );
    }

    #[test]
    fn logo_file_names_are_safe() {
        assert_eq!(logo_file_name("BTC"), "BTC.png");
        assert_eq!(logo_file_name("1000SATS"), "1000SATS.png");
        assert_eq!(logo_file_name("CON"), "x-434f4e.png");
        assert_eq!(logo_file_name("COM1"), "x-434f4d31.png");
        assert_eq!(logo_file_name("COMP"), "COMP.png");
        assert_eq!(logo_file_name("../X"), "x-2e2e2f58.png");
        assert!(logo_file_name("币安人生").starts_with("x-"));
    }

    fn write_cache(dir: &Path) {
        let file = r#"{"fetched_at_ms":1,"assets":[
            {"code":"BTC","name":"Bitcoin","logo_url":"https://x/btc.png"},
            {"code":"USDT","name":"TetherUS","logo_url":"https://x/usdt.png"},
            {"code":"NOLOGO","name":"No Logo"}
        ]}"#;
        std::fs::write(dir.join(ASSETS_FILE), file).unwrap();
        std::fs::create_dir_all(dir.join(LOGOS_DIR)).unwrap();
        std::fs::write(dir.join(LOGOS_DIR).join("BTC.png"), b"\x89PNG\r\n\x1a\n").unwrap();
        std::fs::write(dir.join(LOGOS_DIR).join("ORPHAN.png"), b"\x89PNG\r\n\x1a\n").unwrap();
    }

    #[test]
    fn cached_names_and_logos_load() {
        let tmp = tempfile::tempdir().unwrap();
        write_cache(tmp.path());
        let c = AssetCache::offline(Some(tmp.path().to_path_buf()));
        assert_eq!(c.name("BTC").as_deref(), Some("Bitcoin"));
        assert_eq!(
            c.pair_name("BTC", "USDT").as_deref(),
            Some("Bitcoin / TetherUS")
        );
        assert_eq!(c.pair_name("BTC", "XYZ").as_deref(), Some("Bitcoin / XYZ"));
        assert_eq!(c.pair_name("XYZ", "USDT"), None);
        let logo = c.logo("BTC").unwrap();
        assert_eq!(*logo, *tmp.path().join(LOGOS_DIR).join("BTC.png"));
        // A logo file without a list entry still shows.
        assert!(c.logo("ORPHAN").is_some());
        assert_eq!(c.name("ORPHAN"), None);
    }

    #[test]
    fn missing_logos_fall_back_to_none() {
        let tmp = tempfile::tempdir().unwrap();
        write_cache(tmp.path());
        let c = AssetCache::offline(Some(tmp.path().to_path_buf()));
        // Not downloaded yet (offline: never) / no logo at all / unknown.
        assert_eq!(c.logo("USDT"), None);
        assert_eq!(c.logo("NOLOGO"), None);
        assert_eq!(c.logo("NOPE"), None);
        assert_eq!(c.generation(), 0);
        let d = AssetCache::disabled();
        assert_eq!(d.logo("BTC"), None);
        assert_eq!(d.name("BTC"), None);
    }

    #[test]
    fn corrupt_or_missing_cache_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let c = AssetCache::offline(Some(tmp.path().to_path_buf()));
        assert_eq!(c.name("BTC"), None);
        std::fs::write(tmp.path().join(ASSETS_FILE), "{not json").unwrap();
        let c = AssetCache::offline(Some(tmp.path().to_path_buf()));
        assert_eq!(c.name("BTC"), None);
    }

    #[test]
    fn refresh_keeps_known_logos() {
        let tmp = tempfile::tempdir().unwrap();
        write_cache(tmp.path());
        let c = AssetCache::offline(Some(tmp.path().to_path_buf()));
        c.install(
            vec![CachedAsset {
                code: "btc".into(),
                name: "Bitcoin (new)".into(),
                logo_url: None,
            }],
            &HashSet::new(),
        );
        assert_eq!(c.name("BTC").as_deref(), Some("Bitcoin (new)"));
        assert!(c.logo("BTC").is_some());
    }
}
