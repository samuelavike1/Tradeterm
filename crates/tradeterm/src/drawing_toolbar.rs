//! TradingView's left drawing toolbar: cursor, then one button per tool
//! group (a flyout when the group has more than one implemented tool), then
//! magnet, keep drawing, lock all, hide all and remove all.
//!
//! Groups and their tools come from `DrawingTool` (the `LINES` / `FIB` /
//! `MEASURERS` lists, else `available()` by `group()`), so a newly
//! implemented tool shows up here (and a one-tool group becomes a flyout)
//! without touching this file.
//!
//! The tool and the toggles are app-wide (like TradingView) and pushed into
//! every chart; a chart reports back when placing a drawing released the
//! tool.

use crate::app::{KEY_CONTEXT, TabChart, TradeTerm};
use crate::ui::drawing_tool_icon;
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{Placement, Selectable, WindowExt as _, v_flex};
use gpui_kit::*;
use tt_chart::{DrawingTool, ToolGroup};
use tt_ui::menu::MenuNav;
use tt_ui::prelude::*;
use tt_ui::tokens::*;
use tt_ui::{Menu, hsla};

/// The toolbar's groups, top to bottom (TradingView's order).
const GROUPS: [ToolGroup; 6] = [
    ToolGroup::Lines,
    ToolGroup::Fib,
    ToolGroup::Shapes,
    ToolGroup::Annotation,
    ToolGroup::Forecasting,
    ToolGroup::Measurers,
];

fn group_index(g: ToolGroup) -> usize {
    GROUPS.iter().position(|x| *x == g).unwrap_or(0)
}

/// Flyout section title.
fn group_title(g: ToolGroup) -> &'static str {
    match g {
        ToolGroup::Lines => "Lines",
        ToolGroup::Fib => "Fibonacci",
        ToolGroup::Shapes => "Shapes",
        ToolGroup::Annotation => "Annotation",
        ToolGroup::Forecasting => "Forecasting",
        ToolGroup::Measurers => "Measurers",
    }
}

/// The toolbar group a tool sits in: the explicit flyout lists
/// (`DrawingTool::LINES`, `FIB`, `MEASURERS`) win over `DrawingTool::group`,
/// so a tool can live in another flyout until its own group has one (the
/// arrow in Lines).
fn toolbar_group(t: DrawingTool) -> ToolGroup {
    if DrawingTool::LINES.contains(&t) {
        ToolGroup::Lines
    } else if DrawingTool::FIB.contains(&t) {
        ToolGroup::Fib
    } else if DrawingTool::MEASURERS.contains(&t) {
        ToolGroup::Measurers
    } else {
        t.group()
    }
}

/// The implemented tools of a group: the group's list where it has one,
/// in that order, otherwise in `DrawingTool::ALL` order.
fn group_tools(g: ToolGroup) -> Vec<DrawingTool> {
    let listed: &[DrawingTool] = match g {
        ToolGroup::Lines => &DrawingTool::LINES,
        ToolGroup::Fib => &DrawingTool::FIB,
        ToolGroup::Measurers => &DrawingTool::MEASURERS,
        _ => &[],
    };
    if listed.is_empty() {
        DrawingTool::available()
            .filter(|t| toolbar_group(*t) == g)
            .collect()
    } else {
        listed
            .iter()
            .copied()
            .filter(|t| t.is_available())
            .collect()
    }
}

/// Drawing tool and toggles, shared by all charts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawMode {
    pub tool: Option<DrawingTool>,
    pub magnet: tt_chart::MagnetMode,
    pub keep_drawing: bool,
    pub locked: bool,
    pub hidden: bool,
    /// Tool shown on each group button (last picked), by `GROUPS` index.
    pub shown: [Option<DrawingTool>; GROUPS.len()],
}

