//! TradingView's floating toolbar for the selected drawings (spec 4.1), on
//! the tt-ui kit at the TradingView-measured density (ui-guidelines
//! §2.10: 38 px tall, 38 px buttons, 24 px grip): grip (drag it; the
//! position is remembered), template menu, line / background / text colors
//! (an icon with a color bar, palette + opacity), "— 1px" line width, line
//! style, settings, lock, remove and a "…" menu (clone, copy, visual order,
//! visibility on intervals, hide). TradingView's "Add alert" is left out
//! until tradeterm has alerts (BACKLOG.md).
//!
//! Buttons follow the selection's [`Caps`] (with a multi-selection, the
//! controls all of them have); an edit applies to every selected drawing.
//! It stays clear of the pane-controls gutter and the OHLC readout row: it
//! never goes above [`TOP_CLEAR`] below the price pane's top.

use crate::chart::{Chart, ChartEvent, ChartSource};
use crate::drawing_prefs::{DrawingPrefs, TemplateTarget, template_menu};
use gpui_kit::*;
use std::rc::Rc;
use tt_chart::drawings::form;
use tt_chart::{Caps, Drawing, LineStyle, Props, Rgba, VisibilityPreset, VisualOrder};
use tt_ui::prelude::*;
use tt_ui::tokens::*;
use tt_ui::{ColorBarButton, LineDash, LineStyleButton, LineWidthButton, Menu};

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

    /// TradingView's icons: a pencil (line), a bucket (fill), a "T" (text).
    fn glyph(self) -> Glyph {
        match self {
            ColorKind::Line => Glyph::PENCIL,
            ColorKind::Fill => Glyph::PAINT_BUCKET,
            ColorKind::Text => Glyph::TYPE,
        }
    }
}

fn dash(s: LineStyle) -> LineDash {
    match s {
        LineStyle::Solid => LineDash::Solid,
        LineStyle::Dashed => LineDash::Dashed,
        LineStyle::Dotted => LineDash::Dotted,
    }
}

fn line_style(d: LineDash) -> LineStyle {
    match d {
        LineDash::Solid => LineStyle::Solid,
        LineDash::Dashed => LineStyle::Dashed,
        LineDash::Dotted => LineStyle::Dotted,
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
    fn color_button(&self, kind: ColorKind, current: Rgba, cx: &mut Context<Self>) -> AnyElement {
        let chart = cx.entity().downgrade();
        let id = match kind {
            ColorKind::Line => "dt-color-line",
            ColorKind::Fill => "dt-color-fill",
            ColorKind::Text => "dt-color-text",
        };
        ColorBarButton::new(id, kind.glyph(), to_hsla(current))
            .tooltip(kind.tip())
            .on_change(move |c, _, cx| {
                let c = from_hsla(c);
                chart
                    .update(cx, |ch, cx| {
                        ch.toolbar_edit(kind.slot(), |d| kind.set(d, c), cx)
                    })
                    .ok();
            })
            .into_any_element()
    }

    /// The floating toolbar, or `None` when nothing is selected (or while
    /// placing / dragging).
    pub(crate) fn drawing_toolbar(
        &self,
        _window: &mut Window,
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

        // TradingView's buttons per tool (docs/specs/drawings-tv-ref.md).
        let caps = drawings
            .iter()
            .fold(form::toolbar_caps(primary.tool, &primary.props), |c, d| {
                c & form::toolbar_caps(d.tool, &d.props)
            });
        let same_tool = drawings.iter().all(|d| d.tool == primary.tool);
        let p = &primary.props;

        let mut buttons: Vec<AnyElement> = Vec::new();
        // Laid-out width of the buttons (most are `FT_BUTTON` squares).
        let mut buttons_w = 0.0;
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
                IconButton::new("dt-template", Glyph::GRID_PLUS)
                    .size(ControlSize::Xl)
                    .tooltip("Template")
                    .menu(Anchor::TopLeft, move |menu, _, cx| {
                        template_menu(menu, target.clone(), cx)
                    })
                    .into_any_element(),
            );
            buttons_w += FT_BUTTON;
        }
        if caps.contains(Caps::LINE_COLOR) && !matches!(p, Props::TextBox(_)) {
            buttons.push(self.color_button(ColorKind::Line, p.line_color(), cx));
            buttons_w += FT_BUTTON;
        }
        if caps.contains(Caps::FILL)
            && let Some(f) = p.fill()
        {
            buttons.push(self.color_button(ColorKind::Fill, f.color, cx));
            buttons_w += FT_BUTTON;
        }
        // TradingView shows the text color whenever the tool has text.
        if caps.contains(Caps::TEXT)
            && let Some(ts) = p.text_style()
        {
            buttons.push(self.color_button(ColorKind::Text, ts.color, cx));
            buttons_w += FT_BUTTON;
        }
        if caps.contains(Caps::LINE_WIDTH) {
            let t = this.clone();
            buttons.push(
                LineWidthButton::new("dt-width", p.line_width())
                    .on_change(move |w, _, cx| {
                        t.update(cx, |c, cx| {
                            c.toolbar_edit(SLOT_WIDTH, |d| d.props.set_line_width(w), cx)
                        })
                        .ok();
                    })
                    .into_any_element(),
            );
            buttons_w += FT_W_WIDTH_BUTTON;
        }
        if caps.contains(Caps::LINE_STYLE) {
            let t = this.clone();
            buttons.push(
                LineStyleButton::new("dt-style", dash(p.line_style()))
                    .on_change(move |d, _, cx| {
                        let st = line_style(d);
                        t.update(cx, |c, cx| {
                            c.toolbar_edit(SLOT_STYLE, |d| d.set_style(st), cx)
                        })
                        .ok();
                    })
                    .into_any_element(),
            );
            buttons_w += FT_BUTTON;
        }
        let id = primary.id;
        let t = this.clone();
        buttons.push(
            IconButton::new("dt-settings", Glyph::SETTINGS)
                .size(ControlSize::Xl)
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
            .size(ControlSize::Xl)
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
                .size(ControlSize::Xl)
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
            IconButton::new("dt-more", Glyph::ELLIPSIS)
                .size(ControlSize::Xl)
                .tooltip("More")
                .menu(Anchor::TopLeft, move |menu, _, _| {
                    more_menu(menu, t.clone())
                })
                .into_any_element(),
        );
        // Settings, lock, remove, more.
        buttons_w += 4.0 * FT_BUTTON;

        // Place: the remembered fraction of the chart, else top center of
        // the price pane; clamped inside the pane, below the top row.
        let w = FT_GRIP + buttons_w;
        let h = FT_HEIGHT;
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
        let grip_w = FT_GRIP;

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
