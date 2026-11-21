//! Named layouts on the app bar (TradingView's "Manage layouts", spec S12;
//! storage in `tt_session::layouts`): the layout name with its menu, a
//! "Save" button while autosave is off and there are unsaved changes, the
//! "Open layout…" dialog, layout switching and "Download chart data…".

use crate::app::TradeTerm;
use crate::name_prompt::{self, PromptOpts, Submit};
use crate::top_toolbar::SaveLayout;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{WindowExt as _, h_flex, v_flex};
use gpui_kit::*;
use tt_session::layouts::{Layout, RECENT_LAYOUTS};
use tt_session::settings::{AppSettings, TabSettings};
use tt_session::templates::NameError;
use tt_ui::hsla;
use tt_ui::prelude::*;
use tt_ui::tokens::*;

fn name_error(e: NameError) -> String {
    match e {
        NameError::Exists => "A layout with this name already exists.".into(),
        NameError::NotFound => "No layout with this name.".into(),
        e => e.message().into(),
    }
}

/// The layout name button (menu) and, when there are unsaved changes, Save.
pub fn layout_menu(app: &TradeTerm, cx: &mut Context<TradeTerm>) -> impl IntoElement + use<> {
    let weak = cx.entity().downgrade();
    let unsaved = !app.layout_autosave && app.layout_dirty;
    h_flex()
        .gap(S0_5.px())
        .items_center()
        .child(
            ToolbarButton::text("layout", app.layout_name.clone())
                .caret()
                .tooltip(if unsaved {
                    "Manage layouts (unsaved changes)"
                } else {
                    "Manage layouts"
                })
                .menu_right()
                .menu(move |menu, window, cx| build_menu(menu, &weak, window, cx)),
        )
        .children(unsaved.then(|| {
            ToolbarButton::text("layout-save", "Save")
                .leading(Glyph::SAVE)
                .tooltip_action("Save layout", &SaveLayout, None)
                .on_click(cx.listener(|this, _, window, cx| this.save_layout(window, cx)))
        }))
}

fn build_menu(
    menu: tt_ui::Menu,
    app: &WeakEntity<TradeTerm>,
    window: &mut Window,
    cx: &App,
) -> tt_ui::Menu {
    let Some(a) = app.upgrade() else {
        return menu;
    };
    let a = a.read(cx);
    let (autosave, current) = (a.layout_autosave, a.layout_name.clone());
    let recent: Vec<(String, String)> = a
        .layouts
        .recent(&current, RECENT_LAYOUTS)
        .into_iter()
        .map(|l| (l.name.clone(), l.summary()))
        .collect();
    let act = |f: fn(&mut TradeTerm, &mut Window, &mut Context<TradeTerm>)| {
        let app = app.clone();
        move |window: &mut Window, cx: &mut App| {
            app.update(cx, |a, cx| f(a, window, cx)).ok();
        }
    };
    let mut menu = menu
        .id("layout-menu")
        .width(MenuWidth::Lg)
        .row(
            MenuRow::new("layout-save", "Save layout")
                .icon(Glyph::SAVE)
                .shortcut_action(&SaveLayout, None, window)
                .on_select(act(TradeTerm::save_layout)),
        )
        .row(
            MenuRow::new("layout-autosave", "Autosave")
                .checked(autosave)
                .on_select(act(|a, _, cx| {
                    a.set_layout_autosave(!a.layout_autosave, cx)
                })),
        )
        .separator()
        .row(
            MenuRow::new("layout-copy", "Make a copy…")
                .icon(Glyph::COPY)
                .on_select(act(TradeTerm::copy_layout)),
        )
        .row(
            MenuRow::new("layout-rename", "Rename…")
                .icon(Glyph::RENAME)
                .on_select(act(TradeTerm::rename_layout)),
        )
        .row(
            MenuRow::new("layout-data", "Download chart data…")
                .icon(Glyph::FILE_DOWN)
                .on_select(act(TradeTerm::download_chart_data)),
        )
        .row(
            MenuRow::new("layout-new", "Create new layout…")
                .icon(Glyph::FILE_PLUS)
                .on_select(act(TradeTerm::new_layout)),
        );
    if !recent.is_empty() {
        menu = menu.separator().section("Recently used");
        for (i, (name, summary)) in recent.into_iter().enumerate() {
            let app = app.clone();
            menu = menu.row(
                MenuRow::new(("layout-recent", i), name.clone())
                    .description(summary)
                    .on_select(move |window, cx| {
                        app.update(cx, |a, cx| a.open_layout(&name, window, cx))
                            .ok();
                    }),
            );
        }
    }
    menu.separator().row(
        MenuRow::new("layout-open", "Open layout…")
            .icon(Glyph::FOLDER_OPEN)
            .on_select(act(TradeTerm::open_layouts_dialog)),
    )
}

