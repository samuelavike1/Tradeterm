//! Sliders on the comfortable density (§2.10): [`RangeSlider`] (one or two
//! round handles on a 10 px track) and [`RangeField`] (min field, slider,
//! max field, the TradingView Visibility row).
//!
//! The slider is stateless: the caller owns the values and gets every move
//! through `on_change`. Dragging uses gpui's drag events (no per-frame
//! work while idle); a press on the track moves the nearest handle there.

use crate::button::child_id;
use crate::form::Field;
use crate::theme::{ActiveUiTheme, hsla};
use crate::tokens::*;
use gpui_kit::component::h_flex;
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::cell::Cell;
use std::rc::Rc;
use tt_theme::mix;

type SliderHandler = Rc<dyn Fn(f32, f32, &mut Window, &mut App)>;

/// Drag payload of a handle: which slider and which handle (0 = low).
#[derive(Clone)]
struct SliderDrag {
    slider: ElementId,
    handle: usize,
}

struct NoPreview;

impl Render for NoPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Value at fraction `f` of `min..=max`, snapped to `step`.
pub fn slider_value(min: f32, max: f32, step: f32, f: f32) -> f32 {
    let v = min + (max - min) * f.clamp(0.0, 1.0);
    if step > 0.0 {
        (min + ((v - min) / step).round() * step).clamp(min.min(max), max.max(min))
    } else {
        v
    }
}

