//! The chart as gpui views, in two layers like TradingView's paired canvases:
//!
//! - **base** (cached view): grid, bars, indicators, axes, value tags.
//!   Repainted only after [`Chart::invalidate`] (data, viewport, size or
//!   settings changed); otherwise gpui reuses last frame's primitives.
//! - **overlay** (uncached, on top): crosshair, its axis tags, readout and
//!   legends. Moving the mouse repaints only this.

use crate::canvas::{CanvasFonts, GpuiCanvas};
use crate::motion::{DragVelocity, ViewMotion};
use emath::{Pos2, Rect, pos2, vec2};
use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::rc::Rc;
use std::time::{Duration, Instant};
use tt_chart::{
    ChartData, ChartHit, ChartView, DrawingPress, LegendButton, Mods, RenderInput, Repaint,
    ScaleMode,
};
use tt_core::CandleSeries;
use tt_theme::Theme;
use tt_ui::tokens::*;
use tt_ui::{ActiveUiTheme as _, ControlSize, Glyph, IconButton, Menu, MenuRow, hsla};

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
    legend_hover_button: Option<(u64, LegendButton)>,
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
            legend_hover_button: None,
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
        let theme = view.style.theme(&self.theme);
        let mut canvas = GpuiCanvas::new(window, &self.fonts, full);
        let out = if overlay {
            let input = RenderInput {
                crosshair: self.pointer,
                legend_hover: self.legend_hover,
                legend_hover_button: self.legend_hover_button,
            };
            view.render_overlay(&mut canvas, data, &theme, input);
            None
        } else {
            Some(view.render_base(&mut canvas, full, ppp, data, &theme))
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
    /// collapsed, pane sizes, scale mode).
    Changed,
    /// The price-scale menu changed an app-wide chart option; apply it to
    /// every chart and save it.
    StyleChanged(Box<tt_chart::ChartStyle>),
    /// "More settings…" in the price-scale menu.
    OpenSettings,
    /// Drawings changed (save them), or the drawing tool was released after
    /// placing a drawing (update the toolbar).
    DrawingsChanged,
    /// "Settings…" in a drawing's context menu (the drawing settings
    /// dialog; the drawing is selected).
    OpenDrawingSettings(u64),
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
    /// Eased wheel zoom/scroll and fling momentum.
    motion: ViewMotion,
    drag_velocity: DragVelocity,
    /// A next-frame callback is driving `motion`.
    animating: bool,
    /// Pointer over the price axis or the corner (shows A / L / %).
    axis_hover: bool,
    /// Pane under the pointer (index into the layout's panes): shows its
    /// move / delete / collapse / maximize buttons.
    hover_pane: Option<usize>,
    /// The pane whose buttons were last shown (index, id), kept while they
    /// fade out (ui-guidelines G12).
    last_pane: Option<(usize, u64)>,
    /// Legend button under the pointer and its rect: a tooltip hit area
    /// over the canvas-drawn button (L3).
    legend_tip: Option<(u64, LegendButton, Rect)>,
    last_frame: Option<Instant>,
    /// What the last right-click was on (the drawing context menu reads it).
    pub(crate) menu_target: tt_chart::DrawingMenuTarget,
}

impl<S: ChartSource> EventEmitter<ChartEvent> for Chart<S> {}

pub(crate) fn pos(p: Point<Pixels>) -> Pos2 {
    pos2(f32::from(p.x), f32::from(p.y))
}