impl TradeTerm {
    /// The tabs as they are now (the working copy of the layout).
    pub(crate) fn current_tabs(&self, cx: &App) -> Vec<TabSettings> {
        self.tabs
            .iter()
            .map(|c| c.read(cx).state.read(cx).source.tab.settings())
            .collect()
    }

    /// Called with each settings snapshot as it is written: autosaves the
    /// layout, or works out whether it has unsaved changes.
    pub(crate) fn layout_after_save(&mut self, s: &AppSettings, cx: &mut Context<Self>) {
        if self.layout_autosave {
            if let Err(e) = self.layouts.save(&self.layout_name, &s.tabs, s.active_tab) {
                tracing::warn!("could not autosave the layout: {}", e.message());
            }
            return;
        }
        let dirty = self
            .layouts
            .get(&self.layout_name)
            .is_none_or(|l| l.tabs != s.tabs || l.active_tab != s.active_tab);
        if dirty != self.layout_dirty {
            self.layout_dirty = dirty;
            cx.notify();
        }
    }

    fn write_layout(&mut self, name: &str, cx: &mut Context<Self>) -> Result<String, NameError> {
        let tabs = self.current_tabs(cx);
        let name = self.layouts.save(name, &tabs, self.active)?;
        self.layout_name = name.clone();
        self.layout_dirty = false;
        self.persist(cx);
        cx.notify();
        Ok(name)
    }

