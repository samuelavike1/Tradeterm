//! The tradeterm window: title bar with chart tabs, toolbar, the active
//! chart, status bar.

use crate::indicators::{IndicatorSettingsPanel, IndicatorsPanel};
use crate::search::{self, SymbolSearch};
use crate::source::TabSource;
use crate::ui::{self, lucide};
use egui::Color32;
use futures::StreamExt;
use futures::channel::mpsc;
use gpui_kit::component::Selectable as _;
use gpui_kit::component::button::*;
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::component::menu::*;
use gpui_kit::component::{IconName, Sizable, TitleBar, WindowExt as _, h_flex, v_flex};
use gpui_kit::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tt_brokers::BinanceSource;
use tt_chart::{ChartType, ScaleMode};
use tt_chart_gpui::{Chart, ChartEvent, ChartState, hsla};
use tt_core::{ConnectionStatus, Symbol, Timeframe};
use tt_session::runtime::BackgroundRuntime;
use tt_session::settings::{AppSettings, RecentSymbol, SettingsStore, TabSettings};
use tt_session::synthetic;
use tt_session::tab::{ChartTab, Shared};
use tt_theme::{Theme, ThemeMode, ThemeOverrides};

actions!(
    tradeterm,
    [
        NewTab,
        CloseTab,
        NextTab,
        PrevTab,
        ToggleTheme,
        LoadSynthetic,
        TogglePerf,
    ]
);

/// Key bindings, like the egui app's.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-t", NewTab, None),
        KeyBinding::new("ctrl-w", CloseTab, None),
        KeyBinding::new("ctrl-tab", NextTab, None),
        KeyBinding::new("ctrl-shift-tab", PrevTab, None),
        KeyBinding::new("ctrl-shift-t", ToggleTheme, None),
        KeyBinding::new("ctrl-shift-d", LoadSynthetic, None),
        KeyBinding::new("f12", TogglePerf, None),
    ]);
}

/// Messages from background threads to the UI thread.
enum Wake {
    Now,
    After(Duration),
}

pub type TabChart = Entity<Chart<TabSource>>;

pub struct TradeTerm {
    shared: Shared,
    tabs: Vec<TabChart>,
    active: usize,
    next_tab_id: u64,
    theme_mode: ThemeMode,
    overrides: ThemeOverrides,
    /// Chart palette for the current appearance.
    theme: Theme,
    store: SettingsStore,
    recent: Vec<RecentSymbol>,
    favorite_timeframes: Vec<Timeframe>,
    custom_timeframes: Vec<Timeframe>,
    collapsed_timeframe_sections: Vec<String>,
    favorite_chart_types: Vec<ChartType>,
    favorite_indicators: Vec<tt_core::StudyKind>,
    perf: bool,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
    /// Owns the tokio thread; dropped after the tabs.
    _runtime: BackgroundRuntime,
}

