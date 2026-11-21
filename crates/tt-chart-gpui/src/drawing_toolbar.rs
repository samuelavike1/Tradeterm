//! TradingView's floating toolbar for the selected drawings (spec 4.1), on
//! the tt-ui kit: grip (drag it; the position is remembered), template
//! menu, line / background / text colors (with opacity), line width and
//! style, settings, lock, remove and a "…" menu (clone, copy, visual order,
//! visibility on intervals, hide).
//!
//! Buttons follow the selection's [`Caps`] (with a multi-selection, the
//! controls all of them have); an edit applies to every selected drawing.
//! It stays clear of the pane-controls gutter and the OHLC readout row: it
//! never goes above [`TOP_CLEAR`] below the price pane's top.

use crate::chart::{Chart, ChartEvent, ChartSource};
use crate::drawing_prefs::{DrawingPrefs, TemplateTarget, template_menu};
use gpui_kit::component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_kit::*;
use lucide_icons::Icon;
use std::rc::Rc;
use tt_chart::{Caps, Drawing, LineStyle, Props, Rgba, VisibilityPreset, VisualOrder};
use tt_ui::Menu;
use tt_ui::prelude::*;
use tt_ui::tokens::*;

/// Undo-merge slots of the toolbar's controls (consecutive edits of one
/// control are one undo step).
const SLOT_LINE: u8 = 1;
const SLOT_FILL: u8 = 2;
const SLOT_TEXT: u8 = 3;
const SLOT_WIDTH: u8 = 4;
const SLOT_STYLE: u8 = 5;

/// Space kept free above the toolbar at the top of the price pane: the
/// OHLC readout row and the pane-controls gutter live there (§2.7).
const TOP_CLEAR: f32 = S2 + CONTROL_SM + S1;
/// Distance from the chart's edges.
const EDGE: f32 = S1;

/// Drag payload of the grip.
struct ToolbarDrag;

/// Where in the toolbar the grip was grabbed (set when a drag starts).
#[derive(Default)]
struct ToolbarGrab(Point<Pixels>);

impl Global for ToolbarGrab {}

struct NoPreview;

impl Render for NoPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// A color button's picker, kept across renders; its changes edit the
/// selection.
struct ColorSlot {
    picker: Entity<ColorPickerState>,
    _sub: Subscription,
}

/// Which color a toolbar color button edits.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ColorKind {
    Line,
    Fill,
    Text,
}

impl ColorKind {
    fn slot(self) -> u8 {
        match self {
            ColorKind::Line => SLOT_LINE,
            ColorKind::Fill => SLOT_FILL,
            ColorKind::Text => SLOT_TEXT,
        }
    }

    fn set(self, d: &mut Drawing, c: Rgba) {
        match self {
            ColorKind::Line => d.props.set_line_color(c),
            ColorKind::Fill => d.props.set_fill_color(c),
            ColorKind::Text => d.props.set_text_color(c),
        }
    }

    fn tip(self) -> &'static str {
        match self {
            ColorKind::Line => "Line color",
            ColorKind::Fill => "Background color",
            ColorKind::Text => "Text color",
        }
    }
}

pub(crate) fn to_hsla(c: Rgba) -> Hsla {
    gpui_kit::Rgba {
        r: c.r() as f32 / 255.0,
        g: c.g() as f32 / 255.0,
        b: c.b() as f32 / 255.0,
        a: c.a() as f32 / 255.0,
    }
    .into()
}

pub(crate) fn from_hsla(c: Hsla) -> Rgba {
    let r = gpui_kit::Rgba::from(c);
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Rgba::new(b(r.r), b(r.g), b(r.b), b(r.a))
}

impl<S: ChartSource> Chart<S> {
    /// A floating-toolbar edit of the selection: one undo step per control
    /// (`slot`, 0 = never merged), saved, and remembered as the tool's
    /// last-used style.
    pub(crate) fn toolbar_edit(
        &mut self,
        slot: u8,
        f: impl FnMut(&mut Drawing),
        cx: &mut Context<Self>,
    ) {
        let done = self.state.update(cx, |s, _| {
            let ds = &mut s.source.parts().0.drawings;
            if slot == 0 {
                ds.edit_selected(f)
            } else {
                ds.edit_selected_style(slot, f)
            }
        });
        if !done {
            return;
        }
        let primary = self
            .state
            .read(cx)
            .source
            .view()
            .drawings
            .selected_drawing()
            .map(|d| (d.tool, d.props.clone()));
        if let Some((tool, props)) = primary {
            DrawingPrefs::remember_style(cx, tool, &props);
        }
        self.invalidate(cx);
        cx.emit(ChartEvent::DrawingsChanged);
        cx.notify();
    }