fn mods(m: &Modifiers) -> Mods {
    Mods {
        shift: m.shift,
        ctrl: m.control || m.platform,
    }
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
            motion: ViewMotion::default(),
            drag_velocity: DragVelocity::default(),
            animating: false,
            last_frame: None,
            axis_hover: false,
            hover_pane: None,
            last_pane: None,
            legend_tip: None,
            menu_target: tt_chart::DrawingMenuTarget::None,
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
        let hit = pointer.map(|p| self.state.read(cx).source.view().hit_test(p));
        let (legend_hover, legend_hover_button) = match hit {
            Some(ChartHit::LegendRow(id)) => (Some(id), None),
            Some(ChartHit::LegendButton(id, b)) => (Some(id), Some((id, b))),
            _ => (None, None),
        };
        self.state.update(cx, |s, _| {
            // No crosshair over the legend, like TradingView.
            s.pointer = pointer.filter(|_| legend_hover.is_none());
            s.legend_hover = legend_hover;
            s.legend_hover_button = legend_hover_button;
        });
        self.overlay.update(cx, |_, cx| cx.notify());
    }

    fn view_mut<R>(&self, cx: &mut App, f: impl FnOnce(&mut ChartView) -> R) -> R {
        self.update_state(cx, |s| f(s.source.parts().0))
    }

    /// Pointer input for drawings: runs `f` with the view and the data.
    fn with_drawings<R>(
        &self,
        cx: &mut App,
        f: impl FnOnce(&mut ChartView, ChartData<'_>) -> R,
    ) -> R {
        self.state.update(cx, |s, _| {
            let (v, d) = s.source.parts();
            f(v, d)
        })
    }

    /// Repaints after drawing input.
    fn repaint(&self, r: Repaint, cx: &mut Context<Self>) {
        match r {
            Repaint::None => {}
            Repaint::Overlay => self.overlay.update(cx, |_, cx| cx.notify()),
            Repaint::All => self.invalidate(cx),
        }
    }

    /// Escape: cancels placing a drawing, releases the tool or clears the
    /// selection. Returns whether it did anything.
    pub fn cancel_drawing(&mut self, cx: &mut Context<Self>) -> bool {
        let done = self.view_mut(cx, |v| v.drawings.cancel());
        if done {
            cx.emit(ChartEvent::DrawingsChanged);
            cx.notify();
        }
        done
    }

    /// Delete / Backspace: removes the selected drawing.
    pub fn delete_selected_drawing(&mut self, cx: &mut Context<Self>) -> bool {
        let done = self.view_mut(cx, |v| v.drawings.delete_selected());
        if done {
            cx.emit(ChartEvent::DrawingsChanged);
            cx.notify();
        }
        done
    }

    /// Enter: finishes an open-ended drawing (path, polyline). Returns
    /// whether it did anything.
    pub fn finish_drawing(&mut self, cx: &mut Context<Self>) -> bool {
        let done = self.view_mut(cx, |v| v.drawings.finish_placing());
        if done {
            cx.emit(ChartEvent::DrawingsChanged);
            cx.notify();
        }
        done
    }

    /// Edits the selected drawing (floating toolbar).
    pub(crate) fn edit_drawing(
        &self,
        cx: &mut Context<Self>,
        f: impl FnMut(&mut tt_chart::Drawing),
    ) {
        if self.view_mut(cx, |v| v.drawings.edit_selected(f)) {
            cx.emit(ChartEvent::DrawingsChanged);
            cx.notify();
        }
    }

    fn mouse_down(&mut self, e: &MouseDownEvent, cx: &mut Context<Self>) {
        let p = pos(e.position);
        self.motion.stop_momentum();
        self.drag_velocity.reset();
        let hit = self.state.read(cx).source.view().hit_test(p);
        let double = e.click_count >= 2;
        // Drawings first: the active tool places points; otherwise a press on
        // a drawing selects it and starts dragging it.
        if hit == ChartHit::Plot && !(double && e.modifiers.control) {
            let m = mods(&e.modifiers);
            match self.with_drawings(cx, |v, d| v.drawing_press(p, m, double, d)) {
                DrawingPress::Ignored => {}
                DrawingPress::Handled => {
                    self.invalidate(cx);
                    cx.notify();
                    return;
                }
                DrawingPress::Placed => {
                    self.invalidate(cx);
                    cx.emit(ChartEvent::DrawingsChanged);
                    cx.notify();
                    return;
                }
            }
        }
        match hit {
            ChartHit::LegendButton(id, button) => {
                // The row changes under the pointer: no stale tooltip.
                self.legend_tip = None;
                cx.notify();
                if let Some(id) = self.view_mut(cx, |v| v.press_legend(id, button)) {
                    cx.emit(ChartEvent::OpenIndicatorSettings(id));
                } else {
                    cx.emit(ChartEvent::Changed);
                }
                self.set_pointer(Some(p), cx);
            }
            ChartHit::LegendRow(id) if double => cx.emit(ChartEvent::OpenIndicatorSettings(id)),
            // Ctrl + double-click collapses the pane (TradingView).
            ChartHit::Plot if double && e.modifiers.control => {
                let view = self.state.read(cx).source.view();
                let l = view.last_layout();
                if let Some(&id) = l
                    .panes
                    .iter()
                    .position(|r| r.y_range().contains(p.y))
                    .and_then(|k| l.pane_ids.get(k))
                {
                    self.edit_panes(cx, |v| v.toggle_pane_collapsed(id));
                }
            }
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
                self.drag_velocity.record(d.x, Instant::now());
                self.set_pointer(None, cx);
            }
            return;
        }
        let m = mods(&e.modifiers);
        let r = self.with_drawings(cx, |v, d| v.drawing_motion(p, m, d));
        self.repaint(r, cx);
        let view = self.state.read(cx).source.view();
        let ds = &view.drawings;
        let hit = view.hit_test(p);
        let cursor = if (ds.tool.is_some() && hit == ChartHit::Plot) || ds.banding() {
            CursorStyle::Crosshair
        } else if ds.busy() {
            CursorStyle::ClosedHand
        } else if ds.hovered.is_some() {
            CursorStyle::PointingHand
        } else {
            cursor_for(hit)
        };
        let l = view.last_layout();
        let axis_hover = l.price_axis.x_range().contains(p.x)
            && p.y >= l.price_axis.top()
            && p.y <= l.time_axis.bottom();
        let hover_pane = (p.x >= l.plot.left() && p.x <= l.price_axis.right())
            .then(|| l.panes.iter().position(|r| r.y_range().contains(p.y)))
            .flatten();
        let legend_tip = match hit {
            ChartHit::LegendButton(id, b) => view.legend_button_rect(id, b).map(|r| (id, b, r)),
            _ => None,
        };
        if cursor != self.cursor
            || axis_hover != self.axis_hover
            || hover_pane != self.hover_pane
            || legend_tip != self.legend_tip
        {
            self.cursor = cursor;
            self.axis_hover = axis_hover;
            self.hover_pane = hover_pane;
            self.legend_tip = legend_tip;
            cx.notify();
        }
        self.set_pointer(Some(p), cx);
    }

    fn mouse_up(&mut self, e: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((Drag::Divider(_), _)) = self.drag {
            cx.emit(ChartEvent::Changed);
        }
        let p = pos(e.position);
        let m = mods(&e.modifiers);
        if self.view_mut(cx, |v| v.drawing_release(p, m)) {
            cx.emit(ChartEvent::DrawingsChanged);
            cx.notify();
        }
        if let Some((Drag::Plot, _)) = self.drag
            && !cx.reduce_motion()
        {
            // Released while moving: keep gliding with friction.
            self.motion
                .fling(self.drag_velocity.release(Instant::now()));
            self.animate(window, cx);
        }
        self.drag = None;
        self.set_pointer(Some(pos(e.position)), cx);
    }
}