impl TradeTerm {
    pub fn new(
        settings: AppSettings,
        store: SettingsStore,
        runtime: BackgroundRuntime,
        synthetic: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (tx, mut rx) = mpsc::unbounded::<Wake>();
        let tx_after = tx.clone();
        let source = Arc::new(BinanceSource::new().expect("HTTP client"));
        let shared = Shared {
            wake: Arc::new(move || {
                let _ = tx.unbounded_send(Wake::Now);
            }),
            wake_after: Arc::new(move |d| {
                let _ = tx_after.unbounded_send(Wake::After(d));
            }),
            handle: runtime.handle().clone(),
            source,
            chart_style: tt_chart::ChartStyle::default(),
        };

        // Background data → UI: drain all pending wake-ups, then apply events
        // once (coalesces bursts of ticks into one update).
        cx.spawn(async move |this, cx| {
            while let Some(msg) = rx.next().await {
                let mut delays = Vec::new();
                if let Wake::After(d) = msg {
                    delays.push(d);
                }
                while let Ok(m) = rx.try_recv() {
                    if let Wake::After(d) = m {
                        delays.push(d);
                    }
                }
                for d in delays {
                    let this = this.clone();
                    cx.spawn(async move |cx| {
                        cx.background_executor().timer(d).await;
                        this.update(cx, |app, cx| app.drain(cx)).ok();
                    })
                    .detach();
                }
                if this.update(cx, |app, cx| app.drain(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();

        // Once a second: the bar-close countdown on live charts.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this
                    .update(cx, |app, cx| {
                        if let Some(chart) = app.tabs.get(app.active) {
                            let live = chart.read(cx).state.read(cx).source.tab.synthetic.is_none();
                            if live {
                                chart.clone().update(cx, |chart, cx| chart.invalidate(cx));
                            }
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();

        let dark = match settings.theme_mode {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => window.appearance() == WindowAppearance::Dark,
        };
        let mut app = Self {
            shared,
            tabs: Vec::new(),
            active: settings.active_tab,
            next_tab_id: 0,
            theme_mode: settings.theme_mode,
            overrides: settings.overrides(),
            theme: Theme::for_dark(dark).with_overrides(settings.overrides()),
            store,
            recent: settings.recent_symbols.clone(),
            favorite_timeframes: settings.favorite_timeframes.clone(),
            custom_timeframes: settings.custom_timeframes.clone(),
            collapsed_timeframe_sections: settings.collapsed_timeframe_sections.clone(),
            favorite_chart_types: settings.favorite_chart_types.clone(),
            favorite_indicators: settings.favorite_indicators.clone(),
            perf: false,
            focus: cx.focus_handle(),
            _subscriptions: Vec::new(),
            _runtime: runtime,
        };
        ui::apply_theme(dark, window, cx);
        for (i, t) in settings.tabs.iter().enumerate() {
            let synthetic = (i == settings.active_tab).then_some(synthetic).flatten();
            let chart = app.make_chart(t, synthetic, window, cx);
            app.tabs.push(chart);
        }
        app.active = app.active.min(app.tabs.len().saturating_sub(1));
        let quit = cx.on_app_quit(|app, cx| {
            app.flush_settings(cx);
            async {}
        });
        app._subscriptions.push(quit);
        app.focus.focus(window, cx);
        app
    }

    fn make_chart(
        &mut self,
        t: &TabSettings,
        synthetic: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> TabChart {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let tab = ChartTab::new(id, t, synthetic, &self.shared);
        let mut source = TabSource {
            tab,
            title: String::new(),
            shared: self.shared.clone(),
        };
        source.refresh_title();
        let theme = self.theme.clone();
        let chart = cx.new(|cx| Chart::new(ChartState::new(source, theme), cx));
        let sub = cx.subscribe_in(
            &chart,
            window,
            |app, chart, ev: &ChartEvent, window, cx| match ev {
                ChartEvent::OpenIndicatorSettings(id) => {
                    app.open_indicator_settings(chart.clone(), *id, window, cx)
                }
                ChartEvent::Changed => app.persist(cx),
            },
        );
        self._subscriptions.push(sub);
        chart
    }

    fn tab_chart(&self) -> Option<&TabChart> {
        self.tabs.get(self.active)
    }

    /// Runs `f` on the active tab's session and repaints its chart.
    fn with_tab(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut TabSource, &Shared)) {
        let Some(chart) = self.tab_chart().cloned() else {
            return;
        };
        let shared = self.shared.clone();
        chart.update(cx, |chart, cx| {
            chart.update_state(cx, |s| {
                f(&mut s.source, &shared);
                s.source.refresh_title();
            });
        });
        self.persist(cx);
        cx.notify();
    }

    /// Applies pending market data to every tab.
    fn drain(&mut self, cx: &mut Context<Self>) {
        let shared = self.shared.clone();
        let mut any = false;
        for chart in self.tabs.clone() {
            chart.update(cx, |chart, cx| {
                let n = chart.state.update(cx, |s, _| {
                    let n = s.source.tab.drain_events(&shared);
                    if n > 0 {
                        s.source.refresh_title();
                    }
                    n
                });
                if n > 0 {
                    chart.invalidate(cx);
                    any = true;
                }
            });
        }
        if any {
            // Tab strip change %, status bar.
            cx.notify();
        }
    }

    fn settings_snapshot(&self, cx: &App) -> AppSettings {
        let mut s = AppSettings {
            theme_mode: self.theme_mode,
            tabs: self
                .tabs
                .iter()
                .map(|c| c.read(cx).state.read(cx).source.tab.settings())
                .collect(),
            active_tab: self.active,
            recent_symbols: self.recent.clone(),
            favorite_timeframes: self.favorite_timeframes.clone(),
            custom_timeframes: self.custom_timeframes.clone(),
            collapsed_timeframe_sections: self.collapsed_timeframe_sections.clone(),
            favorite_chart_types: self.favorite_chart_types.clone(),
            favorite_indicators: self.favorite_indicators.clone(),
            ..AppSettings::default()
        };
        s.set_overrides(self.overrides);
        s
    }

    /// Saves settings (debounced by the store).
    fn persist(&mut self, cx: &mut Context<Self>) {
        let snapshot = self.settings_snapshot(cx);
        if let Some(wait) = self.store.tick(&snapshot, Instant::now()) {
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(wait + Duration::from_millis(10))
                    .await;
                this.update(cx, |app, cx| app.persist(cx)).ok();
            })
            .detach();
        }
    }

    pub fn flush_settings(&mut self, cx: &App) {
        let snapshot = self.settings_snapshot(cx);
        self.store.flush(&snapshot);
    }

    // ---- actions -----------------------------------------------------------

    fn new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let settings = self
            .tab_chart()
            .map(|c| c.read(cx).state.read(cx).source.tab.settings())
            .unwrap_or_default();
        let chart = self.make_chart(&settings, None, window, cx);
        self.active = (self.active + 1).min(self.tabs.len());
        self.tabs.insert(self.active, chart);
        self.persist(cx);
        cx.notify();
    }

    fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 || index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        if self.active > index || self.active >= self.tabs.len() {
            self.active = self.active.saturating_sub(1);
        }
        self.persist(cx);
        cx.notify();
    }

    fn select_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.tabs.len() && index != self.active {
            self.active = index;
            self.persist(cx);
            cx.notify();
        }
    }

    fn set_theme(&mut self, mode: ThemeMode, window: &mut Window, cx: &mut Context<Self>) {
        self.theme_mode = mode;
        let dark = match mode {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => window.appearance() == WindowAppearance::Dark,
        };
        self.theme = Theme::for_dark(dark).with_overrides(self.overrides);
        ui::apply_theme(dark, window, cx);
        for chart in &self.tabs {
            let theme = self.theme.clone();
            chart.update(cx, |chart, cx| chart.update_state(cx, |s| s.theme = theme));
        }
        self.persist(cx);
        cx.notify();
    }

    fn toggle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let next = if self.theme.dark {
            ThemeMode::Light
        } else {
            ThemeMode::Dark
        };
        self.set_theme(next, window, cx);
    }

    pub fn set_symbol(
        &mut self,
        symbol: Symbol,
        info: Option<tt_core::SymbolInfo>,
        cx: &mut Context<Self>,
    ) {
        if let Some(i) = &info {
            let entry = RecentSymbol {
                symbol: i.symbol.to_string(),
                base: i.base.clone(),
                quote: i.quote.clone(),
            };
            self.recent.retain(|r| r.symbol != entry.symbol);
            self.recent.insert(0, entry);
            self.recent.truncate(tt_session::settings::MAX_RECENT);
        }
        self.with_tab(cx, |s, shared| {
            s.tab.synthetic = None;
            s.tab.set_symbol(symbol, shared);
        });
    }

    /// Opens the symbol search, optionally with text already typed.
    pub fn open_search(&mut self, initial: &str, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity().downgrade();
        let (source, handle) = (self.shared.source.clone(), self.shared.handle.clone());
        let (theme, recent) = (self.theme.clone(), search::recent_rows(&self.recent));
        let initial = initial.to_owned();
        let panel = cx
            .new(|cx| SymbolSearch::new(app, source, handle, theme, recent, &initial, window, cx));
        let focus_once = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if focus_once.replace(false) {
                SymbolSearch::focus_input(&panel, window, cx);
            }
            dialog
                .title("Symbol Search")
                .w(px(680.))
                .entrance(DialogEntrance::FadeSlide)
                .child(panel.clone())
        });
    }