    /// Applies "Visibility on intervals" presets to the selection.
    pub fn set_drawings_visibility(&mut self, p: VisibilityPreset, cx: &mut Context<Self>) {
        let done = self.state.update(cx, |s, _| {
            let ds = &mut s.source.parts().0.drawings;
            let Some(tf) = ds.timeframe() else {
                return false;
            };
            let v = tt_chart::Visibility::preset(p, tf);
            let done = ds.edit_selected(|d| d.visibility = v);
            // Drawings hidden on this interval leave the selection.
            ds.set_timeframe(None);
            ds.set_timeframe(Some(tf));
            done
        });
        if done {
            self.invalidate(cx);
            cx.emit(ChartEvent::DrawingsChanged);
            cx.notify();
        }
    }

    /// A color button bound to the selection's `kind` color.
    fn color_button(
        &self,
        kind: ColorKind,
        current: Rgba,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let chart = cx.entity().downgrade();
        let key = match kind {
            ColorKind::Line => "dt-color-line",
            ColorKind::Fill => "dt-color-fill",
            ColorKind::Text => "dt-color-text",
        };
        let slot = window.use_keyed_state(key, cx, move |window, cx| {
            let picker = cx.new(|cx| ColorPickerState::new(window, cx));
            let sub = cx.subscribe(&picker, move |_, _, ev: &ColorPickerEvent, cx| {
                if let ColorPickerEvent::Change(Some(c)) = ev {
                    let c = from_hsla(*c);
                    chart
                        .update(cx, |ch, cx| {
                            ch.toolbar_edit(kind.slot(), |d| kind.set(d, c), cx)
                        })
                        .ok();
                }
            });
            ColorSlot { picker, _sub: sub }
        });
        let picker = slot.read(cx).picker.clone();
        // Follow the selection (no change event, so nothing feeds back).
        if picker.read(cx).value().map(from_hsla) != Some(current) {
            picker.update(cx, |p, cx| p.set_value(to_hsla(current), window, cx));
        }
        div()
            .id(SharedString::from(format!("{key}-tip")))
            .flex_none()
            .tooltip(tip(kind.tip(), None))
            .child(ColorField::new(&picker).compact())
            .into_any_element()
    }

