//! The tradeterm window: title bar with chart tabs, toolbar, the active
//! chart, status bar.

use crate::source::TabSource;
use crate::ui;
use futures::StreamExt;
use futures::channel::mpsc;
use gpui_kit::component::{WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tt_brokers::BinanceSource;
use tt_chart::ChartType;
use tt_chart_gpui::{Chart, ChartEvent, ChartState, hsla};
use tt_core::{Symbol, Timeframe};
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
        /// "Indicators, metrics, and strategies" ("/" on the chart).
        OpenIndicators,
    ]
);

/// Key context of the app root. Chart-only shortcuts ("/", drawing tools)
/// are bound in it; tooltips look their keys up in it.
pub const KEY_CONTEXT: &str = "Tradeterm";

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
        KeyBinding::new("/", OpenIndicators, Some(KEY_CONTEXT)),
    ]);
    cx.bind_keys(crate::drawing_toolbar::key_bindings());
}

/// F12 overlay numbers (per second, for the active chart).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PerfStats {
    pub(crate) events: u64,
    pub(crate) prev: (tt_chart_gpui::LayerStats, tt_chart_gpui::LayerStats, u64),
    /// (base paints, base ms avg, overlay paints, overlay ms avg, events)
    pub(crate) last: (u64, f64, u64, f64, u64),
}

/// Messages from background threads to the UI thread.
enum Wake {
    Now,
    After(Duration),
}

pub type TabChart = Entity<Chart<TabSource>>;

pub struct TradeTerm {
    pub(crate) shared: Shared,
    pub(crate) tabs: Vec<TabChart>,
    pub(crate) active: usize,
    next_tab_id: u64,
    pub(crate) theme_mode: ThemeMode,
    overrides: ThemeOverrides,
    /// Chart palette for the current appearance.
    pub(crate) theme: Theme,
    /// Chart settings shared by all charts (TradingView's Settings dialog).
    pub(crate) chart_style: tt_chart::ChartStyle,
    store: SettingsStore,
    pub(crate) recent: Vec<RecentSymbol>,
    pub(crate) favorite_timeframes: Vec<Timeframe>,
    pub(crate) custom_timeframes: Vec<Timeframe>,
    pub(crate) collapsed_timeframe_sections: Vec<String>,
    pub(crate) favorite_chart_types: Vec<ChartType>,
    pub(crate) favorite_indicators: Vec<tt_core::StudyKind>,
    pub(crate) perf: bool,
    pub(crate) perf_stats: PerfStats,
    /// Quote chip of the symbol search, remembered between openings.
    pub(crate) search_quote: usize,
    /// Drawing tool and toggles (left toolbar), shared by all charts.
    pub(crate) draw: crate::drawing_toolbar::DrawMode,
    pub(crate) focus: FocusHandle,
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
        ui::apply_theme(&app.theme, cx);
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

    /// Saves settings (debounced by the store).
    pub(crate) fn persist(&mut self, cx: &mut Context<Self>) {
        self.stash_drawings(cx);
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

    pub fn flush_settings(&mut self, cx: &mut App) {
        self.stash_drawings(cx);
        let snapshot = self.settings_snapshot(cx);
        self.store.flush(&snapshot);
    }

    // ---- actions -----------------------------------------------------------

    pub(crate) fn new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(crate) fn set_theme(
        &mut self,
        mode: ThemeMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.theme_mode = mode;
        let dark = match mode {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => window.appearance() == WindowAppearance::Dark,
        };
        self.theme = Theme::for_dark(dark).with_overrides(self.overrides);
        ui::apply_theme(&self.theme, cx);
        for chart in &self.tabs {
            let theme = self.theme.clone();
            chart.update(cx, |chart, cx| chart.update_state(cx, |s| s.theme = theme));
        }
        self.persist(cx);
        cx.notify();
    }

    pub(crate) fn toggle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        if &self.theme != tt_ui::ActiveUiTheme::ui(&**cx) {
            ui::apply_theme(&self.theme, cx);
        }
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

    pub fn set_timeframe(&mut self, tf: Timeframe, cx: &mut Context<Self>) {
        self.with_tab(cx, |s, shared| s.tab.set_timeframe(tf, shared));
    }

    pub(crate) fn load_synthetic(&mut self, cx: &mut Context<Self>) {
        self.perf = true;
        self.with_tab(cx, |s, shared| {
            s.tab.set_synthetic(Some(synthetic::DEFAULT_BARS), shared)
        });
    }
}

impl Render for TradeTerm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme.clone();
        let chart = self.tab_chart().cloned();
        v_flex()
            .id("tradeterm")
            .key_context(KEY_CONTEXT)
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
                // Drawing keys: Escape, Delete / Backspace. (The Alt tool
                // shortcuts are actions, see `drawing_toolbar::key_bindings`.)
                if let Some(chart) = this.tab_chart().cloned() {
                    let handled = match k.key.as_str() {
                        "escape" if !m.modified() => chart.update(cx, |c, cx| c.cancel_drawing(cx)),
                        "delete" | "backspace" if !m.modified() => {
                            chart.update(cx, |c, cx| c.delete_selected_drawing(cx))
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
                // "/" is the `OpenIndicators` binding; this catches layouts
                // where the binding's keystroke does not match.
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
            .on_action(cx.listener(|this, _: &OpenIndicators, window, cx| {
                // As the old "/" key handler: only while the chart area has
                // the keyboard.
                if window.has_active_dialog(cx) || !this.focus.is_focused(window) {
                    cx.propagate();
                    return;
                }
                this.open_indicators(window, cx);
            }))
            .map(|root| crate::drawing_toolbar::on_pick_actions(root, cx))
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
    /// Tab strip clicks (called from `title_bar::tab_pill`).
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