    /// "Indicators, metrics, and strategies" (toolbar button or "/").
    pub fn open_indicators(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity().downgrade();
        let (theme, favorites) = (self.theme.clone(), self.favorite_indicators.clone());
        let panel = cx.new(|cx| IndicatorsPanel::new(app, theme, favorites, window, cx));
        let focus_once = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if focus_once.replace(false) {
                IndicatorsPanel::focus_input(&panel, window, cx);
            }
            dialog
                .title("Indicators, metrics, and strategies")
                .w(px(780.))
                .entrance(DialogEntrance::FadeSlide)
                .child(panel.clone())
        });
    }

    pub fn add_indicator(&mut self, kind: tt_core::StudyKind, cx: &mut Context<Self>) {
        self.with_tab(cx, |s, _| {
            s.tab
                .chart
                .indicators
                .push(tt_chart::Indicator::new(kind.default_study()))
        });
    }

    pub fn set_favorite_indicators(
        &mut self,
        favorites: Vec<tt_core::StudyKind>,
        cx: &mut Context<Self>,
    ) {
        self.favorite_indicators = favorites;
        self.persist(cx);
    }

    /// Settings of one indicator (legend gear or double-click).
    fn open_indicator_settings(
        &mut self,
        chart: TabChart,
        id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let exists = chart
            .read(cx)
            .state
            .read(cx)
            .source
            .tab
            .chart
            .indicators
            .iter()
            .any(|i| i.id == id);
        if !exists {
            return;
        }
        let theme = self.theme.clone();
        let panel = cx.new(|cx| {
            IndicatorSettingsPanel::new(chart, id, theme, window, cx)
                .expect("indicator checked above")
        });
        let title = panel.read(cx).title();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(title.clone())
                .w(px(460.))
                .entrance(DialogEntrance::FadeSlide)
                .child(panel.clone())
        });
    }

    fn set_timeframe(&mut self, tf: Timeframe, cx: &mut Context<Self>) {
        self.with_tab(cx, |s, shared| s.tab.set_timeframe(tf, shared));
    }

    // ---- rendering ---------------------------------------------------------

    fn title_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut tabs = h_flex().gap_1().h_full().items_center();
        for (i, chart) in self.tabs.iter().enumerate() {
            let s = &chart.read(cx).state.read(cx).source.tab;
            let (label, badge, change) = (s.display_symbol(), s.base_asset(), s.change_pct());
            tabs = tabs.child(ui::tab_pill(
                i,
                i == self.active,
                label,
                badge,
                change,
                self.tabs.len() > 1,
                &self.theme,
                window,
                cx,
            ));
        }
        let this = cx.entity().downgrade();
        let dark = self.theme.dark;
        let mode = self.theme_mode;
        TitleBar::new().child(
            h_flex()
                .w_full()
                .h_full()
                .pr_2()
                .justify_between()
                .child(
                    tabs.child(
                        Button::new("new-tab")
                            .ghost()
                            .small()
                            .icon(IconName::Plus)
                            .tooltip("New tab (Ctrl+T)")
                            .on_click(cx.listener(|this, _, window, cx| this.new_tab(window, cx))),
                    ),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .child(
                            Button::new("theme")
                                .ghost()
                                .small()
                                .icon(if dark { IconName::Sun } else { IconName::Moon })
                                .tooltip("Toggle theme (Ctrl+Shift+T)")
                                .on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.toggle_theme(window, cx)
                                    }),
                                ),
                        )
                        .child(
                            Button::new("settings")
                                .ghost()
                                .small()
                                .icon(IconName::Settings)
                                .dropdown_menu(move |menu, _, _| {
                                    let item = |label: &'static str, m: ThemeMode| {
                                        let this = this.clone();
                                        PopupMenuItem::new(label).checked(mode == m).on_click(
                                            move |_, window, cx| {
                                                this.update(cx, |app, cx| {
                                                    app.set_theme(m, window, cx)
                                                })
                                                .ok();
                                            },
                                        )
                                    };
                                    let synth = this.clone();
                                    menu.min_w(px(240.))
                                        .label("Theme")
                                        .item(item("Dark", ThemeMode::Dark))
                                        .item(item("Light", ThemeMode::Light))
                                        .item(item("System", ThemeMode::System))
                                        .separator()
                                        .item(
                                            PopupMenuItem::new("Load 1M synthetic bars").on_click(
                                                move |_, _, cx| {
                                                    synth
                                                        .update(cx, |app, cx| {
                                                            app.load_synthetic(cx)
                                                        })
                                                        .ok();
                                                },
                                            ),
                                        )
                                }),
                        ),
                ),
        )
    }

    fn load_synthetic(&mut self, cx: &mut Context<Self>) {
        self.perf = true;
        self.with_tab(cx, |s, shared| {
            s.tab.set_synthetic(Some(synthetic::DEFAULT_BARS), shared)
        });
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(chart) = self.tab_chart() else {
            return div();
        };
        let state = chart.read(cx).state.read(cx);
        let tab = &state.source.tab;
        let (symbol, tf, chart_type, scale) = (
            tab.display_symbol(),
            tab.timeframe,
            tab.chart.chart_type,
            tab.chart.scale_mode,
        );
        let this = cx.entity().downgrade();

        let mut tfs = h_flex().gap_0p5();
        let mut favorites = self.favorite_timeframes.clone();
        if !favorites.contains(&tf) {
            favorites.push(tf);
        }
        favorites.sort_by_key(|t| t.duration_ms());
        for t in favorites {
            tfs = tfs.child(
                Button::new(SharedString::from(format!("tf-{}", t.code())))
                    .ghost()
                    .small()
                    .label(t.label())
                    .selected(t == tf)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_timeframe(t, cx))),
            );
        }
        let tf_menu = {
            let this = this.clone();
            let all: Vec<Timeframe> = Timeframe::ALL
                .iter()
                .copied()
                .chain(self.custom_timeframes.iter().copied())
                .collect();
            Button::new("tf-menu")
                .ghost()
                .small()
                .icon(IconName::ChevronDown)
                .dropdown_menu(move |mut menu, _, _| {
                    menu = menu.min_w(px(200.)).max_h(px(520.)).scrollable(true);
                    let mut section = "";
                    for &t in &all {
                        let name = ui::timeframe_section(t);
                        if name != section {
                            if !section.is_empty() {
                                menu = menu.separator();
                            }
                            menu = menu.label(name);
                            section = name;
                        }
                        let this = this.clone();
                        menu =
                            menu.item(PopupMenuItem::new(t.long_name()).checked(t == tf).on_click(
                                move |_, _, cx| {
                                    this.update(cx, |app, cx| app.set_timeframe(t, cx)).ok();
                                },
                            ));
                    }
                    menu
                })
        };

        let ct_menu = {
            let this = this.clone();
            Button::new("chart-type")
                .ghost()
                .small()
                .child(lucide(ui::chart_type_icon(chart_type), 17.))
                .dropdown_caret(true)
                .tooltip(chart_type.label())
                .dropdown_menu(move |mut menu, _, _| {
                    menu = menu.min_w(px(220.));
                    for (g, group) in ChartType::GROUPS.iter().enumerate() {
                        if g > 0 {
                            menu = menu.separator();
                        }
                        for &t in group.iter() {
                            let this = this.clone();
                            menu = menu.item(
                                PopupMenuItem::new(t.label())
                                    .checked(t == chart_type)
                                    .on_click(move |_, _, cx| {
                                        this.update(cx, |app, cx| {
                                            app.with_tab(cx, |s, _| s.tab.chart.chart_type = t)
                                        })
                                        .ok();
                                    }),
                            );
                        }
                    }
                    menu
                })
        };

        let mut scales = h_flex().gap_0p5();
        for (mode, label) in [
            (ScaleMode::Linear, "Linear"),
            (ScaleMode::Logarithmic, "Log"),
            (ScaleMode::Percent, "%"),
        ] {
            scales = scales.child(
                Button::new(label)
                    .ghost()
                    .small()
                    .label(label)
                    .selected(scale == mode)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.with_tab(cx, |s, _| s.tab.chart.scale_mode = mode)
                    })),
            );
        }

        h_flex()
            .h(px(40.))
            .px_2()
            .gap_1()
            .items_center()
            .border_b_1()
            .border_color(hsla(self.theme.border))
            .child(
                Button::new("ticker")
                    .ghost()
                    .small()
                    .icon(IconName::Search)
                    .label(symbol)
                    .tooltip("Symbol search (or just start typing)")
                    .on_click(cx.listener(|this, _, window, cx| this.open_search("", window, cx))),
            )
            .child(ui::separator(&self.theme))
            .child(tfs)
            .child(tf_menu)
            .child(ui::separator(&self.theme))
            .child(ct_menu)
            .child(ui::separator(&self.theme))
            .child(
                Button::new("indicators")
                    .ghost()
                    .small()
                    .child(
                        h_flex()
                            .gap_1p5()
                            .child(lucide(lucide_icons::Icon::SquareFunction, 17.))
                            .child("Indicators"),
                    )
                    .tooltip("Indicators, metrics, and strategies (/)")
                    .on_click(cx.listener(|this, _, window, cx| this.open_indicators(window, cx))),
            )
            .child(div().flex_1())
            .child(scales)
    }

    fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = &self.theme;
        let Some(chart) = self.tab_chart() else {
            return div();
        };
        let state = chart.read(cx).state.read(cx);
        let tab = &state.source.tab;
        let (dot, text) = match &tab.status {
            ConnectionStatus::Connected => (t.up, "Connected".to_owned()),
            ConnectionStatus::Connecting => (
                Color32::from_rgb(0xf5, 0xb5, 0x2e),
                "Connecting…".to_owned(),
            ),
            other => (t.down, format!("{other:?}")),
        };
        let last = tab.series.last();
        let precision = tab.price_precision();
        let muted = hsla(t.text_muted);
        let mut bar = h_flex()
            .h(px(28.))
            .px_3()
            .gap_4()
            .items_center()
            .text_size(px(12.5))
            .border_t_1()
            .border_color(hsla(t.border))
            .child(
                h_flex()
                    .gap_1p5()
                    .items_center()
                    .child(ui::status_dot(
                        dot,
                        tab.status == ConnectionStatus::Connecting,
                    ))
                    .child(text),
            );
        if let Some(c) = last {
            let color = if c.is_bullish() { t.up } else { t.down };
            bar = bar.child(
                h_flex()
                    .gap_1()
                    .child(div().text_color(muted).child("Last"))
                    .child(
                        div()
                            .text_color(hsla(color))
                            .child(tt_chart::scale::format_price(c.close, precision)),
                    ),
            );
        }
        bar = bar.child(
            h_flex()
                .gap_1()
                .child(div().text_color(muted).child("Bars"))
                .child(tab.series.len().to_string()),
        );
        if self.perf {
            let (b, o) = (state.base_stats, state.overlay_stats);
            bar = bar.child(div().text_color(muted).child(format!(
                "base {:.2} ms · {} prims  |  overlay {:.2} ms · {} prims",
                b.last.as_secs_f64() * 1000.0,
                b.primitives,
                o.last.as_secs_f64() * 1000.0,
                o.primitives
            )));
        }
        let info = tab
            .symbol_info
            .as_ref()
            .map(|i| format!("{} / {}  tick {}", i.base, i.quote, i.tick_size))
            .unwrap_or_default();
        bar.child(div().flex_1())
            .child(div().text_color(muted).child(info))
            .child(
                div()
                    .text_color(muted)
                    .child(format!("{} · UTC", tab.source_name(&self.shared))),
            )
    }
}