impl<S: ChartSource> Chart<S> {
    /// Mouse wheel / trackpad. Wheel notches are eased; trackpad pixel
    /// deltas (already smooth) and reduced motion apply directly.
    fn scroll(&mut self, e: &ScrollEvent, window: &mut Window, cx: &mut Context<Self>) {
        let x = f32::from(e.position.x);
        match e.delta {
            ScrollDelta::Lines(lines) if !cx.reduce_motion() => {
                // Same scale as `ChartView::wheel`: ~20 points per notch.
                let (dx, dy) = (lines.x * 20.0, lines.y * 20.0);
                self.motion
                    .add_zoom(dy * tt_chart_wheel_zoom_per_point(), x);
                self.motion.add_pan(-dx);
                self.animate(window, cx);
            }
            _ => {
                let d = e.delta.pixel_delta(px(20.));
                self.view_mut(cx, |v| v.wheel(vec2(f32::from(d.x), f32::from(d.y)), x));
            }
        }
    }

    /// Starts the per-frame motion loop if it isn't running.
    fn animate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.animating || !self.motion.active() {
            return;
        }
        self.animating = true;
        self.last_frame = Some(Instant::now());
        let this = cx.entity().downgrade();
        window.on_next_frame(move |window, cx| Self::frame(this, window, cx));
    }

    /// One motion frame; reschedules itself while anything is moving.
    fn frame(this: WeakEntity<Self>, window: &mut Window, cx: &mut App) {
        let Some(chart) = this.upgrade() else { return };
        let again = chart.update(cx, |c, cx| {
            let now = Instant::now();
            let dt = c.last_frame.map_or(0.0, |t| (now - t).as_secs_f32());
            c.last_frame = Some(now);
            let step = c.motion.step(dt);
            c.view_mut(cx, |v| {
                if step.zoom != 0.0 {
                    let plot = v.last_layout().plot;
                    let x = if plot.x_range().contains(step.anchor_x) {
                        step.anchor_x
                    } else {
                        plot.right()
                    };
                    v.zoom(step.zoom.exp(), x, plot.right());
                }
                if step.pan != 0.0 {
                    v.pan(step.pan);
                }
            });
            c.animating = c.motion.active();
            c.animating
        });
        if again {
            window.on_next_frame(move |window, cx| Self::frame(this, window, cx));
        }
    }
}