    /// The floating toolbar, or `None` when nothing is selected (or while
    /// placing / dragging).
    pub(crate) fn drawing_toolbar(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (drawings, primary, layout) = {
            let state = self.state.read(cx);
            let view = state.source.view();
            let ds = &view.drawings;
            if ds.busy() || ds.hidden {
                return None;
            }
            let primary = ds.selected_drawing()?.clone();
            let all: Vec<Drawing> = ds.selected_drawings().cloned().collect();
            (all, primary, view.last_layout().clone())
        };
        let pane = *layout.panes.first()?;
        let plot = layout.plot;
        let this = cx.entity().downgrade();

        let caps = drawings.iter().fold(primary.props.capabilities(), |c, d| {
            c & d.props.capabilities()
        });
        let same_tool = drawings.iter().all(|d| d.tool == primary.tool);
        let p = &primary.props;

        let mut buttons: Vec<AnyElement> = Vec::new();
        if same_tool {
            let (t_cur, t_apply) = (this.clone(), this.clone());
            let target = TemplateTarget {
                tool: primary.tool,
                current: Rc::new(move |cx| {
                    let c = t_cur.upgrade()?;
                    let ds = &c.read(cx).state.read(cx).source.view().drawings;
                    ds.selected_drawing().map(|d| d.props.clone())
                }),
                apply: Rc::new(move |props, _, cx| {
                    t_apply
                        .update(cx, |c, cx| {
                            c.toolbar_edit(
                                0,
                                |d| {
                                    d.apply_style(&props);
                                },
                                cx,
                            )
                        })
                        .ok();
                }),
            };
            buttons.push(
                ToolbarButton::icon("dt-template", Glyph::Lucide(Icon::LayoutTemplate))
                    .size(ControlSize::Sm)
                    .caret()
                    .tooltip("Template")
                    .menu(move |menu, _, cx| template_menu(menu, target.clone(), cx))
                    .into_any_element(),
            );
        }
        if caps.contains(Caps::LINE_COLOR) && !matches!(p, Props::TextBox(_)) {
            buttons.push(self.color_button(ColorKind::Line, p.line_color(), window, cx));
        }
        if caps.contains(Caps::FILL)
            && let Some(f) = p.fill()
        {
            buttons.push(self.color_button(ColorKind::Fill, f.color, window, cx));
        }
        if caps.contains(Caps::TEXT)
            && p.text_on()
            && let Some(ts) = p.text_style()
        {
            buttons.push(self.color_button(ColorKind::Text, ts.color, window, cx));
        }
        if caps.contains(Caps::LINE_WIDTH) {
            let now = p.line_width();
            let t = this.clone();
            buttons.push(
                ToolbarButton::text("dt-width", format!("{now}px"))
                    .size(ControlSize::Sm)
                    .caret()
                    .tooltip("Line width")
                    .menu(move |mut menu, _, _| {
                        menu = menu.width(MenuWidth::Sm);
                        for w in 1u8..=4 {
                            let t = t.clone();
                            menu = menu.row(
                                MenuRow::new(("dt-width-opt", w as usize), format!("{w}px"))
                                    .checked(w == now)
                                    .on_select(move |_, cx| {
                                        t.update(cx, |c, cx| {
                                            c.toolbar_edit(
                                                SLOT_WIDTH,
                                                |d| d.props.set_line_width(w),
                                                cx,
                                            )
                                        })
                                        .ok();
                                    }),
                            );
                        }
                        menu
                    })
                    .into_any_element(),
            );
        }
        if caps.contains(Caps::LINE_STYLE) {
            let now = p.line_style();
            let t = this.clone();
            buttons.push(
                ToolbarButton::text("dt-style", style_mark(now))
                    .size(ControlSize::Sm)
                    .caret()
                    .tooltip("Line style")
                    .menu(move |mut menu, _, _| {
                        menu = menu.width(MenuWidth::Sm);
                        for (i, st) in LineStyle::ALL.into_iter().enumerate() {
                            let t = t.clone();
                            menu = menu.row(
                                MenuRow::new(
                                    ("dt-style-opt", i),
                                    format!("{}   {}", style_mark(st), st.label()),
                                )
                                .checked(st == now)
                                .on_select(move |_, cx| {
                                    t.update(cx, |c, cx| {
                                        c.toolbar_edit(SLOT_STYLE, |d| d.set_style(st), cx)
                                    })
                                    .ok();
                                }),
                            );
                        }
                        menu
                    })
                    .into_any_element(),
            );
        }
        let id = primary.id;
        let t = this.clone();
        buttons.push(
            IconButton::new("dt-settings", Glyph::SETTINGS)
                .size(ControlSize::Sm)
                .tooltip("Settings")
                .on_click(move |_, _, cx| {
                    t.update(cx, |_, cx| cx.emit(ChartEvent::OpenDrawingSettings(id)))
                        .ok();
                })
                .into_any_element(),
        );
        let locked = drawings.iter().all(|d| d.locked);
        let t = this.clone();
        buttons.push(
            IconButton::new(
                "dt-lock",
                if locked {
                    Glyph::LOCK
                } else {
                    Glyph::LOCK_OPEN
                },
            )
            .size(ControlSize::Sm)
            .toggled(locked)
            .tooltip(if locked { "Unlock" } else { "Lock" })
            .on_click(move |_, _, cx| {
                t.update(cx, |c, cx| {
                    c.toggle_drawings_locked(cx);
                })
                .ok();
            })
            .into_any_element(),
        );
        let t = this.clone();
        buttons.push(
            IconButton::new("dt-delete", Glyph::TRASH)
                .size(ControlSize::Sm)
                .tooltip("Remove")
                .on_click(move |_, _, cx| {
                    t.update(cx, |c, cx| {
                        c.delete_selected_drawing(cx);
                    })
                    .ok();
                })
                .into_any_element(),
        );
        let t = this.clone();
        buttons.push(
            ToolbarButton::icon("dt-more", Glyph::ELLIPSIS)
                .size(ControlSize::Sm)
                .tooltip("More")
                .menu(move |menu, _, _| more_menu(menu, t.clone()))
                .into_any_element(),
        );

        // Place: the remembered fraction of the chart, else top center of
        // the price pane; clamped inside the pane, below the top row.
        let n = buttons.len() as f32;
        let w = S0_5 * 2.0 + (CONTROL_SM / 2.0 + S0_5) + n * (CONTROL_SM + S0_5) + 2.0 * S3;
        let h = CONTROL_SM + 2.0 * S0_5 + 2.0;
        let (x, y) = match DrawingPrefs::get(cx).toolbar_pos {
            Some([fx, fy]) => (
                plot.left() + fx * plot.width(),
                plot.top() + fy * plot.height(),
            ),
            None => (pane.center().x - w / 2.0, pane.top() + TOP_CLEAR),
        };
        let x = x.min(pane.right() - w - EDGE).max(plot.left() + EDGE);
        let y = y.min(pane.bottom() - h - EDGE).max(pane.top() + TOP_CLEAR);
        let origin = plot.min;
        let grip_w = S0_5 + CONTROL_SM / 2.0 + S0_5;

        let drag_this = this.clone();
        Some(
            div()
                .id("drawing-floating-toolbar")
                .absolute()
                .left(px(x - origin.x))
                .top(px(y - origin.y))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_drag_move::<ToolbarDrag>(move |e, _, cx| {
                    let grab = cx.default_global::<ToolbarGrab>().0;
                    let p = e.event.position - grab;
                    let (px_, py_) = (f32::from(p.x), f32::from(p.y));
                    let fx = ((px_ - plot.left()) / plot.width().max(1.0)).clamp(0.0, 1.0);
                    let fy = ((py_ - plot.top()) / plot.height().max(1.0)).clamp(0.0, 1.0);
                    DrawingPrefs::update(cx, |pr| pr.toolbar_pos = Some([fx, fy]));
                    drag_this.update(cx, |_, cx| cx.notify()).ok();
                })
                .child(Overlay::new().floating().children(buttons))
                .child(
                    div()
                        .id("drawing-toolbar-grip")
                        .absolute()
                        .left_0()
                        .top_0()
                        .h_full()
                        .w(px(grip_w))
                        .cursor_grab()
                        .tooltip(tip("Drag to move", None))
                        .on_drag(ToolbarDrag, |_, offset, _, cx| {
                            cx.set_global(ToolbarGrab(offset));
                            cx.new(|_| NoPreview)
                        }),
                )
                .into_any_element(),
        )
    }
}

