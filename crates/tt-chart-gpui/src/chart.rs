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
use tt_chart::{ChartData, ChartHit, ChartView, RenderInput};
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

/// What a chart draws: its view state and data. Implemented by the app's
/// tab session (live streams) and by [`SeriesSource`] (a fixed series), so
/// the data lives in one place and is never copied per frame.
pub trait ChartSource: 'static {
    /// The view (mutable) and the data to draw this frame.
    fn parts(&mut self) -> (&mut ChartView, ChartData<'_>);
    fn view(&self) -> &ChartView;
    /// Called after each base-layer render (e.g. to load older history).
    fn rendered(&mut self, _out: tt_chart::RenderOutput) {}
}

/// A fixed series (benchmarks, previews).
pub struct SeriesSource {
    pub view: ChartView,
    pub series: CandleSeries,
    pub title: String,
    pub tick_size: Option<f64>,
    pub bar_interval_ms: Option<i64>,
}

impl ChartSource for SeriesSource {
    fn parts(&mut self) -> (&mut ChartView, ChartData<'_>) {
        (
            &mut self.view,
            ChartData {
                series: &self.series,
                title: &self.title,
                tick_size: self.tick_size,
                now_ms: None,
                bar_interval_ms: self.bar_interval_ms,
                bar_close_ms: None,
            },
        )
    }

    fn view(&self) -> &ChartView {
        &self.view
    }
}

/// A chart's source plus its drawing state.
pub struct ChartState<S: ChartSource> {
    pub source: S,
    pub theme: Theme,
    pub fonts: CanvasFonts,
    pointer: Option<Pos2>,
    legend_hover: Option<u64>,
    pub base_stats: LayerStats,
    pub overlay_stats: LayerStats,
}

impl<S: ChartSource> ChartState<S> {
    pub fn new(source: S, theme: Theme) -> Self {
        Self {
            source,
            theme,
            fonts: CanvasFonts::default(),
            pointer: None,
            legend_hover: None,
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
        let (view, data) = self.source.parts();
        let mut canvas = GpuiCanvas::new(window, &self.fonts, full);
        let out = if overlay {
            let input = RenderInput {
                crosshair: self.pointer,
                legend_hover: self.legend_hover,
            };
            view.render_overlay(&mut canvas, data, &self.theme, input);
            None
        } else {
            Some(view.render_base(&mut canvas, full, ppp, data, &self.theme))
        };
        let primitives = canvas.primitives;
        canvas.finish(cx);
        if let Some(out) = out {
            self.source.rendered(out);
        }
        let stats = if overlay {
            &mut self.overlay_stats
        } else {
            &mut self.base_stats
        };
        stats.record(started, primitives);
    }
}

struct BaseLayer<S: ChartSource> {
    state: Entity<ChartState<S>>,
}

impl<S: ChartSource> Render for BaseLayer<S> {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.clone();
        canvas(
            |_, _, _| {},
            move |b, (), window, cx| state.update(cx, |s, cx| s.paint(false, b, window, cx)),
        )
        .size_full()
    }
}

struct OverlayLayer<S: ChartSource> {
    state: Entity<ChartState<S>>,
}

impl<S: ChartSource> Render for OverlayLayer<S> {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.clone();
        canvas(
            |_, _, _| {},
            move |b, (), window, cx| state.update(cx, |s, cx| s.paint(true, b, window, cx)),
        )
        .size_full()
    }
}

/// Something the chart asks its owner to do.
#[derive(Debug, Clone, PartialEq)]
pub enum ChartEvent {
    /// The gear (or a double-click) on an indicator's legend row.
    OpenIndicatorSettings(u64),
    /// Persisted chart state changed (indicators shown/removed, legend
    /// collapsed, pane sizes).
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Drag {
    Plot,
    PriceAxis,
    TimeAxis,
    Divider(usize),
}

/// An interactive chart, TradingView-style: drag the plot to pan, the price
/// axis to scale prices, the time axis to zoom, a pane divider to resize;
/// wheel zooms; double-click resets (plot) or returns to auto-scale (price
/// axis); legend rows show eye / gear / trash on hover.
pub struct Chart<S: ChartSource> {
    pub state: Entity<ChartState<S>>,
    base: Entity<BaseLayer<S>>,
    overlay: Entity<OverlayLayer<S>>,
    drag: Option<(Drag, Pos2)>,
    cursor: CursorStyle,
}

impl<S: ChartSource> EventEmitter<ChartEvent> for Chart<S> {}

fn pos(p: Point<Pixels>) -> Pos2 {
    pos2(f32::from(p.x), f32::from(p.y))
}

fn cursor_for(hit: ChartHit) -> CursorStyle {
    match hit {
        ChartHit::LegendButton(..) => CursorStyle::PointingHand,
        ChartHit::LegendRow(_) => CursorStyle::Arrow,
        ChartHit::PaneDivider(_) => CursorStyle::ResizeUpDown,
        ChartHit::PriceAxis => CursorStyle::ResizeUpDown,
        ChartHit::TimeAxis => CursorStyle::ResizeLeftRight,
        ChartHit::Plot => CursorStyle::Crosshair,
        ChartHit::None => CursorStyle::Arrow,
    }
}

impl<S: ChartSource> Chart<S> {
    pub fn new(state: ChartState<S>, cx: &mut Context<Self>) -> Self {
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
            drag: None,
            cursor: CursorStyle::Crosshair,
        }
    }