    /// Save layout (Ctrl+S). A layout that was never saved asks for a name.
    pub(crate) fn save_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.layout_name.clone();
        if self.layouts.contains(&name) {
            self.write_layout(&name, cx).ok();
            window.push_notification(format!("Layout “{name}” saved"), cx);
            return;
        }
        self.prompt_layout_name("Save layout", name, "Save", window, cx, |a, name, _, cx| {
            a.write_layout(&name, cx).map(|_| ())
        });
    }

    pub(crate) fn set_layout_autosave(&mut self, on: bool, cx: &mut Context<Self>) {
        self.layout_autosave = on;
        if on {
            let name = self.layout_name.clone();
            self.write_layout(&name, cx).ok();
        }
        self.persist(cx);
        cx.notify();
    }

    /// Name dialog for a layout; `f` stores it (an error keeps the dialog).
    fn prompt_layout_name(
        &mut self,
        title: &'static str,
        initial: String,
        ok: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl Fn(
            &mut TradeTerm,
            String,
            &mut Window,
            &mut Context<TradeTerm>,
        ) -> Result<(), NameError>
        + 'static,
    ) {
        let app = cx.entity().downgrade();
        name_prompt::open(
            window,
            cx,
            PromptOpts {
                title,
                label: "Layout name",
                initial,
                ok,
                checks: Vec::new(),
            },
            move |name, _, _, window, cx| {
                let r = app.update(cx, |a, cx| f(a, name, window, cx));
                match r {
                    Ok(Ok(())) | Err(_) => Submit::Close,
                    Ok(Err(e)) => Submit::Error(name_error(e)),
                }
            },
        );
    }

    /// "Make a copy…": the current tabs under a new name, which becomes the
    /// current layout.
    pub(crate) fn copy_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let initial = self
            .layouts
            .unique_name(&format!("{} copy", self.layout_name));
        self.prompt_layout_name(
            "Make a copy",
            initial,
            "Save",
            window,
            cx,
            |a, name, _, cx| {
                if a.layouts.contains(&name) {
                    return Err(NameError::Exists);
                }
                a.write_layout(&name, cx).map(|_| ())
            },
        );
    }

    pub(crate) fn rename_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let initial = self.layout_name.clone();
        self.prompt_layout_name(
            "Rename layout",
            initial,
            "Rename",
            window,
            cx,
            |a, name, _, cx| {
                let from = a.layout_name.clone();
                if a.layouts.contains(&from) {
                    a.layout_name = a.layouts.rename(&from, &name)?;
                } else {
                    if a.layouts.contains(&name) {
                        return Err(NameError::Exists);
                    }
                    a.layout_name = tt_session::templates::validate_name(&name)?;
                }
                a.persist(cx);
                cx.notify();
                Ok(())
            },
        );
    }

    /// "Create new layout…": one chart of the current symbol and interval,
    /// no indicators beyond the defaults, under a new name.
    pub(crate) fn new_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let initial = self.layouts.unique_name("New layout");
        self.prompt_layout_name(
            "Create new layout",
            initial,
            "Create",
            window,
            cx,
            |a, name, window, cx| {
                if a.layouts.contains(&name) {
                    return Err(NameError::Exists);
                }
                a.store_current_layout(cx);
                let tab = a.tab_chart().map_or_else(TabSettings::default, |c| {
                    let t = &c.read(cx).state.read(cx).source.tab;
                    TabSettings {
                        symbol: t.symbol.to_string(),
                        timeframe: t.timeframe,
                        ..TabSettings::default()
                    }
                });
                let layout = Layout {
                    name,
                    tabs: vec![tab],
                    ..Layout::default()
                };
                let name = a.layouts.save(&layout.name, &layout.tabs, 0)?;
                a.switch_to(Layout { name, ..layout }, window, cx);
                Ok(())
            },
        );
    }

    /// With autosave on, the layout being left is saved first (TradingView
    /// keeps it); with it off, unsaved changes are dropped like a reload.
    fn store_current_layout(&mut self, cx: &mut Context<Self>) {
        if self.layout_autosave {
            let tabs = self.current_tabs(cx);
            self.layouts
                .save(&self.layout_name, &tabs, self.active)
                .ok();
        }
    }

    /// Opens the saved layout `name`.
    pub(crate) fn open_layout(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(layout) = self.layouts.get(name).cloned() else {
            return;
        };
        if layout.name == self.layout_name {
            return;
        }
        self.store_current_layout(cx);
        self.switch_to(layout, window, cx);
    }

    /// Replaces the tabs with `layout`'s.
    fn switch_to(&mut self, layout: Layout, window: &mut Window, cx: &mut Context<Self>) {
        // Stash the drawings of the tabs that go away.
        self.persist(cx);
        let charts: Vec<_> = layout
            .tabs
            .iter()
            .map(|t| self.make_chart(t, None, window, cx))
            .collect();
        if charts.is_empty() {
            return;
        }
        self.tabs = charts;
        self.active = layout.active_tab.min(self.tabs.len() - 1);
        self.layout_name = layout.name.clone();
        self.layout_dirty = false;
        self.layouts.touch(&layout.name);
        self.refresh_ui_theme(cx);
        self.persist(cx);
        cx.notify();
    }

    pub(crate) fn open_layouts_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity().downgrade();
        let panel = cx.new(|cx| LayoutsPanel::new(app, window, cx));
        let input = panel.read(cx).input.clone();
        tt_ui::open_dialog(
            window,
            cx,
            tt_ui::DialogOpts::new("Open layout", DialogSize::Md)
                .on_open(move |w, cx| input.update(cx, |s, cx| s.focus(w, cx))),
            panel,
        );
    }

    /// "Download chart data…": the active chart's bars as CSV (time in
    /// ISO 8601 UTC, open, high, low, close, volume).
    pub(crate) fn download_chart_data(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(chart) = self.tab_chart() else {
            return;
        };
        let tab = &chart.read(cx).state.read(cx).source.tab;
        let csv = chart_csv(&tab.series);
        let name = format!("{}_{}.csv", tab.display_symbol(), tab.timeframe.code());
        crate::snapshot_menu::save_bytes_as(name, csv.into_bytes(), "Chart data", window, cx);
    }
}

/// CSV of a series: `time,open,high,low,close,volume`.
pub fn chart_csv(s: &tt_core::CandleSeries) -> String {
    let mut out = String::with_capacity(s.len() * 64 + 40);
    out.push_str("time,open,high,low,close,volume\n");
    let (t, o, h, l, c, v) = (
        s.open_times(),
        s.opens(),
        s.highs(),
        s.lows(),
        s.closes(),
        s.volumes(),
    );
    for i in 0..s.len() {
        let stamp = iso_utc(t[i]);
        out.push_str(&format!(
            "{stamp},{},{},{},{},{}\n",
            o[i], h[i], l[i], c[i], v[i]
        ));
    }
    out
}