impl DrawMode {
    /// The tool a group button shows: the last one picked from it, else the
    /// group's default (the date-and-price ruler for measurers, like
    /// TradingView; the first tool otherwise).
    fn shown_tool(&self, g: ToolGroup, tools: &[DrawingTool]) -> DrawingTool {
        self.shown[group_index(g)]
            .filter(|t| tools.contains(t))
            .or_else(|| {
                (g == ToolGroup::Measurers)
                    .then_some(DrawingTool::DateAndPriceRange)
                    .filter(|t| tools.contains(t))
            })
            .unwrap_or(tools[0])
    }
}

impl Default for DrawMode {
    fn default() -> Self {
        Self {
            tool: None,
            magnet: tt_chart::MagnetMode::Off,
            keep_drawing: false,
            locked: false,
            hidden: false,
            shown: [None; GROUPS.len()],
        }
    }
}

actions!(
    drawing,
    [
        PickTrendLine,
        PickHorizontalLine,
        PickHorizontalRay,
        PickVerticalLine,
        PickCrossLine,
        PickFibRetracement,
        PickRectangle,
    ]
);

/// TradingView's drawing shortcuts (`Alt + key`, `Alt + Shift + R`), bound
/// in the app's key context so tooltips can show them.
pub fn key_bindings() -> Vec<KeyBinding> {
    let ctx = Some(KEY_CONTEXT);
    vec![
        KeyBinding::new("alt-t", PickTrendLine, ctx),
        KeyBinding::new("alt-h", PickHorizontalLine, ctx),
        KeyBinding::new("alt-j", PickHorizontalRay, ctx),
        KeyBinding::new("alt-v", PickVerticalLine, ctx),
        KeyBinding::new("alt-c", PickCrossLine, ctx),
        KeyBinding::new("alt-f", PickFibRetracement, ctx),
        KeyBinding::new("alt-shift-r", PickRectangle, ctx),
    ]
}

/// The action that picks `tool`, if it has a shortcut.
pub fn pick_action(tool: DrawingTool) -> Option<Box<dyn Action>> {
    Some(match tool {
        DrawingTool::TrendLine => Box::new(PickTrendLine),
        DrawingTool::HorizontalLine => Box::new(PickHorizontalLine),
        DrawingTool::HorizontalRay => Box::new(PickHorizontalRay),
        DrawingTool::VerticalLine => Box::new(PickVerticalLine),
        DrawingTool::CrossLine => Box::new(PickCrossLine),
        DrawingTool::FibRetracement => Box::new(PickFibRetracement),
        DrawingTool::Rectangle => Box::new(PickRectangle),
        _ => return None,
    })
}

/// Registers the pick-tool action handlers on the app root.
pub fn on_pick_actions(root: Stateful<Div>, cx: &mut Context<TradeTerm>) -> Stateful<Div> {
    fn pick<A: Action>(
        root: Stateful<Div>,
        tool: DrawingTool,
        cx: &mut Context<TradeTerm>,
    ) -> Stateful<Div> {
        root.on_action(
            cx.listener(move |this, _: &A, window, cx| this.on_pick_shortcut(tool, window, cx)),
        )
    }
    let root = pick::<PickTrendLine>(root, DrawingTool::TrendLine, cx);
    let root = pick::<PickHorizontalLine>(root, DrawingTool::HorizontalLine, cx);
    let root = pick::<PickHorizontalRay>(root, DrawingTool::HorizontalRay, cx);
    let root = pick::<PickVerticalLine>(root, DrawingTool::VerticalLine, cx);
    let root = pick::<PickCrossLine>(root, DrawingTool::CrossLine, cx);
    let root = pick::<PickFibRetracement>(root, DrawingTool::FibRetracement, cx);
    pick::<PickRectangle>(root, DrawingTool::Rectangle, cx)
}

