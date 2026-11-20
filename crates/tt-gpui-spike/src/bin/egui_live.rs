//! egui twin of `tt-gpui-spike --live 3`: 3 charts + a 200-row watchlist,
//! all ticking at 10 Hz.

#[allow(dead_code)]
#[path = "../../../tradeterm/src/fonts.rs"]
mod fonts;
#[allow(dead_code)]
#[path = "../../../tradeterm/src/icons.rs"]
mod icons;
#[allow(dead_code)]
#[path = "../../../tradeterm/src/synthetic.rs"]
mod synthetic;

use egui::{Color32, RichText};
use std::time::{Duration, Instant};
use tt_chart::{ChartData, ChartView, Indicator};
use tt_core::{Candle, CandleSeries, Source, Study, StudyKind};
use tt_theme::Theme;

struct Quote {
    symbol: String,
    price: f64,
    open: f64,
    flash: i8,
}

struct App {
    charts: Vec<(ChartView, CandleSeries, String)>,
    quotes: Vec<Quote>,
    rng: u64,
    ticks: u64,
    next_tick: Instant,
}

impl App {
    fn rand(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    }

    fn tick(&mut self) {
        self.ticks += 1;
        let new_bar = self.ticks.is_multiple_of(50);
        for k in 0..self.charts.len() {
            let r = self.rand();
            let s = &mut self.charts[k].1;
            let Some(mut c) = s.last() else { continue };
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
            s.upsert(c);
        }
        for i in 0..self.quotes.len() {
            let r = self.rand();
            let q = &mut self.quotes[i];
            q.flash = q.flash.signum() * (q.flash.abs() - 1).max(0);
            if r.abs() < 0.17 {
                let old = q.price;
                q.price *= 1.0 + r * 0.004;
                q.flash = if q.price >= old { 3 } else { -3 };
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = Instant::now();
        if now >= self.next_tick {
            self.tick();
            self.next_tick = now + Duration::from_millis(100);
        }
        ctx.request_repaint_after(self.next_tick - Instant::now());
        let theme = Theme::dark();
        let up = Color32::from_rgb(0x08, 0x99, 0x81);
        let down = Color32::from_rgb(0xf2, 0x36, 0x45);
        egui::Panel::right("watchlist")
            .exact_size(320.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for q in &self.quotes {
                        let chg = (q.price / q.open - 1.0) * 100.0;
                        let bg = match q.flash {
                            f if f > 0 => up.gamma_multiply(0.2),
                            f if f < 0 => down.gamma_multiply(0.2),
                            _ => Color32::TRANSPARENT,
                        };
                        egui::Frame::new().fill(bg).show(ui, |ui| {
                            ui.set_height(22.0);
                            ui.horizontal(|ui| {
                                ui.add_sized(
                                    [130.0, 20.0],
                                    egui::Label::new(RichText::new(&q.symbol).size(13.0)),
                                );
                                ui.add_sized(
                                    [90.0, 20.0],
                                    egui::Label::new(
                                        RichText::new(format!("{:.2}", q.price)).size(13.0),
                                    ),
                                );
                                ui.label(
                                    RichText::new(format!("{chg:+.2}%"))
                                        .size(13.0)
                                        .color(if chg >= 0.0 { up } else { down }),
                                );
                            });
                        });
                    }
                });
            });
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let h = ui.available_height() / self.charts.len() as f32;
            for (chart, series, title) in &mut self.charts {
                ui.allocate_ui(egui::vec2(ui.available_width(), h), |ui| {
                    let data = ChartData {
                        series,
                        title,
                        tick_size: Some(0.01),
                        now_ms: None,
                        bar_interval_ms: Some(60_000),
                        bar_close_ms: None,
                    };
                    chart.show(ui, data, &theme);
                });
            }
        });
    }
}

fn main() -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1920.0, 1040.0]),
        ..Default::default()
    };
    eframe::run_native(
        "egui live",
        opts,
        Box::new(|cc| {
            let bold = fonts::install(&cc.egui_ctx);
            let charts = (0..3)
                .map(|i| {
                    let mut chart = ChartView::default();
                    chart.style.bold_family = bold.clone();
                    chart.style.icon_family = Some(icons::family());
                    chart.indicators = vec![
                        Indicator::new(StudyKind::Volume.default_study()),
                        Indicator::new(Study::Ema {
                            length: 21,
                            source: Source::Close,
                        }),
                        Indicator::new(StudyKind::BollingerBands.default_study()),
                        Indicator::new(StudyKind::Rsi.default_study()),
                        Indicator::new(StudyKind::Macd.default_study()),
                    ];
                    chart.zoom_to_bars(150);
                    (
                        chart,
                        synthetic::generate_minutes(5_000, 11 + i),
                        format!("LIVE{i} · 1m"),
                    )
                })
                .collect();
            let quotes = (0..200)
                .map(|i| {
                    let p = 10.0 + (i as f64 * 7.3) % 900.0;
                    Quote {
                        symbol: format!("SYM{i:03}USDT"),
                        price: p,
                        open: p,
                        flash: 0,
                    }
                })
                .collect();
            Ok(Box::new(App {
                charts,
                quotes,
                rng: 0x9e3779b97f4a7c15,
                ticks: 0,
                next_tick: Instant::now(),
            }))
        }),
    )
}
