//! "Object tree and data window" (TradingView's third right-sidebar panel,
//! docs/specs/right-panel.md §4): two segmented tabs.
//!
//! - Object tree: the active chart's main series, its indicators (price
//!   pane first, then panes) and its drawings (top of the z-order first),
//!   with eye / lock / trash, click to select, double-click to rename (a
//!   drawing) or open the settings (an indicator), drag to change a
//!   drawing's z-order.
//! - Data window: date and time of the bar under the crosshair (the last
//!   bar without one), then a section per series and indicator with their
//!   values (`crate::data_window`).
//!
//! The view is rendered cached; the app wakes it when the chart changes and
//! the chart when the crosshair moves to another bar.

use crate::app::TradeTerm;
use crate::ui::{chart_type_icon, drawing_tool_icon};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::collections::HashSet;
use tt_chart::ChartView;
use tt_core::Placement;
use tt_ui::prelude::*;
use tt_ui::tokens::*;
use tt_ui::{IconButton, hsla};

/// One row of the object tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Node {
    Series,
    Indicator(u64),
    Drawing(u64),
}

/// The tree's rows, TradingView's order: the series, the price-pane
/// indicators then the pane indicators (each in draw order), then the
/// drawings from the top of the z-order down. `indicators`: (id, in its
/// own pane); `drawings`: ids in z-order (last on top).
pub fn tree_nodes(indicators: &[(u64, bool)], drawings: &[u64]) -> Vec<Node> {
    let mut out = Vec::with_capacity(1 + indicators.len() + drawings.len());
    out.push(Node::Series);
    out.extend(
        indicators
            .iter()
            .filter(|(_, pane)| !pane)
            .map(|(id, _)| Node::Indicator(*id)),
    );
    out.extend(
        indicators
            .iter()
            .filter(|(_, pane)| *pane)
            .map(|(id, _)| Node::Indicator(*id)),
    );
    out.extend(drawings.iter().rev().map(|id| Node::Drawing(*id)));
    out
}

/// Dropping drawing `dragged` on the row of drawing `target`: the z-order
/// index it moves to (it takes the target's place; rows below move down).
pub fn drop_index(z_order: &[u64], dragged: u64, target: u64) -> Option<usize> {
    if dragged == target || !z_order.contains(&dragged) {
        return None;
    }
    z_order.iter().position(|id| *id == target)
}

/// Drag payload of a drawing row.
#[derive(Clone)]
struct DraggedDrawing {
    id: u64,
    label: SharedString,
    icon: Glyph,
}

impl Render for DraggedDrawing {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ui();
        h_flex()
            .h(H_TREE_ROW.px())
            .px(S2.px())
            .gap(S2.px())
            .rounded(R_MD.px())
            .bg(hsla(t.overlay_bg))
            .border_1()
            .border_color(hsla(t.border))
            .text_size(TEXT_BODY.px())
            .text_color(hsla(t.text))
            .child(glyph(self.icon, ICON_SVG))
            .child(self.label.clone())
    }
}

pub struct ObjectTreePanel {
    app: WeakEntity<TradeTerm>,
    /// The data window tab is shown.
    pub(crate) data_window: bool,
    /// Collapsed data-window sections (0 = the series).
    collapsed: HashSet<u64>,
}

impl ObjectTreePanel {
    pub fn new(app: WeakEntity<TradeTerm>, data_window: bool, _cx: &mut Context<Self>) -> Self {
        Self {
            app,
            data_window,
            collapsed: HashSet::new(),
        }
    }

    fn set_tab(&mut self, data: bool, cx: &mut Context<Self>) {
        if self.data_window != data {
            self.data_window = data;
            let app = self.app.clone();
            cx.defer(move |cx| {
                app.update(cx, |a, cx| a.persist(cx)).ok();
            });
            cx.notify();
        }
    }

