//! TradingView's watchlist panel (docs/specs/right-panel.md §3): the list
//! name with its menu (create, rename, copy, delete lists, add section,
//! clear, switch), "+" to add a symbol (our symbol search), "…" for the
//! columns, a sortable column header, 30 px rows (logo, ticker, last,
//! change, change %, volume) with sections, drag to reorder, a context
//! menu, and the selected symbol's details under the list.
//!
//! Prices: Binance 24 h mini tickers for the current list
//! ([`tt_session::quotes::QuoteBoard`], streamed only while the panel is
//! open), and the last close of any tab showing the symbol (the tab's own
//! stream ticks faster). The view is rendered cached and woken at most
//! every 150 ms by new quotes, and by the app on tab ticks.

use crate::app::TradeTerm;
use crate::data_window::{signed, signed_pct, tv_minus, volume_text};
use crate::name_prompt::{PromptOpts, Submit};
use crate::search::SymbolSearch;
use crate::ui::badge_color;
use futures::StreamExt;
use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::component::menu::PopupMenuItem;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tt_chart::scale::format_price;
use tt_core::{Symbol, SymbolInfo};
use tt_session::assets::AssetCache;
use tt_session::quotes::{Quote, QuoteBoard};
use tt_session::tab::Shared;
use tt_session::watchlist::{Sort, SortKey, WatchItem, Watchlists};
use tt_ui::menu::MenuWidth;
use tt_ui::prelude::*;
use tt_ui::tokens::*;
use tt_ui::{IconButton, Menu, ToolbarButton, hsla};

/// One symbol's numbers for a row.
#[derive(Debug, Clone, Copy, Default)]
struct RowValues {
    last: Option<f64>,
    change: Option<f64>,
    change_pct: Option<f64>,
    volume: Option<f64>,
}

impl RowValues {
    fn get(&self, key: SortKey) -> Option<f64> {
        match key {
            SortKey::Symbol => None,
            SortKey::Last => self.last,
            SortKey::Change => self.change,
            SortKey::ChangePct => self.change_pct,
            SortKey::Volume => self.volume,
        }
    }
}

/// Row values from the 24 h quote, with the last price of a tab showing
/// the symbol when there is one.
fn row_values(q: Option<&Quote>, tab_last: Option<f64>) -> RowValues {
    let last = tab_last.or(q.map(|q| q.last));
    let open = q.map(|q| q.open).filter(|o| *o != 0.0);
    RowValues {
        last,
        change: last.zip(open).map(|(l, o)| l - o),
        change_pct: last.zip(open).map(|(l, o)| (l / o - 1.0) * 100.0),
        volume: q.map(|q| q.volume),
    }
}

/// Decimals for a price in the list (the symbol's tick size when a tab
/// knows it).
fn decimals(price: f64, known: Option<usize>) -> usize {
    known.unwrap_or_else(|| tt_chart::scale::price_decimals(price))
}

/// Drag payload of a row.
#[derive(Clone)]
struct DraggedRow {
    index: usize,
    label: SharedString,
}

impl Render for DraggedRow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ui();
        div()
            .h(H_WL_ROW.px())
            .px(S2.px())
            .flex()
            .items_center()
            .rounded(R_MD.px())
            .bg(hsla(t.overlay_bg))
            .border_1()
            .border_color(hsla(t.border))
            .text_size(TEXT_BODY.px())
            .text_color(hsla(t.text))
            .child(self.label.clone())
    }
}

pub struct WatchlistPanel {
    app: WeakEntity<TradeTerm>,
    pub(crate) lists: Watchlists,
    quotes: QuoteBoard,
    assets: AssetCache,
    /// Row whose details show (symbol); the chart's symbol when `None`.
    selected: Option<String>,
    live: bool,
    _wake: Task<()>,
}