/// `ChartView::wheel`'s zoom sensitivity (ln factor per scrolled point).
fn tt_chart_wheel_zoom_per_point() -> f32 {
    tt_chart::WHEEL_ZOOM_PER_POINT
}

impl<S: ChartSource> Render for Chart<S> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("tt-chart")
            .relative()
            .size_full()
            .cursor(self.cursor)
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| this.mouse_move(e, cx)))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !*hovered && this.drag.is_none() {
                    this.set_pointer(None, cx);
                    if this.axis_hover || this.hover_pane.is_some() || this.legend_tip.is_some() {
                        this.axis_hover = false;
                        this.hover_pane = None;
                        this.legend_tip = None;
                        cx.notify();
                    }
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, _, cx| this.mouse_down(e, cx)),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, e: &MouseUpEvent, window, cx| this.mouse_up(e, window, cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, e: &MouseUpEvent, window, cx| this.mouse_up(e, window, cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, e: &MouseDownEvent, _, cx| this.right_mouse_down(e, cx)),
            )
            .on_scroll(cx.listener(|this, e: &ScrollEvent, window, cx| this.scroll(e, window, cx)))
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
            .children(self.legend_tooltip(cx))
            .children(self.corner_controls(window, cx))
            .children(self.pane_controls(window, cx))
            .children(self.drawing_toolbar(cx))
            .context_menu(self.drawing_menu(cx))
    }
}

/// A price-scale menu action on the chart.
type MenuAction<S> = Rc<dyn Fn(&mut Chart<S>, &mut Context<Chart<S>>)>;

impl<S: ChartSource> Chart<S> {
    /// Changes an app-wide style option and reports it.
    fn edit_style(&self, cx: &mut Context<Self>, f: impl FnOnce(&mut tt_chart::ChartStyle)) {
        let style = self.view_mut(cx, |v| {
            f(&mut v.style);
            v.style.clone()
        });
        cx.emit(ChartEvent::StyleChanged(Box::new(style)));
    }

    /// Changes this chart's scale and reports it.
    fn edit_scale(&self, cx: &mut Context<Self>, f: impl FnOnce(&mut ChartView)) {
        self.view_mut(cx, f);
        cx.emit(ChartEvent::Changed);
    }

