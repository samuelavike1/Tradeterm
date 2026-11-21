//! TradingView's right-panel "Alerts" (docs/specs/alerts.md §1): an
//! "Alerts | Log" segmented control; the Alerts tab has "+" (create),
//! search, sort and options menus and the alert list (status, symbol,
//! condition, message; edit / stop–restart / clone / delete on hover); the
//! Log tab lists triggered alerts with a clear button. Both have
//! TradingView's empty states.
//!
//! Contract for the sidebar: `AlertsPanel::new(window, cx)` and `Render`;
//! the panel fills the width and height it is given (TradingView's panel
//! is `AL_W_PANEL` = 300 wide) and reads the app's
//! [`crate::alerts::AlertCenter`] global, so it needs nothing else.

use crate::alerts::{AlertCenter, center};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use lucide_icons::Icon as Lucide;
use tt_session::alerts::{Alert, AlertStatus, LogEntry, format_local_short};
use tt_ui::prelude::*;
use tt_ui::tokens::*;
use tt_ui::{SearchField, hsla};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanelTab {
    Alerts,
    Log,
}

/// TradingView's sort menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sort {
    SymbolAsc,
    SymbolDesc,
    NameAsc,
    NameDesc,
    MessageAsc,
    MessageDesc,
    CreatedOldest,
    CreatedNewest,
    TriggeredOldest,
    TriggeredNewest,
}

impl Sort {
    const ALL: [Sort; 10] = [
        Sort::SymbolAsc,
        Sort::SymbolDesc,
        Sort::NameAsc,
        Sort::NameDesc,
        Sort::MessageAsc,
        Sort::MessageDesc,
        Sort::CreatedOldest,
        Sort::CreatedNewest,
        Sort::TriggeredOldest,
        Sort::TriggeredNewest,
    ];

    fn label(self) -> &'static str {
        match self {
            Sort::SymbolAsc => "Symbol (A to Z)",
            Sort::SymbolDesc => "Symbol (Z to A)",
            Sort::NameAsc => "Name (A to Z)",
            Sort::NameDesc => "Name (Z to A)",
            Sort::MessageAsc => "Message (A to Z)",
            Sort::MessageDesc => "Message (Z to A)",
            Sort::CreatedOldest => "Date created (oldest first)",
            Sort::CreatedNewest => "Date created (newest first)",
            Sort::TriggeredOldest => "Time triggering (oldest first)",
            Sort::TriggeredNewest => "Time triggering (newest first)",
        }
    }

    fn sort(self, v: &mut [Alert]) {
        let lower = |s: &str| s.to_lowercase();
        match self {
            Sort::SymbolAsc => v.sort_by_key(|a| lower(&a.symbol)),
            Sort::SymbolDesc => v.sort_by_key(|a| std::cmp::Reverse(lower(&a.symbol))),
            Sort::NameAsc => v.sort_by_key(|a| lower(&a.title())),
            Sort::NameDesc => v.sort_by_key(|a| std::cmp::Reverse(lower(&a.title()))),
            Sort::MessageAsc => v.sort_by_key(|a| lower(&a.message)),
            Sort::MessageDesc => v.sort_by_key(|a| std::cmp::Reverse(lower(&a.message))),
            Sort::CreatedOldest => v.sort_by_key(|a| (a.created_ms, a.id)),
            Sort::CreatedNewest => v.sort_by_key(|a| std::cmp::Reverse((a.created_ms, a.id))),
            Sort::TriggeredOldest => v.sort_by_key(|a| a.last_triggered_ms.unwrap_or(i64::MAX)),
            Sort::TriggeredNewest => {
                v.sort_by_key(|a| std::cmp::Reverse(a.last_triggered_ms.unwrap_or(i64::MIN)))
            }
        }
    }
}

/// "Show alerts" in the options menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Show {
    All,
    Active,
    Inactive,
}

pub struct AlertsPanel {
    tab: PanelTab,
    sort: Sort,
    show: Show,
    only_current_symbol: bool,
    /// The search row is open.
    searching: bool,
    query: Entity<InputState>,
    /// Watching the alert center (it may be installed after the panel).
    observed: bool,
    _subs: Vec<Subscription>,
}