    /// Runs `f` on the active chart's view; when it reports a change, the
    /// chart repaints and the change is saved (as a drawing change when
    /// `drawings`).
    fn edit(
        &self,
        drawings: bool,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut ChartView) -> bool + 'static,
    ) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        app.update(cx, |app, cx| {
            let Some(chart) = app.tab_chart().cloned() else {
                return;
            };
            let changed = chart.update(cx, |c, cx| {
                c.update_state(cx, |s| f(&mut s.source.tab.chart))
            });
            if changed {
                if drawings {
                    app.drawings_changed(&chart, cx);
                } else {
                    app.persist(cx);
                    cx.notify();
                }
            }
        });
        cx.notify();
    }

    fn rename_drawing(&self, id: u64, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        crate::name_prompt::open(
            window,
            cx,
            crate::name_prompt::PromptOpts {
                title: "Rename",
                label: "Name",
                initial: name,
                ok: "Rename",
                checks: Vec::new(),
            },
            move |name, _, _, _, cx| {
                let name = name.trim().to_owned();
                this.update(cx, |p, cx| {
                    p.edit(true, cx, move |v| {
                        v.drawings.edit_drawing(id, |d| d.name = name)
                    })
                })
                .ok();
                crate::name_prompt::Submit::Close
            },
        );
    }

    fn open_indicator_settings(&self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let app = self.app.clone();
        window.defer(cx, move |window, cx| {
            app.update(cx, |a, cx| {
                if let Some(chart) = a.tab_chart().cloned() {
                    a.open_indicator_settings(chart, id, window, cx);
                }
            })
            .ok();
        });
    }

    fn tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ui().clone();
        let seg = |id: &'static str, label: &'static str, on: bool| {
            div()
                .id(id)
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .rounded(R_MD.px())
                .cursor_pointer()
                .text_size(TEXT_BODY.px())
                .font_weight(FontWeight::MEDIUM)
                .when(on, |d| {
                    d.bg(hsla(t.bg))
                        .border_1()
                        .border_color(hsla(t.border))
                        .text_color(hsla(t.text))
                })
                .when(!on, |d| {
                    d.text_color(hsla(t.text_muted))
                        .hover(|s| s.text_color(hsla(t.text)))
                })
                .child(label)
        };
        h_flex()
            .flex_none()
            .mx(S4.px())
            .mt(S3.px())
            .mb(S2.px())
            .h((H_SEGMENT + 4.0).px())
            .p(S0_5.px())
            .rounded(R_LG.px())
            .bg(hsla(t.panel_alt))
            .child(
                seg("ot-tab-tree", "Object tree", !self.data_window)
                    .on_click(cx.listener(|this, _, _, cx| this.set_tab(false, cx))),
            )
            .child(
                seg("ot-tab-data", "Data window", self.data_window)
                    .on_click(cx.listener(|this, _, _, cx| this.set_tab(true, cx))),
            )
    }

    fn tree(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(app) = self.app.upgrade() else {
            return Empty.into_any_element();
        };
        let Some(chart) = app.read(cx).tab_chart().cloned() else {
            return EmptyState::new("No chart").into_any_element();
        };
        let t = cx.ui().clone();
        let state = chart.read(cx).state.read(cx);
        let tab = &state.source.tab;
        let view = &tab.chart;
        let inds: Vec<(u64, bool)> = view
            .indicators
            .iter()
            .map(|i| (i.id, i.placement() == Placement::Pane))
            .collect();
        let z: Vec<u64> = view.drawings.items.iter().map(|d| d.id).collect();
        let mut rows: Vec<AnyElement> = Vec::new();
        for node in tree_nodes(&inds, &z) {
            match node {
                Node::Series => {
                    let title = format!(
                        "{} · {}, {}",
                        tab.display_symbol(),
                        tab.source_name(&app.read(cx).shared),
                        tab.timeframe
                    );
                    rows.push(
                        tree_row("ot-series", chart_type_icon(view.chart_type), title, &t)
                            .font_weight(FontWeight::SEMIBOLD)
                            .into_any_element(),
                    );
                }
                Node::Indicator(id) => {
                    let Some(ind) = view.indicators.iter().find(|i| i.id == id) else {
                        continue;
                    };
                    let visible = ind.visible;
                    let group = SharedString::from(format!("ot-ind-{id}"));
                    let row = tree_row(
                        group.clone(),
                        Glyph::TREE_INDICATOR,
                        ind.study.legend_title(),
                        &t,
                    )
                    .group(group.clone())
                    .when(!visible, |d| d.text_color(hsla(t.text_disabled)))
                    .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                        if e.click_count() >= 2 {
                            this.open_indicator_settings(id, window, cx);
                        }
                    }))
                    .child(
                        h_flex()
                            .gap(S0_5.px())
                            .child(
                                row_action(
                                    format!("ot-ind-eye-{id}"),
                                    if visible {
                                        Glyph::DRAW_EYE
                                    } else {
                                        Glyph::DRAW_EYE_OFF
                                    },
                                    if visible { "Hide" } else { "Show" },
                                    &group,
                                    !visible,
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.edit(false, cx, move |v| {
                                            if let Some(i) = v.indicator_mut(id) {
                                                i.visible = !i.visible;
                                            }
                                            true
                                        })
                                    },
                                )),
                            )
                            .child(
                                row_action(
                                    format!("ot-ind-del-{id}"),
                                    Glyph::DRAW_TRASH,
                                    "Remove",
                                    &group,
                                    false,
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.edit(false, cx, move |v| {
                                            v.remove_indicator(id);
                                            true
                                        })
                                    },
                                )),
                            ),
                    );
                    rows.push(row.into_any_element());
                }
                Node::Drawing(id) => {
                    let Some(d) = view.drawings.items.iter().find(|d| d.id == id) else {
                        continue;
                    };
                    let (hidden, locked) = (d.hidden, d.locked);
                    let selected = view.drawings.is_selected(id);
                    let title: SharedString = d.title().to_owned().into();
                    let icon = drawing_tool_icon(d.tool);
                    let name = d.name.clone();
                    let group = SharedString::from(format!("ot-dr-{id}"));
                    let z = z.clone();
                    let drag = DraggedDrawing {
                        id,
                        label: title.clone(),
                        icon,
                    };
                    let active = hsla(t.active);
                    let row = tree_row(group.clone(), icon, title, &t)
                        .group(group.clone())
                        .when(selected, |d| d.bg(hsla(t.selection)))
                        .when(hidden, |d| d.text_color(hsla(t.text_disabled)))
                        .on_drag(drag, |d, _, _, cx| cx.new(|_| d.clone()))
                        .drag_over::<DraggedDrawing>(move |s, _, _, _| {
                            s.border_t_2().border_color(active)
                        })
                        .on_drop(cx.listener(move |this, d: &DraggedDrawing, _, cx| {
                            if let Some(to) = drop_index(&z, d.id, id) {
                                let moved = d.id;
                                this.edit(true, cx, move |v| v.drawings.move_drawing(moved, to));
                            }
                        }))
                        .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                            if e.click_count() >= 2 {
                                this.rename_drawing(id, name.clone(), window, cx);
                            } else {
                                this.edit(false, cx, move |v| {
                                    let ds = &mut v.drawings;
                                    if ds.hidden || ds.locked {
                                        return false;
                                    }
                                    ds.select_only(id);
                                    true
                                });
                            }
                        }))
                        .child(
                            h_flex()
                                .gap(S0_5.px())
                                .child(
                                    row_action(
                                        format!("ot-dr-lock-{id}"),
                                        if locked {
                                            Glyph::DRAW_LOCK
                                        } else {
                                            Glyph::DRAW_UNLOCK
                                        },
                                        if locked { "Unlock" } else { "Lock" },
                                        &group,
                                        locked,
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.edit(true, cx, move |v| {
                                                v.drawings
                                                    .edit_drawing(id, |d| d.locked = !d.locked)
                                            })
                                        },
                                    )),
                                )
                                .child(
                                    row_action(
                                        format!("ot-dr-eye-{id}"),
                                        if hidden {
                                            Glyph::DRAW_EYE_OFF
                                        } else {
                                            Glyph::DRAW_EYE
                                        },
                                        if hidden { "Show" } else { "Hide" },
                                        &group,
                                        hidden,
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.edit(true, cx, move |v| {
                                                v.drawings
                                                    .edit_drawing(id, |d| d.hidden = !d.hidden)
                                            })
                                        },
                                    )),
                                )
                                .child(
                                    row_action(
                                        format!("ot-dr-del-{id}"),
                                        Glyph::DRAW_TRASH,
                                        "Remove",
                                        &group,
                                        false,
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.edit(true, cx, move |v| {
                                                v.drawings.remove_drawing(id)
                                            })
                                        },
                                    )),
                                ),
                        );
                    rows.push(row.into_any_element());
                }
            }
        }
        v_flex()
            .id("ot-tree")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(S1.px())
            .children(rows)
            .into_any_element()
    }
}

