//! A tab session as the chart's data source.

use std::time::{SystemTime, UNIX_EPOCH};
use tt_chart::{ChartData, ChartView};
use tt_chart_gpui::ChartSource;
use tt_session::tab::{ChartTab, Shared};

/// One tab: its live session and the chart title.
pub struct TabSource {
    pub tab: ChartTab,
    pub title: String,
    pub shared: Shared,
}

impl TabSource {
    /// "BTCUSDT · 15m · Binance"
    pub fn refresh_title(&mut self) {
        self.title = format!(
            "{} · {} · {}",
            self.tab.display_symbol(),
            self.tab.timeframe,
            self.tab.source_name(&self.shared)
        );
        // Name label, currency label and watermark (allocates only on change).
        let ticker = self.tab.display_symbol();
        let interval = self.tab.timeframe.to_string();
        let (quote, description) = match &self.tab.symbol_info {
            Some(i) if !i.quote.is_empty() => {
                (i.quote.as_str(), format!("{} / {}", i.base, i.quote))
            }
            _ => ("", String::new()),
        };
        self.tab
            .chart
            .set_symbol_labels(&ticker, &interval, quote, &description);
    }
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

impl ChartSource for TabSource {
    fn parts(&mut self) -> (&mut ChartView, ChartData<'_>) {
        let tab = &mut self.tab;
        let live = tab.synthetic.is_none();
        let bar_close_ms = tab
            .series
            .last()
            .map(|c| tab.timeframe.bar_end_ms(c.open_time_ms));
        (
            &mut tab.chart,
            ChartData {
                series: &tab.series,
                title: &self.title,
                tick_size: tab.symbol_info.as_ref().map(|i| i.tick_size),
                now_ms: live.then(now_ms),
                bar_interval_ms: Some(tab.timeframe.duration_ms()),
                bar_close_ms,
            },
        )
    }

    fn view(&self) -> &ChartView {
        &self.tab.chart
    }

    fn rendered(&mut self, out: tt_chart::RenderOutput) {
        // Scrolled near the oldest bar: load more history.
        if out.needs_older_history {
            self.tab.request_backfill(&self.shared);
        }
    }
}
