//! Owns the tokio runtime on a dedicated background thread so the UI thread
//! never runs (or blocks on) async network code.

use std::io;
use std::thread::JoinHandle;
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::sync::oneshot;

pub struct BackgroundRuntime {
    handle: Handle,
    shutdown: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl BackgroundRuntime {
    pub fn start() -> io::Result<Self> {
        let (handle_tx, handle_rx) = std::sync::mpsc::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        let thread = std::thread::Builder::new()
            .name("tokio-main".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .thread_name("tt-net")
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = handle_tx.send(Err(e));
                        return;
                    }
                };
                let _ = handle_tx.send(Ok(rt.handle().clone()));
                // Park this thread until the app asks us to stop.
                rt.block_on(async {
                    let _ = shutdown_rx.await;
                });
                rt.shutdown_timeout(Duration::from_secs(1));
            })?;

        let handle = handle_rx
            .recv()
            .map_err(|_| io::Error::other("tokio thread exited during startup"))??;

        Ok(Self {
            handle,
            shutdown: Some(shutdown_tx),
            thread: Some(thread),
        })
    }

    pub fn handle(&self) -> &Handle {
        &self.handle
    }
}

impl Drop for BackgroundRuntime {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
