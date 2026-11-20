//! TradingView's left drawing toolbar: cursor, line tools (flyout), Fib
//! retracement, rectangle, measurers (flyout), then magnet, keep drawing,
//! lock all, hide all and remove all.
//!
//! The tool and the toggles are app-wide (like TradingView) and pushed into
//! every chart; a chart reports back when placing a drawing released the
//! tool.

use crate::app::{TabChart, TradeTerm};
use crate::ui::lucide;
use gpui_kit::base::PopoverState;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::*;
use gpui_kit::component::popover::Popover;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::v_flex;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use lucide_icons::Icon;
use tt_chart::DrawingTool;
use tt_chart_gpui::hsla;

/// Width of the toolbar.
pub const WIDTH: f32 = 46.0;
/// Icon color of the active tool (TradingView's blue).
const ACTIVE: ecolor::Color32 = ecolor::Color32::from_rgb(0x29, 0x62, 0xff);

/// Drawing tool and toggles, shared by all charts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawMode {
    pub tool: Option<DrawingTool>,
    pub magnet: bool,
    pub keep_drawing: bool,
    pub locked: bool,
    pub hidden: bool,
    /// Tool shown on the Lines / Measure group buttons (last picked).
    pub line_tool: DrawingTool,
    pub measure_tool: DrawingTool,
}

impl Default for DrawMode {
    fn default() -> Self {
        Self {
            tool: None,
            magnet: false,
            keep_drawing: false,
            locked: false,
            hidden: false,
            line_tool: DrawingTool::TrendLine,
            measure_tool: DrawingTool::DateAndPriceRange,
        }
    }
}

/// Tool for a TradingView shortcut (`Alt + key`, `Alt + Shift + R`).
pub fn shortcut_tool(key: &str, shift: bool) -> Option<DrawingTool> {
    Some(match (key, shift) {
        ("t", false) => DrawingTool::TrendLine,
        ("h", false) => DrawingTool::HorizontalLine,
        ("j", false) => DrawingTool::HorizontalRay,
        ("v", false) => DrawingTool::VerticalLine,
        ("c", false) => DrawingTool::CrossLine,
        ("f", false) => DrawingTool::FibRetracement,
        ("r", true) => DrawingTool::Rectangle,
        _ => return None,
    })
}

impl TradeTerm {
    /// Changes the drawing mode and pushes it into every chart.
    pub fn set_draw_mode(&mut self, f: impl FnOnce(&mut DrawMode), cx: &mut Context<Self>) {
        f(&mut self.draw);
        if let Some(t) = self.draw.tool {
            if DrawingTool::LINES.contains(&t) {
                self.draw.line_tool = t;
            } else if DrawingTool::MEASURERS.contains(&t) {
                self.draw.measure_tool = t;
            }
        }
        self.apply_draw_mode(cx);
        cx.notify();
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
                    d.selected = None;
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
        let t = self.theme().clone();
        let m = self.draw;
        let count = self.drawing_count(cx);

        let tool_button = |id: &'static str, icon: Icon, active: bool, tip: String| {
            let (bg, fg) = (hsla(t.selection), hsla(t.text));
            div()
                .id(id)
                .size(px(34.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.))
                .cursor_pointer()
                .text_color(fg)
                .when(active, |d| d.bg(bg).text_color(hsla(ACTIVE)))
                .hover(move |d| d.bg(bg))
                .child(lucide(icon, 19.))
                .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
        };
        let tip = |tool: DrawingTool| match tool.shortcut() {
            Some(k) => format!("{} ({k})", tool.label()),
            None => tool.label().to_owned(),
        };

        let cursor = tool_button(
            "dt-cursor",
            Icon::Crosshair,
            m.tool.is_none(),
            "Cross".into(),
        )
        .on_click(cx.listener(|this, _, _, cx| this.pick_tool(None, cx)));

        let group = |id: &'static str,
                     shown: DrawingTool,
                     tools: &'static [DrawingTool],
                     title: &'static str,
                     cx: &mut Context<Self>| {
            let active = m.tool.is_some_and(|x| tools.contains(&x));
            let main = tool_button(id, shown.icon(), active, tip(shown)).on_click(cx.listener(
                move |this, _, _, cx| {
                    let pick = (this.draw.tool != Some(shown)).then_some(shown);
                    this.pick_tool(pick, cx)
                },
            ));
            div()
                .relative()
                .child(main)
                .child(div().absolute().right(px(-6.)).top(px(9.)).child(flyout(
                    SharedString::from(format!("{id}-fly")),
                    title,
                    tools,
                    m.tool,
                    cx,
                )))
        };

        let lines = group("dt-lines", m.line_tool, &DrawingTool::LINES, "LINES", cx);
        let fib = tool_button(
            "dt-fib",
            DrawingTool::FibRetracement.icon(),
            m.tool == Some(DrawingTool::FibRetracement),
            tip(DrawingTool::FibRetracement),
        )
        .on_click(cx.listener(|this, _, _, cx| {
            let pick = (this.draw.tool != Some(DrawingTool::FibRetracement))
                .then_some(DrawingTool::FibRetracement);
            this.pick_tool(pick, cx)
        }));
        let rect = tool_button(
            "dt-rect",
            DrawingTool::Rectangle.icon(),
            m.tool == Some(DrawingTool::Rectangle),
            tip(DrawingTool::Rectangle),
        )
        .on_click(cx.listener(|this, _, _, cx| {
            let pick =
                (this.draw.tool != Some(DrawingTool::Rectangle)).then_some(DrawingTool::Rectangle);
            this.pick_tool(pick, cx)
        }));
        let measure = group(
            "dt-measure",
            m.measure_tool,
            &DrawingTool::MEASURERS,
            "MEASURERS",
            cx,
        );