    /// Tooltip hit area over the canvas-drawn legend button under the
    /// pointer (ui-guidelines L3, §3.5 `tip()` for canvas hit rects). It
    /// exists only while that button is hovered, so it never goes stale.
    fn legend_tooltip(&self, cx: &App) -> Option<AnyElement> {
        let (id, button, r) = self.legend_tip?;
        let view = self.state.read(cx).source.view();
        let label = match button {
            LegendButton::ToggleVisible => {
                let visible = view
                    .indicators
                    .iter()
                    .find(|i| i.id == id)
                    .is_none_or(|i| i.visible);
                if visible { "Hide" } else { "Show" }
            }
            LegendButton::Settings => "Settings",
            LegendButton::Remove => "Remove",
            LegendButton::Collapse if view.legend_collapsed => "Show indicator legend",
            LegendButton::Collapse => "Hide indicator legend",
        };
        let origin = view.last_layout().plot.min;
        Some(
            div()
                .id(ElementId::Name(
                    format!("legend-tip-{id}-{button:?}").into(),
                ))
                .absolute()
                .left(px(r.left() - origin.x))
                .top(px(r.top() - origin.y))
                .w(px(r.width()))
                .h(px(r.height()))
                .cursor_pointer()
                .tooltip(tt_ui::tip(label, None))
                .into_any_element(),
        )
    }