impl TradeTerm {
    /// Changes the drawing mode and pushes it into every chart.
    pub fn set_draw_mode(&mut self, f: impl FnOnce(&mut DrawMode), cx: &mut Context<Self>) {
        f(&mut self.draw);
        if let Some(t) = self.draw.tool {
            self.draw.shown[group_index(toolbar_group(t))] = Some(t);
        }
        self.apply_draw_mode(cx);
        cx.notify();
    }

    /// A drawing shortcut: like the old key handler, only while the chart
    /// area has the keyboard (no dialog, app root focused).
    fn on_pick_shortcut(&mut self, tool: DrawingTool, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_dialog(cx)
            || !self.focus.is_focused(window)
            || self.tab_chart().is_none()
        {
            cx.propagate();
            return;
        }
        self.pick_tool(Some(tool), cx);
    }

    /// Picks a tool (picking the active one again returns to the cursor).
    pub fn pick_tool(&mut self, tool: Option<DrawingTool>, cx: &mut Context<Self>) {
        self.set_draw_mode(|m| m.tool = tool, cx);
    }

    pub(crate) fn apply_draw_mode(&self, cx: &mut App) {
        for chart in &self.tabs {
            self.apply_draw_mode_to(chart, cx);
        }
    }

    pub(crate) fn apply_draw_mode_to(&self, chart: &TabChart, cx: &mut App) {
        let m = self.draw;
        chart.update(cx, |c, cx| {
            c.update_state(cx, |s| {
                let d = &mut s.source.tab.chart.drawings;
                if d.tool != m.tool {
                    // Drop a half-placed drawing of the previous tool.
                    if d.busy() {
                        d.cancel();
                    }
                    d.tool = m.tool;
                }
                d.magnet = m.magnet;
                d.keep_drawing = m.keep_drawing;
                d.locked = m.locked;
                d.hidden = m.hidden;
                if m.hidden || m.locked {
                    d.clear_selection();
                }
            });
            cx.notify();
        });
    }

    /// A chart's drawings changed: save, and follow its tool (placing a
    /// drawing releases it unless "keep drawing" is on).
    pub(crate) fn drawings_changed(&mut self, chart: &TabChart, cx: &mut Context<Self>) {
        let tool = chart.read(cx).state.read(cx).source.tab.chart.drawings.tool;
        if tool != self.draw.tool {
            self.draw.tool = tool;
            self.apply_draw_mode(cx);
        }
        self.persist(cx);
        cx.notify();
    }

    fn remove_all_drawings(&mut self, cx: &mut Context<Self>) {
        if let Some(chart) = self.tab_chart().cloned() {
            chart.update(cx, |c, cx| {
                c.update_state(cx, |s| s.source.tab.chart.drawings.remove_all());
                cx.notify();
            });
            self.persist(cx);
            cx.notify();
        }
    }

    fn drawing_count(&self, cx: &App) -> usize {
        self.tab_chart()
            .map(|c| {
                c.read(cx)
                    .state
                    .read(cx)
                    .source
                    .tab
                    .chart
                    .drawings
                    .items
                    .len()
            })
            .unwrap_or(0)
    }

    pub(crate) fn drawing_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let m = self.draw;
        let border = hsla(cx.ui().border);
        let app = cx.entity().downgrade();

        let mut bar = v_flex()
            .id("drawing-toolbar")
            .w(W_DRAWING_TOOLBAR.px())
            .h_full()
            .flex_none()
            .items_center()
            .gap(S1.px())
            .pt(S1.px())
            .border_r_1()
            .border_color(border)
            .child(
                tool_button("dt-cursor", Glyph::DRAW_CURSOR, m.tool.is_none())
                    .tooltip("Cross")
                    .on_click(cx.listener(|this, _, _, cx| this.pick_tool(None, cx))),
            )
            .child(separator());