        let magnet = tool_button(
            "dt-magnet",
            Icon::Magnet,
            m.magnet,
            "Magnet: snap to bar open / high / low / close".into(),
        )
        .on_click(cx.listener(|this, _, _, cx| this.set_draw_mode(|m| m.magnet = !m.magnet, cx)));
        let keep = tool_button(
            "dt-keep",
            Icon::PencilLine,
            m.keep_drawing,
            "Keep drawing".into(),
        )
        .on_click(cx.listener(|this, _, _, cx| {
            this.set_draw_mode(|m| m.keep_drawing = !m.keep_drawing, cx)
        }));
        let lock = tool_button(
            "dt-lock",
            if m.locked { Icon::Lock } else { Icon::LockOpen },
            m.locked,
            if m.locked {
                "Unlock all drawings"
            } else {
                "Lock all drawings"
            }
            .into(),
        )
        .on_click(cx.listener(|this, _, _, cx| this.set_draw_mode(|m| m.locked = !m.locked, cx)));
        let hide = tool_button(
            "dt-hide",
            if m.hidden { Icon::EyeOff } else { Icon::Eye },
            m.hidden,
            if m.hidden {
                "Show all drawings"
            } else {
                "Hide all drawings"
            }
            .into(),
        )
        .on_click(cx.listener(|this, _, _, cx| this.set_draw_mode(|m| m.hidden = !m.hidden, cx)));

        // Remove all: the popover's single item is the confirmation.
        let app = cx.entity().downgrade();
        let theme = t.clone();
        let remove = Popover::new("dt-remove-pop")
            .anchor(Anchor::LeftCenter)
            .trigger(
                Button::new("dt-remove")
                    .ghost()
                    .child(lucide(Icon::Trash2, 19.))
                    .tooltip("Remove drawings"),
            )
            .content(move |_, _, cx| {
                let pop = cx.entity();
                let app = app.clone();
                let label = match count {
                    0 => "No drawings to remove".to_owned(),
                    1 => "Remove 1 drawing".to_owned(),
                    n => format!("Remove {n} drawings"),
                };
                div()
                    .id("dt-remove-all")
                    .px_2()
                    .py_1p5()
                    .rounded(px(5.))
                    .when(count > 0, |d| {
                        d.cursor_pointer()
                            .hover(|s| s.bg(hsla(theme.selection)))
                            .on_click(move |_, window, cx| {
                                app.update(cx, |a, cx| a.remove_all_drawings(cx)).ok();
                                dismiss(&pop, window, cx);
                            })
                    })
                    .when(count == 0, |d| d.text_color(hsla(theme.text_muted)))
                    .child(label)
                    .into_any_element()
            });

        let sep = || div().w(px(26.)).h(px(1.)).my_1().bg(hsla(t.border));
        v_flex()
            .id("drawing-toolbar")
            .w(px(WIDTH))
            .h_full()
            .flex_none()
            .items_center()
            .gap_1()
            .pt_1()
            .border_r_1()
            .border_color(hsla(t.border))
            .child(cursor)
            .child(sep())
            .child(lines)
            .child(fib)
            .child(rect)
            .child(measure)
            .child(sep())
            .child(magnet)
            .child(keep)
            .child(lock)
            .child(hide)
            .child(sep())
            .child(remove)
    }
}

/// "MEASURERS" → "Measurers".
fn title_case(s: &str) -> String {
    let lower = s.to_lowercase();
    let mut c = lower.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

fn dismiss(pop: &Entity<PopoverState>, window: &mut Window, cx: &mut App) {
    pop.update(cx, |s, cx| s.dismiss(window, cx));
}

/// The small chevron on a group button and its flyout menu (opens to the
/// right, like TradingView's).
fn flyout(
    id: SharedString,
    title: &'static str,
    tools: &'static [DrawingTool],
    current: Option<DrawingTool>,
    cx: &mut Context<TradeTerm>,
) -> impl IntoElement {
    let app = cx.entity().downgrade();
    Popover::new(id.clone())
        .anchor(Anchor::LeftCenter)
        .trigger(
            Button::new(SharedString::from(format!("{id}-chev")))
                .ghost()
                .xsmall()
                .child(lucide(Icon::ChevronRight, 10.))
                .tooltip(title_case(title)),
        )
        .content(move |_, _, cx| {
            let pop = cx.entity();
            let Some(app_e) = app.upgrade() else {
                return div().into_any_element();
            };
            let theme = app_e.read(cx).theme().clone();
            let mut list = v_flex().w(px(250.)).child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(11.))
                    .text_color(hsla(theme.text_muted))
                    .child(title),
            );
            for &tool in tools {
                let (app, pop) = (app.clone(), pop.clone());
                list = list.child(
                    div()
                        .id(SharedString::from(format!("dt-item-{tool:?}")))
                        .flex()
                        .items_center()
                        .gap_2()
                        .h(px(32.))
                        .px_2()
                        .rounded(px(5.))
                        .cursor_pointer()
                        .hover(|s| s.bg(hsla(theme.selection)))
                        .when(current == Some(tool), |d| d.bg(hsla(theme.selection)))
                        .child(lucide(tool.icon(), 17.))
                        .child(div().flex_1().child(tool.label()))
                        .children(tool.shortcut().map(|k| {
                            div()
                                .text_size(px(11.))
                                .text_color(hsla(theme.text_muted))
                                .child(k)
                        }))
                        .on_click(move |_, window, cx| {
                            app.update(cx, |a, cx| a.pick_tool(Some(tool), cx)).ok();
                            dismiss(&pop, window, cx);
                        }),
                );
            }
            list.into_any_element()
        })
}