/// The "…" menu: clone, copy, visual order, visibility presets, hide.
fn more_menu<S: ChartSource>(menu: Menu, t: WeakEntity<Chart<S>>) -> Menu {
    let row =
        |id: &'static str, label: &'static str, f: fn(&mut Chart<S>, &mut Context<Chart<S>>)| {
            let t = t.clone();
            MenuRow::new(id, label).on_select(move |_, cx| {
                t.update(cx, f).ok();
            })
        };
    let mut menu = menu
        .width(MenuWidth::Lg)
        .row(
            row("dt-clone", "Clone", |c, cx| {
                c.clone_drawings(cx);
            })
            .icon(Glyph::COPY),
        )
        .row(
            row("dt-copy", "Copy", |c, cx| {
                c.copy_drawings(cx);
            })
            .shortcut("ctrl-c"),
        )
        .separator()
        .section("Visual order");
    for (i, op) in VisualOrder::ALL.into_iter().enumerate() {
        let t = t.clone();
        menu = menu.row(
            MenuRow::new(("dt-order", i), op.label()).on_select(move |_, cx| {
                t.update(cx, |c, cx| {
                    c.reorder_drawings(op, cx);
                })
                .ok();
            }),
        );
    }
    menu = menu.separator().section("Visibility on intervals");
    for (i, p) in VisibilityPreset::ALL.into_iter().enumerate() {
        let t = t.clone();
        menu = menu.row(
            MenuRow::new(("dt-vis", i), p.label()).on_select(move |_, cx| {
                t.update(cx, |c, cx| c.set_drawings_visibility(p, cx)).ok();
            }),
        );
    }
    menu.separator().row(
        row("dt-hide", "Hide", |c, cx| {
            c.hide_drawings(cx);
        })
        .icon(Glyph::EYE_OFF),
    )
}

/// A short text picture of a line style (the style button and menu).
fn style_mark(s: LineStyle) -> &'static str {
    match s {
        LineStyle::Solid => "\u{2014}\u{2014}",
        LineStyle::Dashed => "\u{2013} \u{2013}",
        LineStyle::Dotted => "\u{00b7}\u{00b7}\u{00b7}",
    }
}