impl AlertsPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let sub = cx.subscribe(&query, |_, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        });
        let mut me = Self {
            tab: PanelTab::Alerts,
            sort: Sort::CreatedNewest,
            show: Show::All,
            only_current_symbol: false,
            searching: false,
            query,
            observed: false,
            _subs: vec![sub],
        };
        me.observe_center(cx);
        me
    }

    fn observe_center(&mut self, cx: &mut Context<Self>) {
        if self.observed {
            return;
        }
        if let Some(c) = center(cx) {
            self._subs.push(cx.observe(&c, |_, _, cx| cx.notify()));
            self.observed = true;
        }
    }

    /// Runs `f` on the alert center.
    fn with_center(cx: &mut App, f: impl FnOnce(&mut AlertCenter, &mut Context<AlertCenter>)) {
        if let Some(c) = center(cx) {
            c.update(cx, f);
        }
    }

    /// The alerts shown: filter, search, sort.
    fn visible_alerts(&self, cx: &App) -> Vec<Alert> {
        let Some(c) = center(cx) else {
            return Vec::new();
        };
        let c = c.read(cx);
        let current = c.chart.as_ref().map(|x| x.symbol.clone());
        let q = self.query.read(cx).value().trim().to_lowercase();
        let mut v: Vec<Alert> = c
            .alerts()
            .iter()
            .filter(|a| match self.show {
                Show::All => true,
                Show::Active => a.status.is_active(),
                Show::Inactive => !a.status.is_active(),
            })
            .filter(|a| !self.only_current_symbol || current.as_deref() == Some(a.symbol.as_str()))
            .filter(|a| {
                !self.searching
                    || q.is_empty()
                    || a.symbol.to_lowercase().contains(&q)
                    || a.title().to_lowercase().contains(&q)
                    || a.message.to_lowercase().contains(&q)
            })
            .cloned()
            .collect();
        self.sort.sort(&mut v);
        v
    }

    // ---- pieces -----------------------------------------------------------

    /// "Alerts | Log": 34 tall, radius 8, the selected segment raised.
    fn segmented(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ui().clone();
        let track = tt_theme::mix(t.bg, t.text, if t.dark { 0.08 } else { 0.05 });
        let raised = if t.dark {
            tt_theme::mix(t.bg, t.text, 0.2)
        } else {
            t.bg
        };
        let seg = |tab: PanelTab, label: &'static str, cx: &mut Context<Self>| {
            let on = self.tab == tab;
            div()
                .id(label)
                .flex_1()
                .h(px(AL_H_SEGMENT_BUTTON))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(AL_R_SEGMENT_BUTTON))
                .cursor_pointer()
                .font_weight(FontWeight::MEDIUM)
                .text_color(hsla(if on { t.text } else { t.text_muted }))
                .when(on, |d| d.bg(hsla(raised)).shadow_xs())
                .when(!on, |d| d.hover(|s| s.text_color(hsla(t.text))))
                .on_click(cx.listener(move |p, _, _, cx| {
                    p.tab = tab;
                    cx.notify();
                }))
                .child(label)
        };
        h_flex()
            .mx(px(AL_PAD_X))
            .mt(px(S3))
            .h(px(AL_H_SEGMENT))
            .p(px((AL_H_SEGMENT - AL_H_SEGMENT_BUTTON) / 2.0))
            .gap(px(S1))
            .rounded(px(AL_R_SEGMENT))
            .bg(hsla(track))
            .child(seg(PanelTab::Alerts, "Alerts", cx))
            .child(seg(PanelTab::Log, "Log", cx))
    }

    fn tool(id: &'static str, g: Glyph, tip: &'static str) -> IconButton {
        IconButton::new(id, g).size(ControlSize::Lg).tooltip(tip)
    }

    fn alerts_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let sort = self.sort;
        let w = this.clone();
        let sort_menu = Self::tool(
            "al-sort",
            Glyph::Lucide(Lucide::ArrowDownWideNarrow),
            "Sort alerts",
        )
        .menu(Anchor::TopRight, move |mut menu, _, _| {
            menu = menu.id("al-sort-menu").width(MenuWidth::Lg);
            for (i, s) in Sort::ALL.iter().enumerate() {
                if i > 0 && i % 2 == 0 {
                    menu = menu.separator();
                }
                let (w, s) = (w.clone(), *s);
                menu = menu.row(
                    MenuRow::new(("al-sort", i), s.label())
                        .checked(s == sort)
                        .on_select(move |_, cx| {
                            w.update(cx, |p, cx| {
                                p.sort = s;
                                cx.notify();
                            })
                            .ok();
                        }),
                );
            }
            menu
        });
        let (show, only_current) = (self.show, self.only_current_symbol);
        let counts = center(cx).map_or((0, 0), |c| {
            let a = c.read(cx).alerts();
            let active = a.iter().filter(|a| a.status.is_active()).count();
            (active, a.len() - active)
        });
        let w = this.clone();
        let options = Self::tool("al-options", Glyph::ELLIPSIS, "Options").menu(
            Anchor::TopRight,
            move |menu, _, _| {
                let set_show = |s: Show| {
                    let w = w.clone();
                    move |_: &mut Window, cx: &mut App| {
                        w.update(cx, |p, cx| {
                            p.show = s;
                            cx.notify();
                        })
                        .ok();
                    }
                };
                let w2 = w.clone();
                menu.id("al-options-menu")
                    .width(MenuWidth::Lg)
                    .row(
                        MenuRow::new("al-restart-all", "Restart all inactive")
                            .icon(Glyph::Lucide(Lucide::RotateCcw))
                            .disabled(counts.1 == 0)
                            .on_select(|_, cx| {
                                Self::with_center(cx, |c, cx| c.restart_all_inactive(cx))
                            }),
                    )
                    .row(
                        MenuRow::new("al-stop-all", "Stop all")
                            .icon(Glyph::Lucide(Lucide::Pause))
                            .disabled(counts.0 == 0)
                            .on_select(|_, cx| Self::with_center(cx, |c, cx| c.stop_all(cx))),
                    )
                    .row(
                        MenuRow::danger("al-delete-inactive", "Delete all inactive")
                            .icon(Glyph::TRASH)
                            .disabled(counts.1 == 0)
                            .on_select(|_, cx| {
                                Self::with_center(cx, |c, cx| c.delete_all_inactive(cx))
                            }),
                    )
                    .section("Show alerts")
                    .row(
                        MenuRow::new("al-show-all", format!("All ({})", counts.0 + counts.1))
                            .checked(show == Show::All)
                            .on_select(set_show(Show::All)),
                    )
                    .row(
                        MenuRow::new("al-show-active", format!("Active only ({})", counts.0))
                            .checked(show == Show::Active)
                            .on_select(set_show(Show::Active)),
                    )
                    .row(
                        MenuRow::new("al-show-inactive", format!("Inactive only ({})", counts.1))
                            .checked(show == Show::Inactive)
                            .on_select(set_show(Show::Inactive)),
                    )
                    .section("Filter alerts")
                    .row(
                        MenuRow::new("al-current", "Current symbol")
                            .toggle(only_current)
                            .keep_open()
                            .on_select(move |_, cx| {
                                w2.update(cx, |p, cx| {
                                    p.only_current_symbol = !p.only_current_symbol;
                                    cx.notify();
                                })
                                .ok();
                            }),
                    )
            },
        );
        let w = this.clone();
        let search = Self::tool("al-search", Glyph::SEARCH, "Search")
            .toggled(self.searching)
            .on_click(move |_, window, cx| {
                w.update(cx, |p, cx| {
                    p.searching = !p.searching;
                    if p.searching {
                        let f = p.query.read(cx).focus_handle(cx);
                        f.focus(window, cx);
                    }
                    cx.notify();
                })
                .ok();
            });
        h_flex()
            .px(px(S2))
            .h(px(AL_TOOL_BUTTON + S2 * 2.0))
            .items_center()
            .child(
                Self::tool("al-create", Glyph::PLUS, "Create alert").on_click(|_, window, cx| {
                    open_create(window, cx);
                }),
            )
            .child(div().flex_1())
            .child(search)
            .child(sort_menu)
            .child(options)
    }

    fn log_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let empty = center(cx).is_none_or(|c| c.read(cx).log().is_empty());
        h_flex()
            .px(px(S2))
            .h(px(AL_TOOL_BUTTON + S2 * 2.0))
            .items_center()
            .child(
                Self::tool("al-clear-log", Glyph::Lucide(Lucide::Brush), "Clear log")
                    .disabled(empty)
                    .on_click(|_, _, cx| Self::with_center(cx, |c, cx| c.clear_log(cx))),
            )
            .child(div().flex_1())
    }

    fn status_color(s: AlertStatus, t: &tt_theme::Theme) -> Hsla {
        hsla(match s {
            AlertStatus::Active => t.up,
            AlertStatus::Triggered => t.active,
            AlertStatus::Stopped | AlertStatus::Expired => t.text_muted,
        })
    }

    fn alert_row(a: &Alert, t: &tt_theme::Theme) -> AnyElement {
        let id = a.id;
        let group = SharedString::from(format!("al-row-{id}"));
        let active = a.status.is_active();
        let mut status = a.status.label().to_owned();
        if let Some(last) = a.last_triggered_ms {
            status.push_str(&format!(" · Last {}", format_local_short(last)));
        }
        if active && let Some(e) = a.expiration {
            status.push_str(&format!(" · Expires {}", format_local_short(e)));
        }
        let edit_alert = a.clone();
        let actions = h_flex()
            .absolute()
            .top(px(S1))
            .right(px(S2))
            .gap(px(S0_5))
            .px(px(S0_5))
            .rounded(px(R_MD))
            .bg(hsla(t.hover))
            .invisible()
            .group_hover(group.clone(), |s| s.visible())
            .child(
                IconButton::new(("al-edit", id), Glyph::PENCIL)
                    .size(ControlSize::Sm)
                    .tooltip("Edit")
                    .on_click(move |_, window, cx| {
                        crate::alert_dialog::AlertDialog::open(
                            edit_alert.clone(),
                            false,
                            window,
                            cx,
                        );
                    }),
            )
            .child(
                IconButton::new(
                    ("al-toggle", id),
                    Glyph::Lucide(if active { Lucide::Pause } else { Lucide::Play }),
                )
                .size(ControlSize::Sm)
                .tooltip(if active { "Stop" } else { "Restart" })
                .on_click(move |_, _, cx| {
                    Self::with_center(cx, |c, cx| c.set_active(id, !active, cx))
                }),
            )
            .child(
                IconButton::new(("al-clone", id), Glyph::COPY)
                    .size(ControlSize::Sm)
                    .tooltip("Clone")
                    .on_click(move |_, _, cx| Self::with_center(cx, |c, cx| c.duplicate(id, cx))),
            )
            .child(
                IconButton::new(("al-delete", id), Glyph::TRASH)
                    .size(ControlSize::Sm)
                    .tooltip("Delete")
                    .on_click(move |_, _, cx| Self::with_center(cx, |c, cx| c.remove(id, cx))),
            );
        let symbol = a.symbol.clone();
        let edit = a.clone();
        v_flex()
            .id(("al-row", id))
            .group(group)
            .relative()
            .px(px(AL_PAD_X))
            .py(px(S2))
            .gap(px(2.0))
            .cursor_pointer()
            .border_b_1()
            .border_color(hsla(t.border))
            .hover(|s| s.bg(hsla(t.hover)))
            .on_click(move |e, window, cx| {
                if e.click_count() >= 2 {
                    crate::alert_dialog::AlertDialog::open(edit.clone(), false, window, cx);
                } else {
                    let s = symbol.clone();
                    Self::with_center(cx, |c, cx| c.show_symbol(s, cx));
                }
            })
            .child(
                h_flex()
                    .gap(px(S2))
                    .items_center()
                    .child(
                        div()
                            .size(px(8.0))
                            .rounded_full()
                            .bg(Self::status_color(a.status, t)),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(a.symbol.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(TEXT_SMALL))
                            .text_color(hsla(t.text_muted))
                            .child(a.interval.label()),
                    ),
            )
            .child(
                div()
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .when(!active, |d| d.text_color(hsla(t.text_muted)))
                    .child(a.title()),
            )
            .child(
                div()
                    .text_size(px(TEXT_SMALL))
                    .text_color(hsla(t.text_muted))
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(a.message.clone()),
            )
            .child(
                div()
                    .text_size(px(TEXT_SMALL))
                    .text_color(Self::status_color(a.status, t))
                    .child(status),
            )
            .child(actions)
            .into_any_element()
    }

    fn log_row(i: usize, e: &LogEntry, t: &tt_theme::Theme) -> AnyElement {
        let symbol = e.symbol.clone();
        let group = SharedString::from(format!("al-log-{i}"));
        v_flex()
            .id(("al-log", i))
            .group(group.clone())
            .relative()
            .px(px(AL_PAD_X))
            .py(px(S2))
            .gap(px(2.0))
            .cursor_pointer()
            .border_b_1()
            .border_color(hsla(t.border))
            .hover(|s| s.bg(hsla(t.hover)))
            .on_click(move |_, _, cx| {
                let s = symbol.clone();
                Self::with_center(cx, |c, cx| c.show_symbol(s, cx));
            })
            .child(
                h_flex()
                    .gap(px(S2))
                    .items_center()
                    .child(glyph(Glyph::ALARM, ICON_SM).color(hsla(t.active)))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(e.symbol.clone()),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(TEXT_SMALL))
                            .text_color(hsla(t.text_muted))
                            .child(format_local_short(e.time_ms)),
                    ),
            )
            .child(
                div()
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(e.title.clone()),
            )
            .child(
                div()
                    .text_size(px(TEXT_SMALL))
                    .text_color(hsla(t.text_muted))
                    .child(e.message.clone()),
            )
            .child(
                h_flex()
                    .absolute()
                    .top(px(S1))
                    .right(px(S2))
                    .invisible()
                    .group_hover(group, |s| s.visible())
                    .child(
                        IconButton::new(("al-log-del", i), Glyph::X)
                            .size(ControlSize::Sm)
                            .tooltip("Remove")
                            .on_click(move |_, _, cx| {
                                Self::with_center(cx, |c, cx| c.remove_log_entry(i, cx))
                            }),
                    ),
            )
            .into_any_element()
    }

    /// TradingView's empty states: an illustration, a line of text and
    /// (Alerts) a "Create alert" button.
    fn empty(alerts: bool, t: &tt_theme::Theme) -> AnyElement {
        let (icon, text) = if alerts {
            (
                Glyph::ALARM_PLUS,
                "Alerts notify you instantly when your conditions are met. Create one to get started.",
            )
        } else {
            (
                Glyph::ALARM,
                "No alerts triggered yet! You will see a list here when they do.",
            )
        };
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .px(px(S5))
            .gap(px(S4))
            .child(glyph(icon, AL_EMPTY_ICON * 0.6).color(hsla(t.text)))
            .child(
                div()
                    .max_w(px(AL_W_EMPTY))
                    .text_center()
                    .text_size(px(AL_EMPTY_TEXT))
                    .line_height(px(AL_EMPTY_TEXT * 1.5))
                    .child(text),
            )
            .when(alerts, |d| {
                d.child(
                    div()
                        .id("al-empty-create")
                        .h(px(C_CONTROL))
                        .px(px(S4))
                        .flex()
                        .items_center()
                        .rounded(px(C_R_CONTROL))
                        .bg(hsla(t.text))
                        .text_color(hsla(t.bg))
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.85))
                        .on_click(|_, window, cx| open_create(window, cx))
                        .child("Create alert"),
                )
            })
            .into_any_element()
    }
}