impl WatchlistPanel {
    pub fn new(
        app: WeakEntity<TradeTerm>,
        shared: &Shared,
        lists: Watchlists,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<()>();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            let _ = tx.unbounded_send(());
        });
        // New quotes → one repaint, at most every 150 ms.
        let task = cx.spawn(async move |this, cx| {
            while rx.next().await.is_some() {
                while rx.try_recv().is_ok() {}
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
            }
        });
        Self {
            app,
            lists: lists.normalized(),
            quotes: QuoteBoard::new(shared.handle.clone(), shared.source.clone(), wake),
            assets: shared.assets.clone(),
            selected: None,
            live: false,
            _wake: task,
        }
    }

    /// Streams quotes while the panel is shown.
    pub fn set_live(&mut self, live: bool) {
        self.live = live;
        self.sync_quotes();
    }

    fn sync_quotes(&mut self) {
        let symbols = if self.live {
            self.lists.current().symbols().map(str::to_owned).collect()
        } else {
            Vec::new()
        };
        self.quotes.set_symbols(symbols);
    }

    /// After an edit: restream if needed, repaint, save.
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.sync_quotes();
        cx.notify();
        let app = self.app.clone();
        cx.defer(move |cx| {
            app.update(cx, |a, cx| a.persist(cx)).ok();
        });
    }

    fn edit(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Watchlists)) {
        f(&mut self.lists);
        self.changed(cx);
    }

    fn add_symbol(&mut self, info: SymbolInfo, cx: &mut Context<Self>) {
        let sym = info.symbol.to_string();
        let item = WatchItem::symbol(&sym, &info.base, &info.quote);
        // After the selected row (TradingView), else at the end.
        let after = self.selected.as_ref().and_then(|s| {
            self.lists
                .current()
                .items
                .iter()
                .position(|i| i.symbol_str() == Some(s))
        });
        if self.lists.current_mut().add_symbol(item, after) {
            self.selected = Some(sym);
            self.changed(cx);
        }
    }

    /// "+": the symbol search, picking adds to the list.
    fn open_add_symbol(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let a = app.read(cx);
        let (symbols, handle) = (a.shared.symbols.clone(), a.shared.handle.clone());
        let assets = a.shared.assets.clone();
        let recent = crate::search::recent_rows(&a.recent);
        let quote = a.search_quote;
        let this = cx.entity().downgrade();
        let weak_app = self.app.clone();
        let panel = cx.new(|cx| {
            let mut s = SymbolSearch::new(
                weak_app, symbols, handle, assets, recent, "", quote, false, window, cx,
            );
            s.set_on_pick(move |info, cx| {
                this.update(cx, |p, cx| p.add_symbol(info, cx)).ok();
            });
            s
        });
        let focus = panel.clone();
        let opts = tt_ui::DialogOpts::new("Add symbol", tt_ui::DialogSize::Md)
            .bare()
            .width(SS_W_DIALOG)
            .on_open(move |window, cx| SymbolSearch::focus_input(&focus, window, cx));
        tt_ui::open_dialog(window, cx, opts, panel);
    }

    /// Opens `symbol` on the active chart (`new_tab`: in a new tab).
    fn open_symbol(
        app: &WeakEntity<TradeTerm>,
        symbol: &str,
        new_tab: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        let sym = Symbol::new(symbol);
        app.update(cx, |a, cx| {
            let info = a
                .shared
                .symbols
                .get()
                .and_then(|l| l.iter().find(|s| s.symbol == sym).cloned());
            if new_tab {
                a.new_tab(window, cx);
            }
            a.set_symbol(sym, info, cx);
        })
        .ok();
    }

    fn prompt_name(
        &self,
        title: &'static str,
        ok: &'static str,
        initial: String,
        window: &mut Window,
        cx: &mut Context<Self>,
        apply: impl Fn(&mut Watchlists, String) + 'static,
    ) {
        let this = cx.entity().downgrade();
        crate::name_prompt::open(
            window,
            cx,
            PromptOpts {
                title,
                label: "Name",
                initial,
                ok,
                checks: Vec::new(),
            },
            move |name, _, _, _, cx| {
                let name = name.trim().to_owned();
                if name.is_empty() {
                    return Submit::Error("Enter a name".into());
                }
                this.update(cx, |p, cx| p.edit(cx, |w| apply(w, name))).ok();
                Submit::Close
            },
        );
    }

    // ---- header ----------------------------------------------------------

    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let name = self.lists.current().name.clone();
        let list_menu = {
            let this = this.clone();
            move |menu: Menu, _: &mut Window, cx: &mut App| -> Menu {
                let Some(p) = this.upgrade() else {
                    return menu;
                };
                let lists = &p.read(cx).lists;
                let can_delete = lists.lists.len() > 1;
                let names: Vec<(usize, String)> = lists
                    .lists
                    .iter()
                    .enumerate()
                    .map(|(i, l)| (i, l.name.clone()))
                    .collect();
                let active = lists.active;
                let current = lists.current().name.clone();
                let act = |id: &'static str, label: &'static str| MenuRow::new(id, label);
                let w = this.clone();
                let mut m = menu
                    .id("wl-list-menu")
                    .width(MenuWidth::Md)
                    .row(act("wl-new", "Create new list…").on_select({
                        let w = w.clone();
                        move |window, cx| {
                            w.update(cx, |p, cx| {
                                p.prompt_name(
                                    "Create new list",
                                    "Create",
                                    String::new(),
                                    window,
                                    cx,
                                    |l, n| l.create(&n),
                                )
                            })
                            .ok();
                        }
                    }))
                    .row(act("wl-rename", "Rename…").on_select({
                        let (w, current) = (w.clone(), current.clone());
                        move |window, cx| {
                            let current = current.clone();
                            w.update(cx, |p, cx| {
                                p.prompt_name(
                                    "Rename list",
                                    "Rename",
                                    current,
                                    window,
                                    cx,
                                    |l, n| {
                                        l.rename(&n);
                                    },
                                )
                            })
                            .ok();
                        }
                    }))
                    .row(act("wl-copy", "Make a copy").on_select({
                        let w = w.clone();
                        move |_, cx| {
                            w.update(cx, |p, cx| p.edit(cx, |l| l.duplicate())).ok();
                        }
                    }))
                    .row(act("wl-section", "Add section…").on_select({
                        let w = w.clone();
                        move |window, cx| {
                            w.update(cx, |p, cx| {
                                p.prompt_name(
                                    "Add section",
                                    "Add",
                                    String::new(),
                                    window,
                                    cx,
                                    |l, n| l.current_mut().add_section(None, &n),
                                )
                            })
                            .ok();
                        }
                    }))
                    .row(act("wl-clear", "Clear list").on_select({
                        let w = w.clone();
                        move |_, cx| {
                            w.update(cx, |p, cx| p.edit(cx, |l| l.current_mut().items.clear()))
                                .ok();
                        }
                    }))
                    .row(
                        MenuRow::danger("wl-delete", "Delete list")
                            .disabled(!can_delete)
                            .on_select({
                                let w = w.clone();
                                move |_, cx| {
                                    w.update(cx, |p, cx| {
                                        p.edit(cx, |l| {
                                            l.delete_current();
                                        })
                                    })
                                    .ok();
                                }
                            }),
                    )
                    .separator()
                    .section("Watchlists");
                for (i, n) in names {
                    let w = w.clone();
                    m = m.row(
                        MenuRow::new(SharedString::from(format!("wl-open-{i}")), n)
                            .checked(i == active)
                            .on_select(move |_, cx| {
                                w.update(cx, |p, cx| p.edit(cx, |l| l.active = i)).ok();
                            }),
                    );
                }
                m
            }
        };
        let settings_menu = move |menu: Menu, _: &mut Window, cx: &mut App| -> Menu {
            let Some(p) = this.upgrade() else {
                return menu;
            };
            let c = p.read(cx).lists.columns;
            let toggle = |id: &'static str,
                          label: &'static str,
                          on: bool,
                          f: fn(&mut tt_session::watchlist::Columns)| {
                let w = this.clone();
                MenuRow::new(id, label)
                    .toggle(on)
                    .keep_open()
                    .on_select(move |_, cx| {
                        w.update(cx, |p, cx| p.edit(cx, |l| f(&mut l.columns))).ok();
                    })
            };
            menu.id("wl-settings-menu")
                .width(MenuWidth::Md)
                .section("Customize columns")
                .row(toggle("wl-col-last", "Last", c.last, |c| c.last = !c.last))
                .row(toggle("wl-col-chg", "Change", c.change, |c| {
                    c.change = !c.change
                }))
                .row(toggle("wl-col-pct", "Change %", c.change_pct, |c| {
                    c.change_pct = !c.change_pct
                }))
                .row(toggle("wl-col-vol", "Volume", c.volume, |c| {
                    c.volume = !c.volume
                }))
                .section("Symbol display")
                .row(toggle("wl-col-logo", "Logo", c.logo, |c| c.logo = !c.logo))
                .row(toggle("wl-col-ticker", "Symbol", !c.name, |c| {
                    c.name = false
                }))
                .row(toggle("wl-col-name", "Name", c.name, |c| c.name = true))
        };
        h_flex()
            .flex_none()
            .h(H_PANEL_HEADER.px())
            .pl(S2.px())
            .pr(S1.px())
            .gap(S0_5.px())
            .child(
                ToolbarButton::text("wl-name", name)
                    .caret()
                    .tooltip("Watchlist menu")
                    .menu(list_menu),
            )
            .child(div().flex_1())
            .child(
                IconButton::new("wl-add", Glyph::PLUS)
                    .size(ControlSize::Xl)
                    .tooltip("Add symbol")
                    .on_click(cx.listener(|this, _, window, cx| this.open_add_symbol(window, cx))),
            )
            .child(
                IconButton::new("wl-settings", Glyph::ELLIPSIS)
                    .size(ControlSize::Xl)
                    .tooltip("Settings")
                    .menu(Anchor::TopRight, settings_menu),
            )
    }

    fn column_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ui().clone();
        let c = self.lists.columns;
        let sort = self.lists.sort;
        let cell =
            |key: SortKey, label: &'static str, width: Option<f32>, cx: &mut Context<Self>| {
                let arrow = sort
                    .filter(|s| s.key == key)
                    .map(|s| if s.descending { " ↓" } else { " ↑" })
                    .unwrap_or("");
                div()
                    .id(SharedString::from(format!("wl-h-{key:?}")))
                    .h_full()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .hover(|s| s.text_color(hsla(t.text)))
                    .map(|d| match width {
                        Some(w) => d.w(w.px()).flex_none().justify_end().pr(S2.px()),
                        None => d.flex_1().min_w_0().pl(S2.px()),
                    })
                    .child(format!("{label}{arrow}"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let s = Sort::cycle(this.lists.sort, key);
                        this.edit(cx, |l| l.sort = s);
                    }))
            };
        h_flex()
            .flex_none()
            .h(H_WL_COLUMNS.px())
            .px(S1.px())
            .text_size(TEXT_BODY.px())
            .text_color(hsla(t.text_muted))
            .border_b_1()
            .border_color(hsla(t.border))
            .child(cell(SortKey::Symbol, "Symbol", None, cx))
            .when(c.last, |d| {
                d.child(cell(SortKey::Last, "Last", Some(W_WL_LAST), cx))
            })
            .when(c.change, |d| {
                d.child(cell(SortKey::Change, "Chg", Some(W_WL_CHG), cx))
            })
            .when(c.change_pct, |d| {
                d.child(cell(SortKey::ChangePct, "Chg%", Some(W_WL_CHG_PCT), cx))
            })
            .when(c.volume, |d| {
                d.child(cell(SortKey::Volume, "Vol", Some(W_WL_VOL), cx))
            })
    }

    // ---- rows ------------------------------------------------------------

    fn symbol_logo(&self, base: &str, symbol: &str, t: &tt_theme::Theme) -> AnyElement {
        let key = if base.is_empty() { symbol } else { base };
        crate::search::logo(
            &self.assets,
            key,
            LOGO_SM,
            hsla(badge_color(t, key)),
            hsla(t.text_on_accent),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn symbol_row(
        &self,
        index: usize,
        symbol: &str,
        base: &str,
        quote: &str,
        v: RowValues,
        precision: Option<usize>,
        active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = cx.ui().clone();
        let c = self.lists.columns;
        let selected = self.selected.as_deref() == Some(symbol);
        let dir = |x: Option<f64>| match x {
            Some(x) if x > 0.0 => hsla(t.up),
            Some(x) if x < 0.0 => hsla(t.down),
            _ => hsla(t.text),
        };
        let d = decimals(v.last.unwrap_or(0.0), precision);
        let num = |text: String, w: f32, color: Hsla| {
            div()
                .w(w.px())
                .flex_none()
                .flex()
                .justify_end()
                .pr(S2.px())
                .overflow_hidden()
                .whitespace_nowrap()
                .text_color(color)
                .child(text)
        };
        let label: SharedString = if c.name {
            self.assets
                .pair_name(base, quote)
                .unwrap_or_else(|| symbol.to_owned())
                .into()
        } else {
            symbol.to_owned().into()
        };
        let sym = symbol.to_owned();
        let app = self.app.clone();
        let this = cx.entity().downgrade();
        let can_drag = self.lists.sort.is_none();
        let accent = hsla(t.active);
        h_flex()
            .id(SharedString::from(format!("wl-row-{symbol}")))
            .flex_none()
            .h(H_WL_ROW.px())
            .px(S1.px())
            .text_size(TEXT_BODY.px())
            .text_color(hsla(t.text))
            .cursor_pointer()
            .when(selected, |d| d.bg(hsla(t.selection)))
            .when(!selected, |d| d.hover(|s| s.bg(hsla(t.hover))))
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .pl(S2.px())
                    .gap(S1_5.px())
                    .when(c.logo, |d| d.child(self.symbol_logo(base, symbol, &t)))
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .when(active, |d| d.font_weight(FontWeight::SEMIBOLD))
                            .child(label.clone()),
                    ),
            )
            .when(c.last, |r| {
                r.child(num(
                    v.last.map_or(String::new(), |x| format_price(x, d)),
                    W_WL_LAST,
                    hsla(t.text),
                ))
            })
            .when(c.change, |r| {
                r.child(num(
                    v.change.map_or(String::new(), |x| signed(x, d)),
                    W_WL_CHG,
                    dir(v.change),
                ))
            })
            .when(c.change_pct, |r| {
                r.child(num(
                    v.change_pct.map_or(String::new(), signed_pct),
                    W_WL_CHG_PCT,
                    dir(v.change_pct),
                ))
            })
            .when(c.volume, |r| {
                r.child(num(
                    v.volume.map_or(String::new(), volume_text),
                    W_WL_VOL,
                    hsla(t.text),
                ))
            })
            .on_click(cx.listener({
                let sym = sym.clone();
                move |this, _, window, cx| {
                    this.selected = Some(sym.clone());
                    cx.notify();
                    Self::open_symbol(&this.app, &sym, false, window, cx);
                }
            }))
            .when(can_drag, |d| {
                d.on_drag(
                    DraggedRow {
                        index,
                        label: label.clone(),
                    },
                    |r, _, _, cx| cx.new(|_| r.clone()),
                )
            })
            .drag_over::<DraggedRow>(move |s, _, _, _| s.border_t_2().border_color(accent))
            .on_drop(cx.listener(move |this, r: &DraggedRow, _, cx| {
                let from = r.index;
                this.edit(cx, |l| {
                    l.current_mut().move_item(from, index);
                });
            }))
            .context_menu(move |menu, _, _| {
                let (app2, sym2) = (app.clone(), sym.clone());
                let (w1, w2) = (this.clone(), this.clone());
                menu.item(
                    PopupMenuItem::new("Open in new tab").on_click(move |_, window, cx| {
                        Self::open_symbol(&app2, &sym2, true, window, cx);
                    }),
                )
                .item(
                    PopupMenuItem::new("Add section above…").on_click(move |_, window, cx| {
                        w1.update(cx, |p, cx| {
                            p.prompt_name(
                                "Add section",
                                "Add",
                                String::new(),
                                window,
                                cx,
                                move |l, n| l.current_mut().add_section(Some(index), &n),
                            )
                        })
                        .ok();
                    }),
                )
                .separator()
                .item(
                    PopupMenuItem::new("Remove from watchlist").on_click(move |_, _, cx| {
                        w2.update(cx, |p, cx| {
                            p.edit(cx, |l| {
                                l.current_mut().remove(index);
                            })
                        })
                        .ok();
                    }),
                )
            })
            .into_any_element()
    }

    fn section_row(
        &self,
        index: usize,
        title: &str,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = cx.ui().clone();
        let accent = hsla(t.active);
        let this = cx.entity().downgrade();
        let name = title.to_owned();
        h_flex()
            .id(SharedString::from(format!("wl-sec-{index}")))
            .flex_none()
            .h(H_WL_SECTION.px())
            .px(S2.px())
            .gap(S1.px())
            .cursor_pointer()
            .text_size(TEXT_CAPTION.px())
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(hsla(t.text_muted))
            .hover(|s| s.bg(hsla(t.hover)))
            .child(glyph(
                if collapsed {
                    Glyph::CHEVRON_RIGHT
                } else {
                    Glyph::CHEVRON_DOWN
                },
                ICON_XS,
            ))
            .child(title.to_uppercase())
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                if e.click_count() >= 2 {
                    let name = this
                        .lists
                        .current()
                        .items
                        .get(index)
                        .and_then(|i| match i {
                            WatchItem::Section { section, .. } => Some(section.clone()),
                            _ => None,
                        })
                        .unwrap_or_default();
                    this.prompt_name("Rename section", "Rename", name, window, cx, move |l, n| {
                        if let Some(WatchItem::Section { section, .. }) =
                            l.current_mut().items.get_mut(index)
                        {
                            *section = n;
                        }
                    });
                } else {
                    this.edit(cx, |l| l.current_mut().toggle_section(index));
                }
            }))
            .on_drag(
                DraggedRow {
                    index,
                    label: name.to_uppercase().into(),
                },
                |r, _, _, cx| cx.new(|_| r.clone()),
            )
            .drag_over::<DraggedRow>(move |s, _, _, _| s.border_t_2().border_color(accent))
            .on_drop(cx.listener(move |this, r: &DraggedRow, _, cx| {
                let from = r.index;
                this.edit(cx, |l| {
                    l.current_mut().move_item(from, index);
                });
            }))
            .context_menu(move |menu, _, _| {
                let w = this.clone();
                menu.item(
                    PopupMenuItem::new("Remove section").on_click(move |_, _, cx| {
                        w.update(cx, |p, cx| {
                            p.edit(cx, |l| {
                                l.current_mut().remove(index);
                            })
                        })
                        .ok();
                    }),
                )
            })
            .into_any_element()
    }

    /// The selected symbol's details (TradingView's lower half).
    #[allow(clippy::too_many_arguments)]
    fn details(
        &self,
        symbol: &str,
        base: &str,
        quote: &str,
        q: Option<Quote>,
        v: RowValues,
        precision: Option<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = cx.ui().clone();
        let d = decimals(v.last.unwrap_or(0.0), precision);
        let dir = match v.change {
            Some(x) if x < 0.0 => hsla(t.down),
            Some(x) if x > 0.0 => hsla(t.up),
            _ => hsla(t.text),
        };
        let name = self
            .assets
            .pair_name(base, quote)
            .unwrap_or_else(|| format!("{base} / {quote}"));
        let stat = |label: &'static str, value: String| {
            v_flex()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(TEXT_SMALL.px())
                        .text_color(hsla(t.text_muted))
                        .child(label),
                )
                .child(div().child(value))
        };
        let fmt = |x: Option<f64>| x.map_or("—".to_owned(), |x| format_price(x, d));
        v_flex()
            .id("wl-details")
            .flex_none()
            .border_t_1()
            .border_color(hsla(t.border))
            .px(S4.px())
            .py(S3.px())
            .gap(S1.px())
            .text_size(TEXT_BODY.px())
            .text_color(hsla(t.text))
            .child(
                h_flex()
                    .gap(S2.px())
                    .child(self.symbol_logo(base, symbol, &t))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(symbol.to_owned()),
                    ),
            )
            .child(
                div()
                    .text_size(TEXT_SMALL.px())
                    .text_color(hsla(t.text_muted))
                    .child(format!("{name} · Binance · Spot")),
            )
            .child(
                h_flex()
                    .items_baseline()
                    .gap(S1.px())
                    .child(
                        div()
                            .text_size(TEXT_PRICE_LG.px())
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(v.last.map_or("—".to_owned(), |x| format_price(x, d))),
                    )
                    .child(
                        div()
                            .text_size(TEXT_SMALL.px())
                            .text_color(hsla(t.text_muted))
                            .child(quote.to_owned()),
                    ),
            )
            .child(div().text_color(dir).child(match (v.change, v.change_pct) {
                (Some(c), Some(p)) => format!("{}  {}", signed(c, d), signed_pct(p)),
                _ => String::new(),
            }))
            .child(
                h_flex()
                    .mt(S2.px())
                    .gap(S2.px())
                    .child(stat("24h high", fmt(q.map(|q| q.high))))
                    .child(stat("24h low", fmt(q.map(|q| q.low)))),
            )
            .child(
                h_flex()
                    .gap(S2.px())
                    .child(stat(
                        "Volume",
                        q.map_or("—".to_owned(), |q| tv_minus(&volume_text(q.volume))),
                    ))
                    .child(stat(
                        "Volume (quote)",
                        q.map_or("—".to_owned(), |q| volume_text(q.quote_volume)),
                    )),
            )
            .into_any_element()
    }
}