impl Render for TradeTerm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme.clone();
        let chart = self.tab_chart().cloned();
        v_flex()
            .id("tradeterm")
            .key_context("Tradeterm")
            .track_focus(&self.focus)
            .size_full()
            .bg(hsla(t.bg))
            .text_color(hsla(t.text))
            .font_family(ui::UI_FONT)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                // Type to search, like TradingView: a letter or digit on the
                // chart opens the symbol search with it.
                let k = &e.keystroke;
                let m = &k.modifiers;
                if m.control || m.alt || m.platform || window.has_active_dialog(cx) {
                    return;
                }
                if !this.focus.is_focused(window) {
                    return;
                }
                let text = k.key_char.clone().unwrap_or_else(|| k.key.clone());
                if text == "/" {
                    this.open_indicators(window, cx);
                    cx.stop_propagation();
                    return;
                }
                if text.chars().count() == 1 && text.chars().all(|c| c.is_ascii_alphanumeric()) {
                    this.open_search(&text, window, cx);
                    cx.stop_propagation();
                }
            }))
            .on_action(cx.listener(|this, _: &NewTab, window, cx| this.new_tab(window, cx)))
            .on_action(cx.listener(|this, _: &CloseTab, _, cx| this.close_tab(this.active, cx)))
            .on_action(cx.listener(|this, _: &NextTab, _, cx| {
                let n = this.tabs.len();
                this.select_tab((this.active + 1) % n, cx)
            }))
            .on_action(cx.listener(|this, _: &PrevTab, _, cx| {
                let n = this.tabs.len();
                this.select_tab((this.active + n - 1) % n, cx)
            }))
            .on_action(
                cx.listener(|this, _: &ToggleTheme, window, cx| this.toggle_theme(window, cx)),
            )
            .on_action(cx.listener(|this, _: &LoadSynthetic, _, cx| this.load_synthetic(cx)))
            .on_action(cx.listener(|this, _: &TogglePerf, _, cx| {
                this.perf = !this.perf;
                cx.notify();
            }))
            .child(self.title_bar(window, cx))
            .child(self.toolbar(cx))
            .child(div().flex_1().min_h_0().children(chart))
            .child(self.status_bar(cx))
    }
}

impl TradeTerm {
    /// Tab strip clicks (called from `ui::tab_pill`).
    pub fn on_tab_click(&mut self, index: usize, close: bool, cx: &mut Context<Self>) {
        if close {
            self.close_tab(index, cx);
        } else {
            self.select_tab(index, cx);
        }
    }

    #[allow(dead_code)]
    pub fn shared(&self) -> &Shared {
        &self.shared
    }
}
