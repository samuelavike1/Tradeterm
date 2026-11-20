use crate::Candle;
use crossbeam_channel::Sender;
use std::fmt;
use std::sync::Arc;

/// Connection state of a market-data stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    /// The stream dropped; `reason` is a short human-readable cause if known.
    /// Adapters retry automatically after reporting this.
    Disconnected {
        reason: Option<String>,
    },
}

impl fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionStatus::Connecting => f.write_str("Connecting…"),
            ConnectionStatus::Connected => f.write_str("Connected"),
            ConnectionStatus::Disconnected { reason: None } => f.write_str("Disconnected"),
            ConnectionStatus::Disconnected { reason: Some(r) } => write!(f, "Disconnected: {r}"),
        }
    }
}

/// Events emitted by a market-data source.
#[derive(Debug, Clone, PartialEq)]
pub enum MarketEvent {
    /// Full history snapshot; replaces whatever the consumer has.
    /// Sent on every (re)connect.
    History(Vec<Candle>),
    /// A live bar update. `closed` is `true` once the bar is final.
    CandleUpdate {
        candle: Candle,
        closed: bool,
    },
    Status(ConnectionStatus),
}

/// Where a data source sends its events.
///
/// Wraps a channel sender plus a `wake` callback invoked after every send.
/// The UI passes something like `ctx.request_repaint()` as `wake`, so adapters
/// can trigger repaints without depending on any UI crate.
#[derive(Clone)]
pub struct EventSink {
    tx: Sender<MarketEvent>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl EventSink {
    pub fn new(tx: Sender<MarketEvent>, wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            tx,
            wake: Arc::new(wake),
        }
    }

    /// Sends an event and wakes the consumer.
    /// Returns `false` if the receiver is gone, meaning the source should stop.
    pub fn send(&self, event: MarketEvent) -> bool {
        let ok = self.tx.send(event).is_ok();
        if ok {
            (self.wake)();
        }
        ok
    }
}

impl fmt::Debug for EventSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventSink").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn send_wakes_and_reports_closed() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let wakes = Arc::new(AtomicUsize::new(0));
        let w = wakes.clone();
        let sink = EventSink::new(tx, move || {
            w.fetch_add(1, Ordering::Relaxed);
        });

        assert!(sink.send(MarketEvent::Status(ConnectionStatus::Connected)));
        assert_eq!(wakes.load(Ordering::Relaxed), 1);
        assert_eq!(
            rx.try_recv().unwrap(),
            MarketEvent::Status(ConnectionStatus::Connected)
        );

        drop(rx);
        assert!(!sink.send(MarketEvent::Status(ConnectionStatus::Connecting)));
        assert_eq!(wakes.load(Ordering::Relaxed), 1);
    }
}