    /// Bottom-right corner, like TradingView: A / L / % at the bottom of the
    /// price axis (on hover by default) and the gear with the price-scale
    /// menu. Kit controls, chrome colors from the UI theme (G2, G8–G11).
    fn corner_controls(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        use gpui_kit::component::Placement;

        let state = self.state.read(cx);
        let view = state.source.view();
        let l = view.last_layout().clone();
        if l.plot.width() <= 0.0 {
            return None;
        }
        let (mode, auto) = (view.scale_mode, view.is_auto_scale());
        let scale_buttons = view.style.scales.scale_buttons;
        let (panel, border) = {
            let t = cx.ui();
            (hsla(t.panel), hsla(t.border))
        };
        let origin = l.plot.min;
        let rel = |x: f32, y: f32| (px(x - origin.x), px(y - origin.y));

        // A / L / %: fade in and out quickly while the pointer is over the axis.
        let shown = match scale_buttons {
            tt_chart::ScaleButtons::Always => true,
            tt_chart::ScaleButtons::OnHover => self.axis_hover,
            tt_chart::ScaleButtons::Hidden => false,
        };
        let opacity = tt_ui::motion::fade_fast("scale-buttons", shown, window, cx);
        let this = cx.entity().downgrade();
        let letter = |id: &'static str, text: &'static str, on: bool, tip: &'static str| {
            IconButton::text(id, text)
                .size(ControlSize::Xs)
                .toggled(on)
                .tooltip(tip)
                .tooltip_placement(Placement::Top)
        };
        // Opaque (labels must not show through) chrome strip with a top
        // hairline (G9).
        let (x0, y0) = rel(l.price_axis.left(), l.time_axis.top() - CONTROL_SM);
        let buttons = div()
            .absolute()
            .left(x0)
            .top(y0)
            .w(px(l.price_axis.width()))
            .h(px(CONTROL_SM))
            .flex()
            .justify_center()
            .gap(px(S1))
            .items_center()
            .bg(panel)
            .border_t_1()
            .border_color(border)
            .opacity(opacity)
            .when(opacity < 0.01, |d| d.invisible())
            .child(
                letter("scale-auto", "A", auto, "Toggle auto scale").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.edit_scale(cx, |v| {
                            if v.is_auto_scale() {
                                v.freeze_scale()
                            } else {
                                v.auto_scale()
                            }
                        })
                    },
                )),
            )
            .child(
                letter(
                    "scale-log",
                    "L",
                    mode == ScaleMode::Logarithmic,
                    "Toggle log scale",
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.edit_scale(cx, |v| {
                        v.scale_mode = if v.scale_mode == ScaleMode::Logarithmic {
                            ScaleMode::Linear
                        } else {
                            ScaleMode::Logarithmic
                        }
                    })
                })),
            )
            .child(
                letter(
                    "scale-pct",
                    "%",
                    mode == ScaleMode::Percent,
                    "Toggle percent scale",
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.edit_scale(cx, |v| {
                        v.scale_mode = if v.scale_mode == ScaleMode::Percent {
                            ScaleMode::Linear
                        } else {
                            ScaleMode::Percent
                        }
                    })
                })),
            );

        // The gear: TradingView's price-scale menu, opening upwards (G10, G11).
        let (gx, gy) = rel(l.price_axis.left(), l.time_axis.top());
        let gear = div()
            .absolute()
            .left(gx)
            .top(gy)
            .w(px(l.price_axis.width()))
            .h(px(l.time_axis.height()))
            .flex()
            .items_center()
            .justify_center()
            .child(
                IconButton::new("scale-settings", Glyph::SETTINGS)
                    .size(ControlSize::Sm)
                    .tooltip("Price scale settings")
                    .tooltip_placement(Placement::Top)
                    .menu(Anchor::BottomRight, move |menu, _, cx| {
                        Self::price_scale_menu(menu, this.clone(), cx)
                    }),
            );
        Some(
            div()
                .absolute()
                .inset_0()
                .child(buttons.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()))
                .child(gear.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()))
                .into_any_element(),
        )
    }

    /// The price-scale menu, read from the chart each time it is built (it
    /// is rebuilt every frame while open, so checks stay current).
    fn price_scale_menu(menu: Menu, this: WeakEntity<Self>, cx: &App) -> Menu {
        let Some(chart) = this.upgrade() else {
            return menu;
        };
        let view = chart.read(cx).state.read(cx).source.view();
        let (mode, auto, inverted) = (view.scale_mode, view.is_auto_scale(), view.invert_scale);
        let sc = view.style.scales;
        let row = |id: &'static str, label: &'static str, f: MenuAction<S>| {
            let t = this.clone();
            MenuRow::new(id, label).on_select(move |_, cx| {
                let f = f.clone();
                t.update(cx, |c, cx| f(c, cx)).ok();
            })
        };
        let mut menu = menu
            .id("price-scale-menu")
            .row(row(
                "reset-scale",
                "Reset price scale",
                Rc::new(|c, cx| c.edit_scale(cx, |v| v.auto_scale())),
            ))
            .separator()
            .row(
                row(
                    "auto-scale",
                    "Auto (fits data to screen)",
                    Rc::new(|c, cx| {
                        c.edit_scale(cx, |v| {
                            if v.is_auto_scale() {
                                v.freeze_scale()
                            } else {
                                v.auto_scale()
                            }
                        })
                    }),
                )
                .toggle(auto),
            )
            .row(
                row(
                    "invert-scale",
                    "Invert scale",
                    Rc::new(|c, cx| c.edit_scale(cx, |v| v.invert_scale = !v.invert_scale)),
                )
                .toggle(inverted),
            )
            .separator();
        for (i, m) in ScaleMode::ALL.into_iter().enumerate() {
            let t = this.clone();
            menu = menu.row(
                MenuRow::new(("scale-mode", i), m.menu_label())
                    .checked(m == mode)
                    .on_select(move |_, cx| {
                        t.update(cx, |c, cx| c.edit_scale(cx, |v| v.scale_mode = m))
                            .ok();
                    }),
            );
        }
        type Toggle = fn(&mut tt_chart::ChartStyle, bool);
        let toggles: [(&'static str, &'static str, bool, Toggle); 5] = [
            (
                "label-last-price",
                "Symbol last price label",
                sc.symbol_label.shows_value(),
                |s, on| s.scales.symbol_label = s.scales.symbol_label.with_value(on),
            ),
            (
                "label-countdown",
                "Countdown to bar close",
                sc.countdown,
                |s, on| s.scales.countdown = on,
            ),
            (
                "label-indicators",
                "Indicator value labels",
                sc.indicators.shows_value(),
                |s, on| s.scales.indicators = s.scales.indicators.with_value(on),
            ),
            (
                "label-no-overlap",
                "No overlapping labels",
                sc.no_overlapping_labels,
                |s, on| s.scales.no_overlapping_labels = on,
            ),
            (
                "label-price-line",
                "Price line",
                sc.symbol_label.shows_line(),
                |s, on| s.scales.symbol_label = s.scales.symbol_label.with_line(on),
            ),
        ];
        menu = menu.separator().section("Labels and lines");
        for (id, label, on, field) in toggles {
            menu = menu.row(
                row(
                    id,
                    label,
                    Rc::new(move |c, cx| c.edit_style(cx, |s| field(s, !on))),
                )
                .toggle(on),
            );
        }
        let t = this.clone();
        menu.separator()
            .row(
                MenuRow::new("more-settings", "More settings…").on_select(move |_, cx| {
                    t.update(cx, |_, cx| cx.emit(ChartEvent::OpenSettings)).ok();
                }),
            )
    }

    /// Changes the pane layout and reports it.
    fn edit_panes(&self, cx: &mut Context<Self>, f: impl FnOnce(&mut ChartView)) {
        self.view_mut(cx, f);
        cx.emit(ChartEvent::Changed);
        cx.notify();
    }

    /// TradingView's pane buttons at the top right of the hovered pane:
    /// move up / down, delete, collapse / restore, maximize / restore. Kit
    /// `IconButton::Sm` with managed tooltips (G13) on the overlay surface
    /// (G14); they fade in and out (G12).
    fn pane_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        use lucide_icons::Icon;

        let l = self.state.read(cx).source.view().last_layout().clone();
        let n = l.panes.len();
        let shown = self
            .hover_pane
            .filter(|&k| n > 1 && k < n && self.drag.is_none())
            .and_then(|k| Some((k, *l.pane_ids.get(k)?)));
        if shown.is_some() {
            self.last_pane = shown;
        }
        let opacity = tt_ui::motion::fade_fast("pane-controls", shown.is_some(), window, cx);
        // Keep the last pane while fading out, if it still exists.
        let still = |&(k, id): &(usize, u64)| n > 1 && l.pane_ids.get(k) == Some(&id);
        let Some((k, id)) = shown.or(self.last_pane.filter(still)) else {
            self.last_pane = None;
            return None;
        };
        if shown.is_none() && opacity < 0.01 {
            self.last_pane = None;
            return None;
        }
        let view = self.state.read(cx).source.view();
        let (collapsed, maximized) = (view.is_pane_collapsed(id), view.maximized == Some(id));
        let rect = l.panes[k];
        let origin = l.plot.min;
        let bg = hsla(cx.ui().overlay_bg);

        let button = |name: &'static str, glyph: Glyph, tip: &'static str| {
            IconButton::new(name, glyph)
                .size(ControlSize::Sm)
                .tooltip(tip)
        };
        let mut row = div()
            .absolute()
            .top(px(rect.top() - origin.y + S1))
            .right(px(l.price_axis.width() + S1_5))
            .flex()
            .gap(px(S0_5))
            .p(px(S0_5))
            .rounded(px(R_MD))
            .bg(bg)
            .opacity(opacity)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        if k >= 2 {
            row = row.child(
                button("pane-up", Icon::ArrowUp.into(), "Move pane up").on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.edit_panes(cx, |v| {
                            v.move_pane(id, true);
                        })
                    },
                )),
            );
        }
        if k >= 1 && k + 1 < n {
            row = row.child(
                button("pane-down", Icon::ArrowDown.into(), "Move pane down").on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.edit_panes(cx, |v| {
                            v.move_pane(id, false);
                        })
                    }),
                ),
            );
        }
        if k >= 1 {
            row = row.child(button("pane-delete", Glyph::TRASH, "Delete pane").on_click(
                cx.listener(move |this, _, _, cx| {
                    this.hover_pane = None;
                    this.edit_panes(cx, |v| v.remove_indicator(id))
                }),
            ));
        }
        let (glyph, tip): (Glyph, _) = if collapsed {
            (Icon::ChevronsUpDown.into(), "Restore pane")
        } else {
            (Icon::ChevronsDownUp.into(), "Collapse pane")
        };
        row = row.child(button("pane-collapse", glyph, tip).on_click(
            cx.listener(move |this, _, _, cx| this.edit_panes(cx, |v| v.toggle_pane_collapsed(id))),
        ));
        let (glyph, tip) = if maximized {
            (Glyph::MINIMIZE, "Restore pane")
        } else {
            (Glyph::MAXIMIZE, "Maximize pane")
        };
        row = row.child(button("pane-maximize", glyph, tip).on_click(
            cx.listener(move |this, _, _, cx| this.edit_panes(cx, |v| v.toggle_pane_maximized(id))),
        ));
        Some(row.into_any_element())
    }
}