        for g in GROUPS {
            let tools = group_tools(g);
            if tools.is_empty() {
                continue;
            }
            let shown = m.shown_tool(g, &tools);
            let active = m.tool.is_some_and(|t| tools.contains(&t));
            let id = SharedString::from(format!("dt-group-{g:?}"));
            let mut button =
                with_tool_tooltip(tool_button(id, drawing_tool_icon(shown), active), shown)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let pick = (this.draw.tool != Some(shown)).then_some(shown);
                        this.pick_tool(pick, cx)
                    }));
            if tools.len() > 1 {
                let app = app.clone();
                button = button.with_flyout(move |menu, window, cx| {
                    group_menu(menu, g, &tools, &app, window, cx)
                });
            }
            bar = bar.child(button);
        }

        bar.child(separator())
            .child(
                // A click cycles off → weak → strong.
                tool_button("dt-magnet", Glyph::DRAW_MAGNET, m.magnet.is_on())
                    .tooltip(match m.magnet {
                        tt_chart::MagnetMode::Off => {
                            "Magnet: snap to bar O / H / L / C (click: weak)"
                        }
                        tt_chart::MagnetMode::Weak => {
                            "Weak magnet: snaps when near (click: strong)"
                        }
                        tt_chart::MagnetMode::Strong => "Strong magnet: always snaps (click: off)",
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_draw_mode(|m| m.magnet = m.magnet.next(), cx)
                    })),
            )
            .child(
                tool_button("dt-keep", Glyph::DRAW_KEEP_DRAWING, m.keep_drawing)
                    .tooltip("Keep drawing")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_draw_mode(|m| m.keep_drawing = !m.keep_drawing, cx)
                    })),
            )
            .child(
                tool_button(
                    "dt-lock",
                    if m.locked {
                        Glyph::DRAW_LOCK
                    } else {
                        Glyph::DRAW_UNLOCK
                    },
                    m.locked,
                )
                .tooltip(if m.locked {
                    "Unlock all drawings"
                } else {
                    "Lock all drawings"
                })
                .on_click(
                    cx.listener(|this, _, _, cx| this.set_draw_mode(|m| m.locked = !m.locked, cx)),
                ),
            )
            .child(
                tool_button(
                    "dt-hide",
                    if m.hidden {
                        Glyph::DRAW_EYE_OFF
                    } else {
                        Glyph::DRAW_EYE
                    },
                    m.hidden,
                )
                .tooltip(if m.hidden {
                    "Show all drawings"
                } else {
                    "Hide all drawings"
                })
                .on_click(
                    cx.listener(|this, _, _, cx| this.set_draw_mode(|m| m.hidden = !m.hidden, cx)),
                ),
            )
            .child(separator())
            .child(RemoveAll { app })
    }
}

/// A 34 px toolbar button with a 28 px icon (TradingView-measured;
/// toggled = on), tooltip to the right (§2.7).
fn tool_button(id: impl Into<ElementId>, glyph: impl Into<Glyph>, on: bool) -> IconButton {
    IconButton::new(id, glyph)
        .size(ControlSize::Drawing)
        .toggled(on)
        .tooltip_placement(Placement::Right)
}

/// The tool's label, with its bound key as a Kbd chip when it has one.
fn with_tool_tooltip(b: IconButton, tool: DrawingTool) -> IconButton {
    match pick_action(tool) {
        Some(a) => b.tooltip_action(tool.label(), a.as_ref(), Some(KEY_CONTEXT)),
        None => b.tooltip(tool.label()),
    }
}

/// Divider between toolbar sections, inset `S2` from the toolbar edges.
fn separator() -> impl IntoElement {
    v_flex().w_full().child(Divider::horizontal().inset(S2))
}