/// "2026-11-21T04:05:00Z".
fn iso_utc(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let (y, m, d) = tt_core::civil_from_days(secs.div_euclid(86_400));
    let rem = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// "Open layout…": filter field and the saved layouts (name, tabs, last
/// saved); click opens, × deletes (not the current one).
pub struct LayoutsPanel {
    app: WeakEntity<TradeTerm>,
    input: Entity<InputState>,
    _sub: Subscription,
}

impl LayoutsPanel {
    fn new(app: WeakEntity<TradeTerm>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search layouts"));
        let _sub = cx.subscribe_in(
            &input,
            window,
            |this: &mut Self, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } => {
                    if let Some(first) = this.rows(cx).first() {
                        let name = first.name.clone();
                        this.open(&name, window, cx);
                    }
                }
                _ => {}
            },
        );
        Self { app, input, _sub }
    }

    fn rows(&self, cx: &App) -> Vec<Layout> {
        let q = self.input.read(cx).value().trim().to_lowercase();
        self.app.upgrade().map_or_else(Vec::new, |a| {
            a.read(cx)
                .layouts
                .list()
                .iter()
                .filter(|l| q.is_empty() || l.name.to_lowercase().contains(&q))
                .cloned()
                .collect()
        })
    }

    fn open(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        window.close_dialog(cx);
        let name = name.to_owned();
        self.app
            .update(cx, |a, cx| a.open_layout(&name, window, cx))
            .ok();
    }
}

impl Render for LayoutsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ui().clone();
        let current = self
            .app
            .upgrade()
            .map(|a| a.read(cx).layout_name.clone())
            .unwrap_or_default();
        let rows = self.rows(cx);
        let mut list = v_flex()
            .id("layouts-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        if rows.is_empty() {
            list = list.child(EmptyState::new(if self.input.read(cx).value().is_empty() {
                "No saved layouts yet. Save one from the layout menu."
            } else {
                "No layouts match."
            }));
        }
        for (i, l) in rows.into_iter().enumerate() {
            let is_current = l.name.to_lowercase() == current.to_lowercase();
            let name = l.name.clone();
            let del = l.name.clone();
            let app = self.app.clone();
            let secondary = format!(
                "{} · saved {}",
                l.summary(),
                crate::snapshot::utc_stamp(l.modified_ms)
            );
            let mut row = ListRow::new(("layout-row", i))
                .tall()
                .leading(tt_ui::glyph(Glyph::CHART_CANDLESTICK, ICON_SM))
                .primary(l.name.clone())
                .secondary(secondary)
                .selected(is_current)
                .on_click(cx.listener(move |this, _, window, cx| this.open(&name, window, cx)));
            if !is_current {
                row = row.trailing(
                    IconButton::new(("layout-del", i), Glyph::TRASH)
                        .size(ControlSize::Xs)
                        .tooltip("Delete layout")
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            app.update(cx, |a, cx| {
                                a.layouts.delete(&del);
                                cx.notify();
                            })
                            .ok();
                        }),
                );
            }
            list = list.child(row);
        }
        v_flex()
            .h(H_DIALOG_BODY_SM.px())
            .gap(S2.px())
            .child(Input::new(&self.input).prefix(tt_ui::glyph(Glyph::SEARCH, ICON_SM)))
            .child(list)
            .child(
                h_flex()
                    .text_size(TEXT_SMALL.px())
                    .text_color(hsla(t.text_muted))
                    .child("Drawings are kept per symbol and shared by all layouts."),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tt_core::Candle;

    #[::core::prelude::v1::test]
    fn csv_has_header_and_iso_times() {
        let mut s = tt_core::CandleSeries::new();
        s.set_history([Candle {
            open_time_ms: 1_795_233_900_000,
            open: 1.0,
            high: 2.5,
            low: 0.5,
            close: 2.0,
            volume: 10.0,
        }]);
        assert_eq!(
            chart_csv(&s),
            "time,open,high,low,close,volume\n2026-11-21T04:05:00Z,1,2.5,0.5,2,10\n"
        );
    }
}