/// "Create alert" from the panel: on the active chart's symbol and price.
fn open_create(window: &mut Window, cx: &mut App) {
    let draft = crate::alerts::new_draft(None, cx);
    crate::alert_dialog::AlertDialog::open(draft, true, window, cx);
}

impl Render for AlertsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.observe_center(cx);
        let t = cx.ui().clone();
        let body: AnyElement = match self.tab {
            PanelTab::Alerts => {
                let alerts = self.visible_alerts(cx);
                let total = center(cx).map_or(0, |c| c.read(cx).alerts().len());
                let toolbar = self.alerts_toolbar(cx).into_any_element();
                let list = if total == 0 {
                    Self::empty(true, &t)
                } else if alerts.is_empty() {
                    EmptyState::new("No alerts match the filters.").into_any_element()
                } else {
                    let rows: Vec<AnyElement> =
                        alerts.iter().map(|a| Self::alert_row(a, &t)).collect();
                    v_flex()
                        .id("al-list")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .children(rows)
                        .into_any_element()
                };
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .child(toolbar)
                    .when(self.searching, |d| {
                        d.child(
                            div()
                                .px(px(AL_PAD_X))
                                .pb(px(S2))
                                .child(SearchField::boxed(&self.query)),
                        )
                    })
                    .child(list)
                    .into_any_element()
            }
            PanelTab::Log => {
                let log: Vec<LogEntry> = center(cx)
                    .map(|c| c.read(cx).log().to_vec())
                    .unwrap_or_default();
                let toolbar = self.log_toolbar(cx).into_any_element();
                let list = if log.is_empty() {
                    Self::empty(false, &t)
                } else {
                    // Newest first.
                    let rows: Vec<AnyElement> = log
                        .iter()
                        .enumerate()
                        .rev()
                        .map(|(i, e)| Self::log_row(i, e, &t))
                        .collect();
                    v_flex()
                        .id("al-log-list")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .children(rows)
                        .into_any_element()
                };
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .child(toolbar)
                    .child(list)
                    .into_any_element()
            }
        };
        v_flex()
            .id("alerts-panel")
            .size_full()
            .min_w_0()
            .bg(hsla(t.panel))
            .text_color(hsla(t.text))
            .text_size(px(TEXT_BODY))
            .child(self.segmented(cx))
            .child(body)
    }
}