/// A 38 px tree row: 28 px icon, label, trailing actions.
fn tree_row(
    id: impl Into<ElementId>,
    icon: Glyph,
    label: impl Into<SharedString>,
    t: &tt_theme::Theme,
) -> Stateful<Div> {
    h_flex()
        .id(id.into())
        .flex_none()
        .h(H_TREE_ROW.px())
        .pl(S1.px())
        .pr(S2.px())
        .gap(S1_5.px())
        .rounded(R_MD.px())
        .text_size(TEXT_BODY.px())
        .text_color(hsla(t.text))
        .hover(|s| s.bg(hsla(t.hover)))
        .child(glyph(icon, ICON_SVG))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(label.into()),
        )
}

/// A 22 px row action, shown on hover (always when `pinned`: hidden,
/// locked).
fn row_action(
    id: String,
    g: Glyph,
    tip: &'static str,
    group: &SharedString,
    pinned: bool,
) -> Stateful<Div> {
    let wrap = div().id(SharedString::from(format!("{id}-wrap")));
    let wrap = if pinned {
        wrap
    } else {
        wrap.invisible().group_hover(group.clone(), |s| s.visible())
    };
    wrap.child(
        IconButton::new(SharedString::from(id), g)
            .size(ControlSize::Xs)
            .tooltip(tip),
    )
}

