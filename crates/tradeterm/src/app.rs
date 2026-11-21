//! The tradeterm window: title bar with chart tabs, toolbar, the active
//! chart, status bar.

use crate::indicators::{IndicatorSettingsPanel, IndicatorsPanel};
use crate::search::{self, SymbolSearch};
use crate::source::TabSource;
use crate::ui::{self, lucide};
use ecolor::Color32;
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
use tt_chart::ChartType;
use tt_chart_gpui::{Chart, ChartEvent, ChartState, hsla};
use tt_core::{ConnectionStatus, Symbol, Timeframe};
use tt_session::fsutil::Poll;
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

/// Key bindings.
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

/// F12 overlay numbers (per second, for the active chart).
#[derive(Debug, Clone, Copy, Default)]
struct PerfStats {
    events: u64,
    prev: (tt_chart_gpui::LayerStats, tt_chart_gpui::LayerStats, u64),
    /// (base paints, base ms avg, overlay paints, overlay ms avg, events)
    last: (u64, f64, u64, f64, u64),
}

/// Messages from background threads to the UI thread.
enum Wake {
    Now,
    After(Duration),
}

pub type TabChart = Entity<Chart<TabSource>>;

pub struct TradeTerm {
    shared: Shared,
    pub(crate) tabs: Vec<TabChart>,
    active: usize,
    next_tab_id: u64,
    theme_mode: ThemeMode,
    overrides: ThemeOverrides,
    /// Chart palette for the current appearance.
    theme: Theme,
    /// Chart settings shared by all charts (TradingView's Settings dialog).
    chart_style: tt_chart::ChartStyle,
    store: SettingsStore,
    recent: Vec<RecentSymbol>,
    pub(crate) favorite_timeframes: Vec<Timeframe>,
    pub(crate) custom_timeframes: Vec<Timeframe>,
    pub(crate) collapsed_timeframe_sections: Vec<String>,
    pub(crate) favorite_chart_types: Vec<ChartType>,
    favorite_indicators: Vec<tt_core::StudyKind>,
    perf: bool,
    perf_stats: PerfStats,
    /// Quote chip of the symbol search, remembered between openings.
    pub(crate) search_quote: usize,
    /// Drawing tool and toggles (left toolbar), shared by all charts.
    pub(crate) draw: crate::drawing_toolbar::DrawMode,
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
        // Chart settings; older up/down color overrides move into them.
        let mut style = settings.chart_style.clone();
        let old = settings.overrides();
        style.colors.up_body = style.colors.up_body.or(old.up);
        style.colors.down_body = style.colors.down_body.or(old.down);
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
            chart_style: style.clone(),
            drawings: Arc::new(std::sync::Mutex::new(settings.drawings.clone())),
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
                        app.sample_perf(cx);
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
            overrides: style_overrides(&style),
            theme: Theme::for_dark(dark).with_overrides(style_overrides(&style)),
            chart_style: style.clone(),
            store,
            recent: settings.recent_symbols.clone(),
            favorite_timeframes: settings.favorite_timeframes.clone(),
            custom_timeframes: settings.custom_timeframes.clone(),
            collapsed_timeframe_sections: settings.collapsed_timeframe_sections.clone(),
            favorite_chart_types: settings.favorite_chart_types.clone(),
            favorite_indicators: settings.favorite_indicators.clone(),
            perf: false,
            perf_stats: PerfStats::default(),
            search_quote: 0,
            draw: Default::default(),
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
                ChartEvent::StyleChanged(style) => app.set_chart_style(style.clone(), true, cx),
                ChartEvent::OpenSettings => app.open_chart_settings(window, cx),
                ChartEvent::DrawingsChanged => app.drawings_changed(chart, cx),
            },
        );
        self._subscriptions.push(sub);
        self.apply_draw_mode_to(&chart, cx);
        chart
    }

    pub(crate) fn tab_chart(&self) -> Option<&TabChart> {
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
        let mut events = 0u64;
        for chart in self.tabs.clone() {
            chart.update(cx, |chart, cx| {
                let n = chart.state.update(cx, |s, _| {
                    let n = s.source.tab.drain_events(&shared) as u64;
                    if n > 0 {
                        s.source.refresh_title();
                    }
                    n
                });
                if n > 0 {
                    chart.invalidate(cx);
                    any = true;
                }
                events += n;
            });
        }
        self.perf_stats.events += events;
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
        // Up/down colors live in the chart settings now.
        s.set_overrides(ThemeOverrides::default());
        s.chart_style = self.chart_style.clone();
        if let Ok(store) = self.shared.drawings.lock() {
            s.drawings = store
                .iter()
                .filter(|(k, v)| !v.is_empty() && k.as_str() != "SYNTH")
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
        }
        s
    }

    /// Moves changed drawings of every tab into the shared store.
    fn stash_drawings(&self, cx: &mut App) {
        let shared = self.shared.clone();
        for chart in &self.tabs {
            chart.update(cx, |c, cx| {
                c.state
                    .update(cx, |s, _| s.source.tab.stash_drawings(&shared))
            });
        }
    }

    /// Saves settings (debounced by the store). Cheap: only marks them
    /// changed; the snapshot is built once they have been stable for the
    /// debounce time, and written on the store's background thread.
    pub(crate) fn persist(&mut self, cx: &mut Context<Self>) {
        // Dirty-flag check per tab; keeps a closing tab's drawings.
        self.stash_drawings(cx);
        if let Some(wait) = self.store.mark_changed(Instant::now()) {
            self.schedule_settings_check(wait, cx);
        }
    }

    fn schedule_settings_check(&self, wait: Duration, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(wait + Duration::from_millis(10))
                .await;
            this.update(cx, |app, cx| app.check_settings(cx)).ok();
        })
        .detach();
    }

    fn check_settings(&mut self, cx: &mut Context<Self>) {
        match self.store.poll(Instant::now()) {
            Poll::Due => {
                self.stash_drawings(cx);
                let snapshot = self.settings_snapshot(cx);
                self.store.save(snapshot);
            }
            Poll::Wait(wait) => self.schedule_settings_check(wait, cx),
            Poll::Idle => {}
        }
    }

    /// Writes pending changes and waits for the background writer (on quit).
    pub fn flush_settings(&mut self, cx: &mut App) {
        self.stash_drawings(cx);
        let snapshot = self.settings_snapshot(cx);
        self.store.flush_with(|| snapshot);
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
            self.stash_drawings(cx);
            self.active = index;
            // Another tab may have changed this symbol's drawings.
            let shared = self.shared.clone();
            self.tabs[index].update(cx, |c, cx| {
                c.update_state(cx, |s| s.source.tab.load_drawings(&shared))
            });
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
        // Opened from the ticker: the current symbol, selected.
        let select_all = initial.is_empty();
        let initial = if select_all {
            self.tab_chart()
                .map(|c| c.read(cx).state.read(cx).source.tab.display_symbol())
                .unwrap_or_default()
        } else {
            initial.to_owned()
        };
        let quote = self.search_quote;
        let panel = cx.new(|cx| {
            SymbolSearch::new(
                app, source, handle, theme, recent, &initial, quote, select_all, window, cx,
            )
        });
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

    fn sample_perf(&mut self, cx: &mut Context<Self>) {
        let Some(chart) = self.tab_chart() else {
            return;
        };
        let s = chart.read(cx).state.read(cx);
        let (b, o) = (s.base_stats, s.overlay_stats);
        let (pb, po, pe) = self.perf_stats.prev;
        let avg = |now: tt_chart_gpui::LayerStats, prev: tt_chart_gpui::LayerStats| {
            let n = now.paints.saturating_sub(prev.paints);
            let ms = now.total.saturating_sub(prev.total).as_secs_f64() * 1000.0;
            (n, if n > 0 { ms / n as f64 } else { 0.0 })
        };
        let ((bn, bms), (on, oms)) = (avg(b, pb), avg(o, po));
        let events = self.perf_stats.events.saturating_sub(pe);
        self.perf_stats.last = (bn, bms, on, oms, events);
        self.perf_stats.prev = (b, o, self.perf_stats.events);
        if self.perf {
            cx.notify();
        }
    }

    fn perf_overlay(&self, cx: &App) -> Option<Div> {
        if !self.perf {
            return None;
        }
        let chart = self.tab_chart()?;
        let s = chart.read(cx).state.read(cx);
        let (bn, bms, on, oms, events) = self.perf_stats.last;
        let t = &self.theme;
        let line = |k: &str, v: String| {
            h_flex()
                .gap_3()
                .child(
                    div()
                        .w(px(110.))
                        .text_color(hsla(t.text_muted))
                        .child(k.to_owned()),
                )
                .child(v)
        };
        Some(
            v_flex()
                .absolute()
                .top(px(36.))
                .right(px(90.))
                .p_3()
                .gap_0p5()
                .rounded(px(8.))
                .bg(hsla(tt_theme::with_alpha(t.panel, 0.94)))
                .border_1()
                .border_color(hsla(t.border))
                .text_size(px(12.))
                .font_family("Consolas")
                .child(div().pb_1().child("Performance (F12)"))
                .child(line(
                    "Base layer",
                    format!("{bn:>3}/s  {bms:5.2} ms  {} prims", s.base_stats.primitives),
                ))
                .child(line(
                    "Overlay",
                    format!(
                        "{on:>3}/s  {oms:5.2} ms  {} prims",
                        s.overlay_stats.primitives
                    ),
                ))
                .child(line("Data events", format!("{events}/s")))
                .child(line("Bars", s.source.tab.series.len().to_string())),
        )
    }

    /// Applies chart settings to every chart (and new tabs); saves them if
    /// `persist` (the dialog previews without saving).
    pub fn set_chart_style(
        &mut self,
        style: tt_chart::ChartStyle,
        persist: bool,
        cx: &mut Context<Self>,
    ) {
        self.overrides = style_overrides(&style);
        self.theme = Theme::for_dark(self.theme.dark).with_overrides(self.overrides);
        self.shared.chart_style = style.clone();
        self.chart_style = style.clone();
        for chart in &self.tabs {
            let (style, theme) = (style.clone(), self.theme.clone());
            chart.update(cx, |chart, cx| {
                chart.update_state(cx, |s| {
                    s.theme = theme;
                    s.source.tab.chart.style = style;
                })
            });
        }
        if persist {
            self.persist(cx);
        }
        cx.notify();
    }

    /// TradingView's chart "Settings" dialog.
    pub fn open_chart_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity().downgrade();
        let (style, base) = (self.chart_style.clone(), Theme::for_dark(self.theme.dark));
        let panel = cx
            .new(|cx| crate::chart_settings::ChartSettingsPanel::new(app, style, base, window, cx));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title("Settings")
                .w(px(780.))
                .entrance(DialogEntrance::FadeSlide)
                .child(panel.clone())
        });
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn current_timeframe(&self, cx: &App) -> Timeframe {
        self.tab_chart().map_or(Timeframe::M1, |c| {
            c.read(cx).state.read(cx).source.tab.timeframe
        })
    }

    pub fn current_chart_type(&self, cx: &App) -> ChartType {
        self.tab_chart().map_or(ChartType::Candles, |c| {
            c.read(cx).state.read(cx).source.tab.chart.chart_type
        })
    }

    pub fn set_chart_type(&mut self, t: ChartType, cx: &mut Context<Self>) {
        self.with_tab(cx, |s, _| s.tab.chart.chart_type = t);
    }

    pub fn toggle_favorite_chart_type(&mut self, t: ChartType, cx: &mut Context<Self>) {
        if let Some(i) = self.favorite_chart_types.iter().position(|f| *f == t) {
            self.favorite_chart_types.remove(i);
        } else {
            self.favorite_chart_types.push(t);
            // Keep TradingView's menu order on the toolbar.
            self.favorite_chart_types
                .sort_by_key(|f| ChartType::ALL.iter().position(|a| a == f));
        }
        self.persist(cx);
        cx.notify();
    }

    pub fn toggle_favorite_timeframe(&mut self, tf: Timeframe, cx: &mut Context<Self>) {
        if let Some(i) = self.favorite_timeframes.iter().position(|f| *f == tf) {
            self.favorite_timeframes.remove(i);
        } else {
            self.favorite_timeframes.push(tf);
            self.favorite_timeframes.sort_by_key(|t| t.duration_ms());
        }
        self.persist(cx);
        cx.notify();
    }

    pub fn toggle_timeframe_section(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some(i) = self
            .collapsed_timeframe_sections
            .iter()
            .position(|k| k == key)
        {
            self.collapsed_timeframe_sections.remove(i);
        } else {
            self.collapsed_timeframe_sections.push(key.to_owned());
        }
        self.persist(cx);
    }

    pub fn remove_custom_timeframe(&mut self, tf: Timeframe, cx: &mut Context<Self>) {
        self.custom_timeframes.retain(|t| *t != tf);
        self.favorite_timeframes.retain(|t| *t != tf);
        self.persist(cx);
        cx.notify();
    }

    /// Adds a user interval to the menu (and switches to it).
    pub fn add_custom_timeframe(&mut self, tf: Timeframe, cx: &mut Context<Self>) {
        if !crate::menus::is_preset(tf) && !self.custom_timeframes.contains(&tf) {
            self.custom_timeframes.push(tf);
        }
        self.set_timeframe(tf, cx);
    }

    pub fn open_custom_interval(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity().downgrade();
        let panel = cx.new(|cx| crate::menus::CustomInterval::new(app, window, cx));
        let focus_once = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if focus_once.replace(false) {
                crate::menus::CustomInterval::focus_input(&panel, window, cx);
            }
            dialog
                .title("Add custom interval")
                .w(px(520.))
                .entrance(DialogEntrance::FadeSlide)
                .child(panel.clone())
        });
    }

    pub fn set_timeframe(&mut self, tf: Timeframe, cx: &mut Context<Self>) {
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
                                    let colors = this.clone();
                                    menu.min_w(px(240.))
                                        .label("Theme")
                                        .item(item("Dark", ThemeMode::Dark))
                                        .item(item("Light", ThemeMode::Light))
                                        .item(item("System", ThemeMode::System))
                                        .separator()
                                        .item(PopupMenuItem::new("Chart settings…").on_click(
                                            move |_, window, cx| {
                                                colors
                                                    .update(cx, |app, cx| {
                                                        app.open_chart_settings(window, cx)
                                                    })
                                                    .ok();
                                            },
                                        ))
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
        let (symbol, tf, chart_type) = (tab.display_symbol(), tab.timeframe, tab.chart.chart_type);

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
        let tf_menu = crate::menus::timeframe_menu(cx);
        // Favorite chart types as quick buttons, then the current type's menu.
        let mut chart_types = h_flex().gap_0p5();
        for fav in self.favorite_chart_types.clone() {
            if fav == chart_type {
                continue;
            }
            chart_types = chart_types.child(
                Button::new(SharedString::from(format!("ct-fav-{}", fav.label())))
                    .ghost()
                    .small()
                    .child(lucide(ui::chart_type_icon(fav), 17.))
                    .tooltip(fav.label())
                    .on_click(cx.listener(move |this, _, _, cx| this.set_chart_type(fav, cx))),
            );
        }
        let ct_menu = chart_types.child(crate::menus::chart_type_menu(chart_type, cx));

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
                if window.has_active_dialog(cx) || !this.focus.is_focused(window) {
                    return;
                }
                // Drawing keys: Escape, Enter, Delete / Backspace, Alt shortcuts.
                if let Some(chart) = this.tab_chart().cloned() {
                    let handled = match k.key.as_str() {
                        "escape" if !m.modified() => chart.update(cx, |c, cx| c.cancel_drawing(cx)),
                        "enter" if !m.modified() => chart.update(cx, |c, cx| c.finish_drawing(cx)),
                        "delete" | "backspace" if !m.modified() => {
                            chart.update(cx, |c, cx| c.delete_selected_drawing(cx))
                        }
                        key if m.alt && !m.control && !m.platform => {
                            match crate::drawing_toolbar::shortcut_tool(key, m.shift) {
                                Some(tool) => {
                                    this.pick_tool(Some(tool), cx);
                                    true
                                }
                                None => false,
                            }
                        }
                        _ => false,
                    };
                    if handled {
                        cx.stop_propagation();
                        return;
                    }
                }
                if m.control || m.alt || m.platform {
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
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.drawing_toolbar(cx))
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            // Clicking the chart gives the keyboard back to the
                            // app (Escape / Delete for drawings).
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| this.focus.focus(window, cx)),
                            )
                            .children(chart)
                            .children(self.perf_overlay(cx)),
                    ),
            )
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

/// Theme up/down overrides that follow the chart's candle body colors, so
/// the tab strip and status bar match the chart.
fn style_overrides(style: &tt_chart::ChartStyle) -> ThemeOverrides {
    ThemeOverrides {
        up: style.colors.up_body,
        down: style.colors.down_body,
    }
}
