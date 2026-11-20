//! The chart as gpui views, in two layers like TradingView's paired canvases:
//!
//! - **base** (cached view): grid, bars, indicators, axes, value tags.
//!   Repainted only after [`Chart::invalidate`] (data, viewport, size or
//!   settings changed); otherwise gpui reuses last frame's primitives.
//! - **overlay** (uncached, on top): crosshair, its axis tags, readout and
//!   legends. Moving the mouse repaints only this.

use crate::canvas::{CanvasFonts, GpuiCanvas};
use egui::{Pos2, Rect, pos2, vec2};
use gpui_kit::*;
use std::time::{Duration, Instant};
use tt_chart::{ChartData, ChartView, RenderInput};
use tt_core::CandleSeries;
use tt_theme::Theme;

/// Paint counters of one layer (for performance overlays and benches).
#[derive(Debug, Clone, Copy, Default)]
pub struct LayerStats {
    pub paints: u64,
    /// CPU time of the last paint (recording + handing primitives to gpui).
    pub last: Duration,
    pub total: Duration,
    pub primitives: usize,
}

impl LayerStats {
    fn record(&mut self, started: Instant, primitives: usize) {
        self.last = started.elapsed();
        self.total += self.last;
        self.paints += 1;
        self.primitives = primitives;
    }
}

/// Everything a chart draws from.
pub struct ChartState {
    pub view: ChartView,
    pub series: CandleSeries,
    pub title: String,
    pub tick_size: Option<f64>,
    pub bar_interval_ms: Option<i64>,
    pub now_ms: Option<i64>,
    pub bar_close_ms: Option<i64>,
    pub theme: Theme,
    pub fonts: CanvasFonts,
    pointer: Option<Pos2>,
    pub base_stats: LayerStats,
    pub overlay_stats: LayerStats,
}

impl ChartState {
    pub fn new(view: ChartView, series: CandleSeries, theme: Theme) -> Self {
        Self {
            view,
            series,
            title: String::new(),
            tick_size: None,
            bar_interval_ms: None,
            now_ms: None,
            bar_close_ms: None,
            theme,
            fonts: CanvasFonts::default(),
            pointer: None,
            base_stats: LayerStats::default(),
            overlay_stats: LayerStats::default(),
        }
    }

    fn paint(&mut self, overlay: bool, b: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let started = Instant::now();
        let full = Rect::from_min_size(
            pos2(f32::from(b.origin.x), f32::from(b.origin.y)),
            vec2(f32::from(b.size.width), f32::from(b.size.height)),
        );
        let ppp = window.scale_factor();
        let Self {
            view,
            series,
            title,
            tick_size,
            bar_interval_ms,
            now_ms,
            bar_close_ms,
            theme,
            fonts,
            pointer,
            ..
        } = self;
        let data = ChartData {
            series,
            title,
            tick_size: *tick_size,
            now_ms: *now_ms,
            bar_interval_ms: *bar_interval_ms,
            bar_close_ms: *bar_close_ms,
        };
        let mut canvas = GpuiCanvas::new(window, fonts, full);
        if overlay {
            let input = RenderInput {
                crosshair: *pointer,
                legend_hover: None,
            };
            view.render_overlay(&mut canvas, data, theme, input);
        } else {
            view.render_base(&mut canvas, full, ppp, data, theme);
        }
        let primitives = canvas.primitives;
        canvas.finish(cx);
        let stats = if overlay {
            &mut self.overlay_stats
        } else {
            &mut self.base_stats
        };
        stats.record(started, primitives);
    }
}

struct BaseLayer {
    state: Entity<ChartState>,
}

impl Render for BaseLayer {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.clone();
        canvas(
            |_, _, _| {},
            move |b, (), window, cx| state.update(cx, |s, cx| s.paint(false, b, window, cx)),
        )
        .size_full()
    }
}

struct OverlayLayer {
    state: Entity<ChartState>,
}

impl Render for OverlayLayer {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.clone();
        canvas(
            |_, _, _| {},
            move |b, (), window, cx| state.update(cx, |s, cx| s.paint(true, b, window, cx)),
        )
        .size_full()
    }
}

/// An interactive chart: drag pans, wheel zooms, the crosshair follows the
/// pointer.
pub struct Chart {
    pub state: Entity<ChartState>,
    base: Entity<BaseLayer>,
    overlay: Entity<OverlayLayer>,
    drag_from: Option<Pos2>,
}

fn pos(p: Point<Pixels>) -> Pos2 {
    pos2(f32::from(p.x), f32::from(p.y))
}

impl Chart {
    pub fn new(state: ChartState, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|_| state);
        let base = cx.new(|_| BaseLayer {
            state: state.clone(),
        });
        let overlay = cx.new(|_| OverlayLayer {
            state: state.clone(),
        });
        Self {
            state,
            base,
            overlay,
            drag_from: None,
        }
    }

    /// Repaint both layers (after data, viewport or settings changed).
    pub fn invalidate(&self, cx: &mut App) {
        self.base.update(cx, |_, cx| cx.notify());
        self.overlay.update(cx, |_, cx| cx.notify());
    }

    /// Changes the chart state and repaints both layers.
    pub fn update_state<R>(&self, cx: &mut App, f: impl FnOnce(&mut ChartState) -> R) -> R {
        let r = self.state.update(cx, |s, _| f(s));
        self.invalidate(cx);
        r
    }

    /// Moves the crosshair; repaints only the overlay.
    pub fn set_pointer(&self, pointer: Option<Pos2>, cx: &mut App) {
        self.state.update(cx, |s, _| s.pointer = pointer);
        self.overlay.update(cx, |_, cx| cx.notify());
    }
}

impl Render for Chart {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("tt-chart")
            .relative()
            .size_full()
            .cursor(CursorStyle::Crosshair)
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                let p = pos(e.position);
                if let Some(from) = this.drag_from {
                    this.drag_from = Some(p);
                    this.update_state(cx, |s| s.view.pan(p.x - from.x));
                    this.set_pointer(None, cx);
                } else {
                    this.set_pointer(Some(p), cx);
                }
            }))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !*hovered {
                    this.set_pointer(None, cx);
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, _, _| {
                    this.drag_from = Some(pos(e.position));
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, e: &MouseUpEvent, _, cx| {
                    this.drag_from = None;
                    this.set_pointer(Some(pos(e.position)), cx);
                }),
            )
            .on_scroll(cx.listener(|this, e: &ScrollEvent, _, cx| {
                let d = e.delta.pixel_delta(px(20.));
                let factor = (f32::from(d.y) * 0.006).exp();
                let x = f32::from(e.position.x);
                this.update_state(cx, |s| {
                    let plot = s.view.last_layout().plot;
                    s.view.zoom(factor, x, plot.right());
                });
            }))
            .child(
                self.base
                    .clone()
                    .cached(StyleRefinement::default().size_full()),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .child(self.overlay.clone()),
            )
    }
}