impl Render for ObjectTreePanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = if self.data_window {
            crate::data_window::render(&self.app, &self.collapsed, window, cx)
        } else {
            self.tree(cx)
        };
        v_flex()
            .id("object-tree-panel")
            .size_full()
            .child(self.tabs(cx))
            .child(body)
    }
}

impl ObjectTreePanel {
    /// Data-window section header click.
    pub(crate) fn toggle_section(&mut self, id: u64, cx: &mut Context<Self>) {
        if !self.collapsed.remove(&id) {
            self.collapsed.insert(id);
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn tree_order_is_series_overlays_panes_then_drawings_top_first() {
        let nodes = tree_nodes(
            &[(1, true), (2, false), (3, true), (4, false)],
            &[10, 11, 12],
        );
        assert_eq!(
            nodes,
            [
                Node::Series,
                Node::Indicator(2),
                Node::Indicator(4),
                Node::Indicator(1),
                Node::Indicator(3),
                Node::Drawing(12),
                Node::Drawing(11),
                Node::Drawing(10),
            ]
        );
        assert_eq!(tree_nodes(&[], &[]), [Node::Series]);
    }

    #[::core::prelude::v1::test]
    fn dropping_a_drawing_takes_the_target_place() {
        // z-order 10 (bottom) .. 13 (top); shown top first: 13 12 11 10.
        let z = [10, 11, 12, 13];
        let apply = |dragged: u64, target: u64| {
            let mut v = z.to_vec();
            let to = drop_index(&v, dragged, target).unwrap();
            let from = v.iter().position(|x| *x == dragged).unwrap();
            let d = v.remove(from);
            v.insert(to, d);
            v.iter().rev().copied().collect::<Vec<_>>()
        };
        // Drag the top row (13) onto 11: 13 takes 11's place, 12 and 11 move up.
        assert_eq!(apply(13, 11), [12, 11, 13, 10]);
        // Drag the bottom row (10) onto 12: it moves up to 12's place.
        assert_eq!(apply(10, 12), [13, 10, 12, 11]);
        assert_eq!(drop_index(&z, 11, 11), None);
        assert_eq!(drop_index(&z, 99, 11), None);
    }
}