    /// Repaint both layers (after data, viewport or settings changed).
    pub fn invalidate(&self, cx: &mut App) {
        self.base.update(cx, |_, cx| cx.notify());
        self.overlay.update(cx, |_, cx| cx.notify());
    }

    /// Changes the chart state and repaints both layers.
    pub fn update_state<R>(&self, cx: &mut App, f: impl FnOnce(&mut ChartState<S>) -> R) -> R {
        let r = self.state.update(cx, |s, _| f(s));
        self.invalidate(cx);
        r
    }

    /// Moves the crosshair; repaints only the overlay.
    pub fn set_pointer(&self, pointer: Option<Pos2>, cx: &mut App) {
        let legend_hover =
            pointer.and_then(|p| match self.state.read(cx).source.view().hit_test(p) {
                ChartHit::LegendRow(id) | ChartHit::LegendButton(id, _) => Some(id),
                _ => None,
            });
        self.state.update(cx, |s, _| {
            // No crosshair over the legend, like TradingView.
            s.pointer = pointer.filter(|_| legend_hover.is_none());
            s.legend_hover = legend_hover;
        });
        self.overlay.update(cx, |_, cx| cx.notify());
    }

    fn view_mut<R>(&self, cx: &mut App, f: impl FnOnce(&mut ChartView) -> R) -> R {
        self.update_state(cx, |s| f(s.source.parts().0))
    }

    fn mouse_down(&mut self, e: &MouseDownEvent, cx: &mut Context<Self>) {
        let p = pos(e.position);
        let hit = self.state.read(cx).source.view().hit_test(p);
        let double = e.click_count >= 2;
        match hit {
            ChartHit::LegendButton(id, button) => {
                if let Some(id) = self.view_mut(cx, |v| v.press_legend(id, button)) {
                    cx.emit(ChartEvent::OpenIndicatorSettings(id));
                } else {
                    cx.emit(ChartEvent::Changed);
                }
                self.set_pointer(Some(p), cx);
            }
            ChartHit::LegendRow(id) if double => cx.emit(ChartEvent::OpenIndicatorSettings(id)),
            ChartHit::Plot if double => self.view_mut(cx, |v| v.reset_view()),
            ChartHit::PriceAxis if double => self.view_mut(cx, |v| v.auto_scale()),
            ChartHit::Plot => self.drag = Some((Drag::Plot, p)),
            ChartHit::PriceAxis => self.drag = Some((Drag::PriceAxis, p)),
            ChartHit::TimeAxis => self.drag = Some((Drag::TimeAxis, p)),
            ChartHit::PaneDivider(k) => self.drag = Some((Drag::Divider(k), p)),
            ChartHit::LegendRow(_) | ChartHit::None => {}
        }
    }

    fn mouse_move(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        let p = pos(e.position);
        if let Some((drag, from)) = self.drag {
            let d = p - from;
            self.drag = Some((drag, p));
            self.view_mut(cx, |v| match drag {
                Drag::Plot => v.drag_plot(d),
                Drag::PriceAxis => v.drag_price_axis(d.y),
                Drag::TimeAxis => v.drag_time_axis(d.x),
                Drag::Divider(k) => v.drag_pane_divider(k, d.y),
            });
            if drag == Drag::Plot {
                self.set_pointer(None, cx);
            }
            return;
        }
        let cursor = cursor_for(self.state.read(cx).source.view().hit_test(p));
        if cursor != self.cursor {
            self.cursor = cursor;
            cx.notify();
        }
        self.set_pointer(Some(p), cx);
    }

    fn mouse_up(&mut self, e: &MouseUpEvent, cx: &mut Context<Self>) {
        if let Some((Drag::Divider(_), _)) = self.drag {
            cx.emit(ChartEvent::Changed);
        }
        self.drag = None;
        self.set_pointer(Some(pos(e.position)), cx);
    }
}

impl<S: ChartSource> Render for Chart<S> {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("tt-chart")
            .relative()
            .size_full()
            .cursor(self.cursor)
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| this.mouse_move(e, cx)))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !*hovered && this.drag.is_none() {
                    this.set_pointer(None, cx);
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, _, cx| this.mouse_down(e, cx)),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, e: &MouseUpEvent, _, cx| this.mouse_up(e, cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, e: &MouseUpEvent, _, cx| this.mouse_up(e, cx)),
            )
            .on_scroll(cx.listener(|this, e: &ScrollEvent, _, cx| {
                let d = e.delta.pixel_delta(px(20.));
                let x = f32::from(e.position.x);
                this.view_mut(cx, |v| v.wheel(vec2(f32::from(d.x), f32::from(d.y)), x));
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