/// Fraction of `v` in `min..=max` (0 when the range is empty).
pub fn slider_frac(min: f32, max: f32, v: f32) -> f32 {
    if max > min {
        ((v - min) / (max - min)).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// A slider with one handle ([`RangeSlider::single`]) or two
/// ([`RangeSlider::new`]): track `C_SLIDER_TRACK` tall, radius 5, the
/// selected part filled, handles `C_SLIDER_HANDLE` (bg fill, 2 px ring).
#[derive(IntoElement)]
pub struct RangeSlider {
    id: ElementId,
    min: f32,
    max: f32,
    step: f32,
    lo: f32,
    hi: f32,
    single: bool,
    width: f32,
    fill: Option<Background>,
    on_change: Option<SliderHandler>,
}

impl RangeSlider {
    /// Two handles, `lo..=hi` within `min..=max`.
    pub fn new(id: impl Into<ElementId>, min: f32, max: f32, lo: f32, hi: f32) -> Self {
        Self {
            id: id.into(),
            min,
            max,
            step: 1.0,
            lo,
            hi,
            single: false,
            width: C_W_SLIDER,
            fill: None,
            on_change: None,
        }
    }

    /// One handle at `v` (the opacity slider); `on_change` gets `(v, v)`.
    pub fn single(id: impl Into<ElementId>, min: f32, max: f32, v: f32) -> Self {
        let mut s = Self::new(id, min, max, min, v);
        s.single = true;
        s
    }

    /// Snap step (default 1; 0 = continuous).
    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// Track width (default `C_W_SLIDER`); `0` fills the parent.
    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }

    /// The whole track's background (the opacity slider's gradient).
    pub fn track_fill(mut self, bg: impl Into<Background>) -> Self {
        self.fill = Some(bg.into());
        self
    }

    /// `(low, high)` after a move (`(v, v)` for a single handle).
    pub fn on_change(mut self, f: impl Fn(f32, f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for RangeSlider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.ui();
        let track = if t.dark {
            mix(t.border, t.text, 0.08)
        } else {
            mix(t.bg, t.text, 0.05)
        };
        let filled = if t.dark {
            mix(t.text, t.bg, 0.15)
        } else {
            mix(t.text, t.bg, 0.27)
        };
        let ring = t.text;
        let knob = t.bg;
        let Self {
            id,
            min,
            max,
            step,
            lo,
            hi,
            single,
            width,
            fill,
            on_change,
        } = self;
        let (fl, fh) = (
            if single {
                0.0
            } else {
                slider_frac(min, max, lo)
            },
            slider_frac(min, max, hi),
        );
        let hs = C_SLIDER_HANDLE;
        // The handles' centers run from hs/2 to width - hs/2.
        let bounds = window
            .use_keyed_state(child_id(&id, "slider-bounds"), cx, |_, _| {
                Rc::new(Cell::new(Bounds::<Pixels>::default()))
            })
            .read(cx)
            .clone();
        let frac_at = move |b: Bounds<Pixels>, x: Pixels| -> f32 {
            let w = f32::from(b.size.width) - hs;
            if w <= 0.0 {
                0.0
            } else {
                (f32::from(x - b.left()) - hs / 2.0) / w
            }
        };
        let apply = {
            let on_change = on_change.clone();
            move |handle: usize, f: f32, w: &mut Window, cx: &mut App| {
                let Some(cb) = &on_change else {
                    return;
                };
                let v = slider_value(min, max, step, f);
                if single {
                    cb(v, v, w, cx);
                } else if handle == 0 {
                    cb(v.min(hi), hi, w, cx);
                } else {
                    cb(lo, v.max(lo), w, cx);
                }
            }
        };
        let handle = |k: usize, f: f32| {
            let drag = SliderDrag {
                slider: id.clone(),
                handle: k,
            };
            div()
                .id(child_id(&id, if k == 0 { "lo" } else { "hi" }))
                .absolute()
                .top(px((C_SLIDER_TRACK - hs) / 2.0))
                .left(relative(f))
                .size(px(hs))
                .rounded_full()
                .bg(hsla(knob))
                .border_2()
                .border_color(hsla(ring))
                .cursor_grab()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_drag(drag, |_, _, _, cx| cx.new(|_| NoPreview))
        };
        let (drag_apply, down_apply) = (apply.clone(), apply);
        let (drag_id, down_bounds) = (id.clone(), bounds.clone());
        div()
            .id(id.clone())
            .relative()
            .flex_none()
            .map(|d| {
                if width > 0.0 {
                    d.w(px(width))
                } else {
                    d.w_full()
                }
            })
            .h(px(C_SLIDER_TRACK))
            .rounded(px(C_SLIDER_TRACK / 2.0))
            .map(|d| match fill {
                Some(bg) => d.bg(bg),
                None => d.bg(hsla(track)),
            })
            .cursor_pointer()
            .child(
                canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .when(!single, |d| {
                // Between the handles' centers.
                d.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(px(hs / 2.0))
                        .right(px(hs / 2.0))
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .h_full()
                                .left(relative(fl))
                                .w(relative((fh - fl).max(0.0)))
                                .bg(hsla(filled)),
                        ),
                )
            })
            .on_mouse_down(MouseButton::Left, move |e, w, cx| {
                let f = frac_at(down_bounds.get(), e.position.x);
                let k = if single || (f - fl).abs() <= (f - fh).abs() && f <= fh {
                    if single { 1 } else { 0 }
                } else {
                    1
                };
                down_apply(k, f, w, cx);
            })
            .on_drag_move::<SliderDrag>(move |e, w, cx| {
                let d = e.drag(cx).clone();
                if d.slider != drag_id {
                    return;
                }
                let f = frac_at(e.bounds, e.event.position.x);
                drag_apply(d.handle, f, w, cx);
            })
            .child(
                // Handles travel over `width - hs`.
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .right(px(hs))
                    .when(!single, |d| d.child(handle(0, fl)))
                    .child(handle(1, fh)),
            )
    }
}

/// TradingView's Visibility row controls: `[min] ——o====o—— [max]`, the
/// fields `C_W_FIELD` wide, `C_GAP` apart. Moving the slider writes both
/// fields and calls `on_change`; typing in a field is the caller's input
/// subscription (it updates the value the slider shows).
#[derive(IntoElement)]
pub struct RangeField {
    id: ElementId,
    min_input: Entity<InputState>,
    max_input: Entity<InputState>,
    range: (f32, f32),
    value: (f32, f32),
    disabled: bool,
    on_change: Option<SliderHandler>,
}

impl RangeField {
    pub fn new(
        id: impl Into<ElementId>,
        min_input: &Entity<InputState>,
        max_input: &Entity<InputState>,
        range: (f32, f32),
        value: (f32, f32),
    ) -> Self {
        Self {
            id: id.into(),
            min_input: min_input.clone(),
            max_input: max_input.clone(),
            range,
            value,
            disabled: false,
            on_change: None,
        }
    }

    /// Dimmed while the row's checkbox is off (still editable).
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    pub fn on_change(mut self, f: impl Fn(f32, f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for RangeField {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let (min_in, max_in) = (self.min_input.clone(), self.max_input.clone());
        let f = self.on_change.clone();
        let slider = RangeSlider::new(
            child_id(&self.id, "range"),
            self.range.0,
            self.range.1,
            self.value.0,
            self.value.1,
        )
        .on_change(move |lo, hi, w, cx| {
            for (input, v) in [(&min_in, lo), (&max_in, hi)] {
                let s = format!("{}", v.round() as i64);
                if input.read(cx).value().trim() != s {
                    input.update(cx, |i, cx| i.set_value(s, w, cx));
                }
            }
            if let Some(f) = &f {
                f(lo, hi, w, cx);
            }
        });
        h_flex()
            .gap(px(C_GAP))
            .items_center()
            .when(self.disabled, |d| d.opacity(0.5))
            .child(Field::new(&self.min_input))
            .child(div().pl(px(1.0)).child(slider))
            .child(Field::new(&self.max_input))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn slider_value_snaps_and_clamps() {
        assert_eq!(slider_value(1.0, 59.0, 1.0, 0.0), 1.0);
        assert_eq!(slider_value(1.0, 59.0, 1.0, 1.0), 59.0);
        assert_eq!(slider_value(1.0, 59.0, 1.0, 1.5), 59.0);
        assert_eq!(slider_value(1.0, 11.0, 1.0, 0.44), 5.0);
        assert_eq!(slider_frac(1.0, 11.0, 6.0), 0.5);
        assert_eq!(slider_frac(3.0, 3.0, 3.0), 0.0);
    }
}
