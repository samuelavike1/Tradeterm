//! Live-data stress: N charts + a 200-row watchlist, all ticking at 10 Hz.

use crate::{ChartPane, synthetic};
use gpui_kit::*;
use std::time::Duration;
use tt_core::Candle;

const ROWS: usize = 200;
const TICK: Duration = Duration::from_millis(100);

struct Quote {
    symbol: SharedString,
    price: f64,
    open: f64,
    /// Ticks left to flash after a change (+1 up / -1 down).
    flash: i8,
}

pub struct LiveApp {
    charts: Vec<Entity<ChartPane>>,
    quotes: Vec<Quote>,
    rng: u64,
    ticks: u64,
}

impl LiveApp {
    pub fn new(n: usize, cx: &mut Context<Self>) -> Self {
        let charts = (0..n)
            .map(|i| {
                let series = synthetic::generate_minutes(5_000, 11 + i as u64);
                cx.new(|_| {
                    ChartPane::new(
                        series,
                        &format!("LIVE{i} · 1m"),
                        &format!("chart{i}"),
                        Some(150),
                    )
                })
            })
            .collect();
        let quotes = (0..ROWS)
            .map(|i| {
                let p = 10.0 + (i as f64 * 7.3) % 900.0;
                Quote {
                    symbol: format!("SYM{i:03}USDT").into(),
                    price: p,
                    open: p,
                    flash: 0,
                }
            })
            .collect();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this.update(cx, |app, cx| app.tick(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        Self {
            charts,
            quotes,
            rng: 0x9e3779b97f4a7c15,
            ticks: 0,
        }
    }

    fn rand(&mut self) -> f64 {
        // xorshift
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        self.ticks += 1;
        let new_bar = self.ticks.is_multiple_of(50);
        for chart in self.charts.clone() {
            let r = self.rand();
            chart.update(cx, |pane, cx| {
                let Some(mut c) = pane.series.last() else {
                    return;
                };
                if new_bar {
                    c = Candle {
                        open_time_ms: c.open_time_ms + 60_000,
                        open: c.close,
                        high: c.close,
                        low: c.close,
                        close: c.close,
                        volume: 0.0,
                    };
                }
                c.close *= 1.0 + r * 0.002;
                c.high = c.high.max(c.close);
                c.low = c.low.min(c.close);
                c.volume += 1.0 + r.abs() * 10.0;
                pane.series.upsert(c);
                cx.notify();
            });
        }
        for i in 0..self.quotes.len() {
            let r = self.rand();
            let q = &mut self.quotes[i];
            q.flash = q.flash.signum() * (q.flash.abs() - 1).max(0);
            // About a third of symbols tick each interval.
            if r.abs() < 0.17 {
                let old = q.price;
                q.price *= 1.0 + r * 0.004;
                q.flash = if q.price >= old { 3 } else { -3 };
            }
        }
        cx.notify();
    }
}

impl Render for LiveApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let up = rgb(0x089981);
        let down = rgb(0xf23645);
        let rows: Vec<AnyElement> = self
            .quotes
            .iter()
            .map(|q| {
                let chg = (q.price / q.open - 1.0) * 100.0;
                let color = if chg >= 0.0 { up } else { down };
                let bg = match q.flash {
                    f if f > 0 => rgba(0x08998133),
                    f if f < 0 => rgba(0xf2364533),
                    _ => rgba(0x00000000),
                };
                div()
                    .flex()
                    .flex_row()
                    .h(px(26.))
                    .px_3()
                    .items_center()
                    .bg(bg)
                    .text_size(px(13.))
                    .child(
                        div()
                            .w(px(130.))
                            .text_color(rgb(0xdddddd))
                            .child(q.symbol.clone()),
                    )
                    .child(
                        div()
                            .w(px(90.))
                            .text_color(rgb(0xdddddd))
                            .child(format!("{:.2}", q.price)),
                    )
                    .child(div().text_color(color).child(format!("{chg:+.2}%")))
                    .into_any_element()
            })
            .collect();
        let _ = cx;
        let charts: Vec<AnyElement> = self
            .charts
            .iter()
            .map(|c| {
                div()
                    .flex_1()
                    .border_b_1()
                    .border_color(rgb(0x303030))
                    .child(c.clone())
                    .into_any_element()
            })
            .collect();
        div()
            .size_full()
            .flex()
            .flex_row()
            .bg(rgb(0x0a0a0a))
            .font_family("Segoe UI")
            .child(div().flex_1().flex().flex_col().children(charts))
            .child(
                div()
                    .id("watchlist")
                    .w(px(320.))
                    .h_full()
                    .border_l_1()
                    .border_color(rgb(0x303030))
                    .overflow_y_scroll()
                    .children(rows),
            )
    }
}
