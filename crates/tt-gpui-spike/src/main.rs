//! Spike: the tradeterm chart on gpui (gpui-kit 0.10, the stack Longbridge
//! Pro uses), to compare with egui.
//!
//! Modes:
//! - default: one chart, 1M synthetic 1m bars + Volume, EMA, BB, RSI, MACD.
//!   Drag pans, wheel zooms, crosshair follows the pointer.
//! - `--bench [secs]`: same chart, pans every frame; prints frame stats per
//!   second and a summary, then quits.
//! - `--live [charts]`: several charts + a 200-row watchlist, all ticking at
//!   10 Hz (like streaming quotes).

mod canvas;
mod live;
#[allow(dead_code)]
#[path = "../../tradeterm/src/synthetic.rs"]
mod synthetic;
mod ui;

use canvas::GpuiCanvas;
use egui::{Pos2, Rect, pos2, vec2};
use gpui_kit::*;
use std::borrow::Cow;
use std::time::{Duration, Instant};
use tt_chart::{ChartData, ChartView, Indicator, RenderInput};
use tt_core::{CandleSeries, Source, Study, StudyKind};
use tt_theme::Theme;

/// Per-second frame statistics.
#[derive(Default, Clone)]
pub struct FrameStats {
    frames: u32,
    record_ms: f64,
    replay_ms: f64,
    max_frame_ms: f64,
    prims: usize,
    window_start: Option<Instant>,
    last_frame: Option<Instant>,
    /// (fps, avg record ms, avg replay ms, max frame interval ms, prims)
    pub history: Vec<(f64, f64, f64, f64, usize)>,
}

impl FrameStats {
    fn push(&mut self, record_ms: f64, replay_ms: f64, prims: usize, label: &str) {
        let now = Instant::now();
        if let Some(last) = self.last_frame {
            self.max_frame_ms = self.max_frame_ms.max((now - last).as_secs_f64() * 1000.0);
        }
        self.last_frame = Some(now);
        let start = *self.window_start.get_or_insert(now);
        self.frames += 1;
        self.record_ms += record_ms;
        self.replay_ms += replay_ms;
        self.prims = prims;
        let el = (now - start).as_secs_f64();
        if el >= 1.0 {
            let f = self.frames as f64;
            let row = (
                f / el,
                self.record_ms / f,
                self.replay_ms / f,
                self.max_frame_ms,
                self.prims,
            );
            println!(
                "[{label}] fps {:6.1}  record {:5.2} ms  replay {:5.2} ms  worst frame {:6.1} ms  prims {}",
                row.0, row.1, row.2, row.3, row.4
            );
            self.history.push(row);
            let history = std::mem::take(&mut self.history);
            *self = FrameStats {
                history,
                last_frame: self.last_frame,
                ..Default::default()
            };
        }
    }

    pub fn last(&self) -> Option<&(f64, f64, f64, f64, usize)> {
        self.history.last()
    }
}

pub fn studies() -> Vec<Indicator> {
    vec![
        Indicator::new(StudyKind::Volume.default_study()),
        Indicator::new(Study::Ema {
            length: 21,
            source: Source::Close,
        }),
        Indicator::new(StudyKind::BollingerBands.default_study()),
        Indicator::new(StudyKind::Rsi.default_study()),
        Indicator::new(StudyKind::Macd.default_study()),
    ]
}

/// One interactive chart.
pub struct ChartPane {
    pub chart: ChartView,
    pub series: CandleSeries,
    pub title: String,
    theme: Theme,
    crosshair: Option<Pos2>,
    drag_from: Option<Pos2>,
    /// Pan every frame (benchmark).
    bench: Option<Bench>,
    pub stats: FrameStats,
    label: String,
}

struct Bench {
    started: Instant,
    secs: u64,
    frame: u64,
    phase: Option<u64>,
}

fn pos(p: Point<Pixels>) -> Pos2 {
    pos2(f32::from(p.x), f32::from(p.y))
}

impl ChartPane {
    pub fn new(
        series: CandleSeries,
        title: &str,
        label: &str,
        bars_on_screen: Option<usize>,
    ) -> Self {
        let mut chart = ChartView::default();
        chart.indicators = studies();
        if let Some(b) = bars_on_screen {
            chart.zoom_to_bars(b);
        }
        Self {
            chart,
            series,
            title: title.into(),
            theme: Theme::dark(),
            crosshair: None,
            drag_from: None,
            bench: None,
            stats: FrameStats::default(),
            label: label.into(),
        }
    }