impl Render for WatchlistPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // What the tabs know: last close (when live) and precision.
        let mut tab_last: HashMap<String, (f64, usize)> = HashMap::new();
        let mut active_symbol = None;
        if let Some(app) = self.app.upgrade() {
            let a = app.read(cx);
            for (i, chart) in a.tabs.iter().enumerate() {
                let tab = &chart.read(cx).state.read(cx).source.tab;
                if tab.synthetic.is_some() {
                    continue;
                }
                let sym = tab.symbol.to_string();
                if i == a.active {
                    active_symbol = Some(sym.clone());
                }
                if let Some(c) = tab.series.last() {
                    tab_last.insert(sym, (c.close, tab.price_precision()));
                }
            }
        }
        let list = self.lists.current().clone();
        let values: HashMap<String, (RowValues, Option<Quote>)> = self.quotes.with(|m| {
            list.symbols()
                .map(|s| {
                    let q = m.get(s).copied();
                    let v = row_values(q.as_ref(), tab_last.get(s).map(|x| x.0));
                    (s.to_owned(), (v, q))
                })
                .collect()
        });
        let rows_idx = list.visible_rows(self.lists.sort, |s, k| {
            values.get(s).and_then(|(v, _)| v.get(k))
        });
        let mut rows: Vec<AnyElement> = Vec::with_capacity(rows_idx.len());
        for i in rows_idx {
            match &list.items[i] {
                WatchItem::Symbol {
                    symbol,
                    base,
                    quote,
                } => {
                    let v = values.get(symbol).map(|x| x.0).unwrap_or_default();
                    let prec = tab_last.get(symbol).map(|x| x.1);
                    let active = active_symbol.as_deref() == Some(symbol);
                    rows.push(self.symbol_row(i, symbol, base, quote, v, prec, active, cx));
                }
                WatchItem::Section { section, collapsed } => {
                    rows.push(self.section_row(i, section, *collapsed, cx));
                }
            }
        }
        let empty = rows.is_empty();
        // Details: the selected row, else the chart's symbol if listed.
        let shown = self
            .selected
            .clone()
            .filter(|s| list.contains(s))
            .or(active_symbol.filter(|s| list.contains(s)));
        let details = shown.and_then(|s| {
            list.items.iter().find_map(|i| match i {
                WatchItem::Symbol {
                    symbol,
                    base,
                    quote,
                } if *symbol == s => Some((symbol.clone(), base.clone(), quote.clone())),
                _ => None,
            })
        });
        let t = cx.ui().clone();
        v_flex()
            .id("watchlist-panel")
            .size_full()
            .text_color(hsla(t.text))
            .child(self.header(cx))
            .child(self.column_header(cx))
            .child(
                v_flex()
                    .id("wl-rows")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(rows)
                    .when(empty, |d| {
                        d.child(EmptyState::new("Add symbols with the + button"))
                    }),
            )
            .children(details.map(|(s, b, q)| {
                let (v, quote) = values.get(&s).copied().unwrap_or_default();
                let prec = tab_last.get(&s).map(|x| x.1);
                self.details(&s, &b, &q, quote, v, prec, cx)
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn row_values_prefer_the_tab_price() {
        let q = Quote {
            last: 110.0,
            open: 100.0,
            high: 120.0,
            low: 90.0,
            volume: 5.0,
            quote_volume: 500.0,
        };
        let v = row_values(Some(&q), None);
        assert_eq!(v.last, Some(110.0));
        assert_eq!(v.change, Some(10.0));
        assert!((v.change_pct.unwrap() - 10.0).abs() < 1e-9);
        let v = row_values(Some(&q), Some(120.0));
        assert_eq!(v.last, Some(120.0));
        assert_eq!(v.change, Some(20.0));
        let v = row_values(None, Some(5.0));
        assert_eq!((v.last, v.change, v.volume), (Some(5.0), None, None));
    }
}