/// A group's flyout: section header, then one large row per tool (28 px
/// icon, label, shortcut; the active one inverted, like TradingView).
fn group_menu(
    menu: Menu,
    g: ToolGroup,
    tools: &[DrawingTool],
    app: &WeakEntity<TradeTerm>,
    window: &mut Window,
    cx: &mut App,
) -> Menu {
    let current = app.upgrade().and_then(|a| a.read(cx).draw.tool);
    let has_keys = tools.iter().any(|t| pick_action(*t).is_some());
    let rows: Vec<MenuRow> = tools
        .iter()
        .map(|&tool| {
            let app = app.clone();
            let row = MenuRow::new(
                SharedString::from(format!("dt-item-{tool:?}")),
                tool.label(),
            )
            .icon(drawing_tool_icon(tool))
            .current(current == Some(tool))
            .on_select(move |_, cx| {
                app.update(cx, |a, cx| a.pick_tool(Some(tool), cx)).ok();
            });
            match pick_action(tool) {
                Some(a) => row.shortcut_action(a.as_ref(), Some(KEY_CONTEXT), window),
                None => row,
            }
        })
        .collect();
    menu.id(SharedString::from(format!("dt-menu-{g:?}")))
        .width(if has_keys {
            MenuWidth::Lg
        } else {
            MenuWidth::Md
        })
        .large()
        .section(group_title(g))
        .rows(rows)
}

/// Remove all: a trash button whose click opens a one-row menu (the
/// destructive confirmation), to the right and top-aligned like the flyouts.
#[derive(IntoElement)]
struct RemoveAll {
    app: WeakEntity<TradeTerm>,
}

fn set_open(st: &Entity<bool>, v: bool, cx: &mut App) {
    st.update(cx, |o, cx| {
        if *o != v {
            *o = v;
            cx.notify();
        }
    })
}

impl RenderOnce for RemoveAll {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = CONTROL_DRAWING;
        let open_state = window.use_keyed_state("dt-remove-open", cx, |_, _| false);
        let nav = MenuNav::keyed("dt-remove-nav", window, cx);
        let focus = nav.read(cx).focus.clone();
        let open = *open_state.read(cx);
        let (toggle_st, change_st, dismiss_st) =
            (open_state.clone(), open_state.clone(), open_state);
        let app = self.app;
        let popover = Popover::new("dt-remove-pop")
            .anchor(Anchor::TopLeft)
            .offset(px(0.))
            .appearance(false)
            .open(open)
            .track_focus(&focus)
            .on_open_change(move |o, _, cx| set_open(&change_st, *o, cx))
            .trigger(AnchorPoint)
            .content(move |_, _, cx| {
                let count = app
                    .upgrade()
                    .map(|a| a.read(cx).drawing_count(cx))
                    .unwrap_or(0);
                let label = match count {
                    0 => "No drawings to remove".to_owned(),
                    1 => "Remove 1 drawing".to_owned(),
                    n => format!("Remove {n} drawings"),
                };
                let app = app.clone();
                let dismiss_st = dismiss_st.clone();
                Menu::new()
                    .id("dt-remove-menu")
                    .row(
                        MenuRow::danger("dt-remove-all", label)
                            .icon(Glyph::TRASH)
                            .disabled(count == 0)
                            .on_select(move |_, cx| {
                                app.update(cx, |a, cx| a.remove_all_drawings(cx)).ok();
                            }),
                    )
                    .nav(nav.clone())
                    .on_dismiss(move |_, cx| set_open(&dismiss_st, false, cx))
            });
        div()
            .relative()
            .flex_none()
            .size(s.px())
            // Mouse-down like the flyouts: an outside click closing the menu
            // lands first, so a click on the open trigger just closes it.
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                set_open(&toggle_st, !open, cx)
            })
            .child(
                tool_button("dt-remove", Glyph::DRAW_TRASH, false)
                    .tooltip("Remove drawings")
                    .open(open),
            )
            .child(div().absolute().top_0().left((s + S1).px()).child(popover))
    }
}

/// Zero-size popover trigger that places a menu at a point.
#[derive(IntoElement)]
struct AnchorPoint;

impl Selectable for AnchorPoint {
    fn selected(self, _: bool) -> Self {
        self
    }

    fn is_selected(&self) -> bool {
        false
    }
}

impl RenderOnce for AnchorPoint {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div().size_0()
    }
}