    fn paint(&mut self, b: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let full = Rect::from_min_size(
            pos(b.origin),
            vec2(f32::from(b.size.width), f32::from(b.size.height)),
        );
        if let Some(bench) = &mut self.bench {
            bench.frame += 1;
            // Alternate: zoomed far out (decimated, all 1M bars) and zoomed in.
            let phase = (bench.started.elapsed().as_secs() / 4) % 2;
            if bench.phase != Some(phase) {
                bench.phase = Some(phase);
                if phase == 0 {
                    self.chart.fit_all();
                } else {
                    self.chart.zoom_to_bars(300);
                }
            }
            let dx = if (bench.frame / 120) % 2 == 0 {
                6.0
            } else {
                -6.0
            };
            self.chart.pan(dx);
            self.crosshair = Some(pos2(
                full.center().x + (bench.frame % 200) as f32,
                full.center().y,
            ));
            if bench.started.elapsed().as_secs() >= bench.secs {
                summarize(&self.stats);
                cx.quit();
            }
            window.request_animation_frame();
        }
        let ppp = window.scale_factor();
        let t0 = Instant::now();
        let mut c = GpuiCanvas::new(window, full);
        let data = ChartData {
            series: &self.series,
            title: &self.title,
            tick_size: Some(0.01),
            now_ms: None,
            bar_interval_ms: Some(60_000),
            bar_close_ms: None,
        };
        self.chart.render(
            &mut c,
            full,
            ppp,
            data,
            &self.theme,
            RenderInput {
                crosshair: self.crosshair,
                legend_hover: None,
            },
        );
        // On-chart stats (top right of the plot).
        if let Some((fps, rec, rep, worst, prims)) = self.stats.last().copied() {
            let plot = self.chart.last_layout().plot;
            use tt_chart::render::Canvas as _;
            c.text(
                pos2(plot.right() - 10.0, plot.top() + 8.0),
                egui::Align2::RIGHT_TOP,
                &format!("gpui  {fps:.0} fps · record {rec:.2} ms · replay {rep:.2} ms · worst {worst:.0} ms · {prims} prims"),
                12.0,
                self.theme.text_muted,
            );
        }
        let t1 = Instant::now();
        let prims = c.prim_count;
        if self.bench.as_ref().is_some_and(|b| b.frame % 30 == 0) {
            println!(
                "    quads {:.2} ms · polylines {:.2} ms · shape {:.2} ms in {} calls · {} mesh triangles · chart.render {:.2} ms",
                c.quad_time.as_secs_f64() * 1000.0,
                c.mesh_time.as_secs_f64() * 1000.0,
                c.shape_time.as_secs_f64() * 1000.0,
                c.shape_calls,
                c.tris,
                self.chart.stats().draw_time.as_secs_f64() * 1000.0
            );
        }
        c.finish(cx);
        let t2 = Instant::now();
        let label = match &self.bench {
            Some(b) if b.phase == Some(0) => "zoomed out: 1M bars decimated".to_owned(),
            Some(_) => "zoomed in: 300 bars".to_owned(),
            None => self.label.clone(),
        };
        self.stats.push(
            (t1 - t0).as_secs_f64() * 1000.0,
            (t2 - t1).as_secs_f64() * 1000.0,
            prims,
            &label,
        );
    }
}

fn summarize(stats: &FrameStats) {
    // Skip the first second (startup, shader compile).
    let h: Vec<_> = stats.history.iter().skip(1).collect();
    if h.is_empty() {
        return;
    }
    let n = h.len() as f64;
    let avg = |f: fn(&&(f64, f64, f64, f64, usize)) -> f64| h.iter().map(f).sum::<f64>() / n;
    println!(
        "SUMMARY fps {:.1}  record {:.2} ms  replay {:.2} ms  worst frame {:.1} ms",
        avg(|r| r.0),
        avg(|r| r.1),
        avg(|r| r.2),
        h.iter().map(|r| r.3).fold(0.0, f64::max)
    );
}

impl Render for ChartPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        div()
            .size_full()
            .on_mouse_move(cx.listener(|me, e: &MouseMoveEvent, _, cx| {
                let p = pos(e.position);
                if let Some(from) = me.drag_from {
                    me.chart.pan(p.x - from.x);
                    me.drag_from = Some(p);
                    me.crosshair = None;
                } else {
                    me.crosshair = Some(p);
                }
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|me, e: &MouseDownEvent, _, cx| {
                    me.drag_from = Some(pos(e.position));
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|me, _: &MouseUpEvent, _, cx| {
                    me.drag_from = None;
                    cx.notify();
                }),
            )
            .on_scroll(cx.listener(|me, e: &ScrollEvent, _, cx| {
                let d = e.delta.pixel_delta(px(20.));
                let factor = (f32::from(d.y) * 0.006).exp();
                let plot = me.chart.last_layout().plot;
                me.chart.zoom(factor, f32::from(e.position.x), plot.right());
                cx.notify();
            }))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, cx| {
                        this.update(cx, |me, cx| me.paint(bounds, window, cx));
                    },
                )
                .size_full(),
            )
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| args.iter().position(|a| a == name);
    let num = |i: Option<usize>, default: u64| {
        i.and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(default)
    };
    let bench = flag("--bench").map(|i| num(Some(i), 20));
    let live = flag("--live").map(|i| num(Some(i), 3) as usize);
    let ui_demo = flag("--ui").is_some();

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            cx.text_system()
                .add_fonts(vec![Cow::Borrowed(lucide_icons::LUCIDE_FONT_BYTES)])
                .expect("icon font");
            let opts = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1920.), px(1040.)),
                    cx,
                ))),
                ..Default::default()
            };
            if ui_demo {
                gpui_kit::component::Theme::change(gpui_kit::component::ThemeMode::Dark, None, cx);
                let opts = WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(1600.), px(900.)),
                        cx,
                    ))),
                    ..gpui_kit::component::TitleBar::window_options()
                };
                gpui_kit::open_window(opts, cx, |window, cx| {
                    cx.new(|cx| ui::UiDemo::new(window, cx))
                })
                .expect("window");
                return;
            }
            if let Some(n) = live {
                cx.open_window(opts, |_, cx| cx.new(|cx| live::LiveApp::new(n, cx)))
                    .expect("window");
                return;
            }
            let started = Instant::now();
            let series = synthetic::generate_minutes(synthetic::DEFAULT_BARS, 7);
            println!("generated {} bars in {:?}", series.len(), started.elapsed());
            cx.open_window(opts, move |_, cx| {
                cx.new(|_| {
                    let mut pane =
                        ChartPane::new(series, "SYNTH · 1m · 1,000,000 bars", "chart", None);
                    if let Some(secs) = bench {
                        pane.bench = Some(Bench {
                            started: Instant::now(),
                            secs,
                            frame: 0,
                            phase: None,
                        });
                    }
                    pane
                })
            })
            .expect("window");
            let _ = Duration::ZERO;
        });
}
