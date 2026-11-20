use crate::chrome::{self, TITLE_BAR_HEIGHT, TabView};
use crate::icons::Icon;
use crate::perf::PerfStats;
use crate::runtime::BackgroundRuntime;
use crate::search::SearchOutcome;
use crate::settings::{AppSettings, SettingsStore};
use crate::synthetic;
use crate::tab::{ChartTab, MAX_HISTORY_BARS, Shared};
use crate::toolbar;
use egui::{
    Align, Color32, Key, KeyboardShortcut, Layout, Modifiers, RichText, Sense, Stroke, vec2,
};
use std::sync::Arc;
use std::time::Instant;
use tracing::warn;
use tt_brokers::{BinanceSource, MarketDataSource};
use tt_chart::{ChartData, ScaleMode};
use tt_core::{ConnectionStatus, Symbol, Timeframe};
use tt_theme::{Theme, ThemeMode, ThemeOverrides};

const TOGGLE_THEME: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::T);
const LOAD_SYNTHETIC: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::D);
const PREV_TAB: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::CTRL.plus(Modifiers::SHIFT), Key::Tab);
const NEXT_TAB: KeyboardShortcut = KeyboardShortcut::new(Modifiers::CTRL, Key::Tab);
const NEW_TAB: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::T);
const CLOSE_TAB: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::W);

pub struct TradeTermApp {
    shared: Shared,
    // Declared before `runtime` so tab streams are aborted before it shuts down.
    tabs: Vec<ChartTab>,
    active: usize,
    next_tab_id: u64,

    theme_mode: ThemeMode,
    overrides: ThemeOverrides,
    /// Resolved palette for the current frame.
    theme: Theme,

    settings_store: SettingsStore,
    perf: PerfStats,
    /// Owns the tokio thread; dropped last (after the tabs).
    _runtime: BackgroundRuntime,
}

impl TradeTermApp {
    pub fn new(
        ctx: egui::Context,
        runtime: BackgroundRuntime,
        settings: AppSettings,
        settings_store: SettingsStore,
        synthetic: Option<usize>,
        bold_family: Option<egui::FontFamily>,
    ) -> anyhow::Result<Self> {
        let settings = settings.normalized();
        let source = Arc::new(BinanceSource::new()?);
        let shared = Shared {
            ctx: ctx.clone(),
            handle: runtime.handle().clone(),
            source: source.clone(),
            bold_family,
        };
        tt_theme::install(&ctx, settings.theme_mode, settings.overrides());
        let theme = tt_theme::current(&ctx, settings.overrides());

        // Warm the symbol list so the first search is instant.
        runtime.handle().spawn(async move {
            if let Err(e) = source.search_symbols("").await {
                warn!("could not preload symbol list: {e:#}");
            }
        });

        let active = settings.active_tab;
        let tabs = settings
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| {
                // `--synthetic` applies to the tab that opens in front.
                let synth = synthetic.filter(|_| i == active);
                ChartTab::new(i as u64, t, synth, &shared)
            })
            .collect::<Vec<_>>();

        Ok(Self {
            next_tab_id: tabs.len() as u64,
            tabs,
            active,
            shared,
            theme_mode: settings.theme_mode,
            overrides: settings.overrides(),
            theme,
            settings_store,
            perf: PerfStats::default(),
            _runtime: runtime,
        })
    }

    fn tab(&mut self) -> &mut ChartTab {
        &mut self.tabs[self.active]
    }

    /// Opens a tab next to the active one, showing the same instrument.
    fn new_tab(&mut self) {
        let settings = self.tabs[self.active].settings();
        let tab = ChartTab::new(self.next_tab_id, &settings, None, &self.shared);
        self.next_tab_id += 1;
        self.active += 1;
        self.tabs.insert(self.active, tab);
    }

    fn close_tab(&mut self, index: usize) {
        if self.tabs.len() <= 1 || index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index); // dropping the tab aborts its stream
        if self.active > index || self.active >= self.tabs.len() {
            self.active = self.active.saturating_sub(1);
        }
    }

    /// Current persisted state, derived from the live app state.
    fn settings_snapshot(&self) -> AppSettings {
        let mut s = AppSettings {
            theme_mode: self.theme_mode,
            tabs: self.tabs.iter().map(ChartTab::settings).collect(),
            active_tab: self.active,
            ..AppSettings::default()
        };
        s.set_overrides(self.overrides);
        s
    }

    fn set_theme(&mut self, mode: ThemeMode, overrides: ThemeOverrides) {
        self.theme_mode = mode;
        self.overrides = overrides;
        tt_theme::install(&self.shared.ctx, mode, overrides);
    }

    fn load_synthetic(&mut self) {
        let shared = self.shared.clone();
        self.tab()
            .set_synthetic(Some(synthetic::DEFAULT_BARS), &shared);
        self.perf.visible = true;
    }

    fn handle_hotkeys(&mut self, ctx: &egui::Context) {
        // Shift variants first: egui matches shortcuts ignoring extra Shift.
        if ctx.input_mut(|i| i.consume_shortcut(&TOGGLE_THEME)) {
            let next = if ctx.theme() == egui::Theme::Dark {
                ThemeMode::Light
            } else {
                ThemeMode::Dark
            };
            self.set_theme(next, self.overrides);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&LOAD_SYNTHETIC)) {
            self.load_synthetic();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&PREV_TAB)) {
            self.active = (self.active + self.tabs.len() - 1) % self.tabs.len();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&NEXT_TAB)) {
            self.active = (self.active + 1) % self.tabs.len();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&NEW_TAB)) {
            self.new_tab();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&CLOSE_TAB)) {
            self.close_tab(self.active);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F12)) {
            self.perf.visible = !self.perf.visible;
        }
    }

    // ---- title bar --------------------------------------------------------

    fn title_bar(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            let theme = self.theme.clone();
            let closable = self.tabs.len() > 1;
            let mut select = None;
            let mut close = None;
            for (i, tab) in self.tabs.iter().enumerate() {
                let label = tab.display_symbol();
                let badge = tab.badge();
                let view = TabView {
                    label: &label,
                    badge: &badge,
                    badge_color: badge_color(&theme, &label),
                    change: tab.change_pct(),
                    active: i == self.active,
                    closable,
                };
                let id = ui.make_persistent_id(("tab", tab.id));
                let action = chrome::tab(ui, id, &view, &theme);
                if action.close {
                    close = Some(i);
                } else if action.select {
                    select = Some(i);
                }
            }
            if let Some(i) = select {
                self.active = i;
            }
            if let Some(i) = close {
                self.close_tab(i);
            }
            if chrome::new_tab_button(ui, &theme).clicked() {
                self.new_tab();
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                chrome::window_controls(ui, &theme);
                ui.add_space(4.0);
                let gear =
                    chrome::title_icon_button(ui, Icon::Settings, &theme).on_hover_text("Settings");
                egui::Popup::menu(&gear).show(|ui| self.settings_menu(ui));
                // Quick light/dark switch (same as Ctrl/Cmd+Shift+T).
                let dark = ui.ctx().theme() == egui::Theme::Dark;
                let (icon, tip) = if dark {
                    (Icon::Sun, "Switch to light theme")
                } else {
                    (Icon::Moon, "Switch to dark theme")
                };
                if chrome::title_icon_button(ui, icon, &theme)
                    .on_hover_text(tip)
                    .clicked()
                {
                    let next = if dark {
                        ThemeMode::Light
                    } else {
                        ThemeMode::Dark
                    };
                    self.set_theme(next, self.overrides);
                }
                ui.add_space(4.0);
                // Whatever is left between the tabs and the buttons moves the window.
                let rest = ui.available_rect_before_wrap();
                chrome::drag_region(ui, rest);
            });
        });
    }

    fn settings_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(240.0);
        ui.label(RichText::new("Theme").strong());
        let mut mode = self.theme_mode;
        for m in ThemeMode::ALL {
            ui.radio_value(&mut mode, m, m.label());
        }
        ui.label(
            RichText::new(format!(
                "Toggle: {}",
                ui.ctx().format_shortcut(&TOGGLE_THEME)
            ))
            .small()
            .weak(),
        );

        ui.separator();
        ui.label(RichText::new("Candle colors").strong());
        let mut overrides = self.overrides;
        color_row(ui, "Up", &mut overrides.up, self.theme.up);
        color_row(ui, "Down", &mut overrides.down, self.theme.down);

        if mode != self.theme_mode || overrides != self.overrides {
            self.set_theme(mode, overrides);
        }

        ui.separator();
        ui.label(RichText::new("Debug").strong());
        ui.checkbox(&mut self.perf.visible, "Performance overlay (F12)");
        let load = format!(
            "Load 1,000,000 synthetic bars ({})",
            ui.ctx().format_shortcut(&LOAD_SYNTHETIC)
        );
        if ui.button(load).clicked() {
            self.load_synthetic();
            ui.close();
        }
        let shared = self.shared.clone();
        if self.tab().synthetic.is_some() && ui.button("Back to live data").clicked() {
            self.tab().set_synthetic(None, &shared);
            ui.close();
        }
    }

    // ---- toolbar ----------------------------------------------------------

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let shared = self.shared.clone();
        let theme = self.theme.clone();
        let tab = &mut self.tabs[self.active];
        toolbar::apply_style(ui, &theme);
        ui.horizontal(|ui| {
            ui.set_height(toolbar::TOOLBAR_HEIGHT);
            tab.search.poll();
            let reply_shared = shared.clone();
            let outcome = tab.search.ui(ui, &theme, move |query, reply| {
                let source = reply_shared.source.clone();
                let ctx = reply_shared.ctx.clone();
                reply_shared.handle.spawn(async move {
                    let result = source
                        .search_symbols(&query)
                        .await
                        .map_err(|e| format!("{e:#}"));
                    let _ = reply.send((query, result));
                    ctx.request_repaint();
                });
            });
            match outcome {
                Some(SearchOutcome::Submit(text)) => tab.set_symbol(Symbol::new(&text), &shared),
                Some(SearchOutcome::Cancel) => tab.search.input = tab.symbol.to_string(),
                None => {}
            }

            toolbar::divider(ui, &theme);
            for tf in Timeframe::ALL {
                let text = RichText::new(tf.label()).size(13.5);
                if ui.selectable_label(tab.timeframe == tf, text).clicked() {
                    tab.set_timeframe(tf, &shared);
                }
            }

            toolbar::divider(ui, &theme);
            toolbar::chart_type_picker(ui, &mut tab.chart.chart_type, &theme);
            toolbar::divider(ui, &theme);

            // Display options on the right, like TradingView's chart settings.
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                for mode in [
                    ScaleMode::Percent,
                    ScaleMode::Logarithmic,
                    ScaleMode::Linear,
                ] {
                    let text = RichText::new(mode.label()).size(13.5);
                    ui.selectable_value(&mut tab.chart.scale_mode, mode, text)
                        .on_hover_text(match mode {
                            ScaleMode::Linear => "Linear price scale",
                            ScaleMode::Logarithmic => "Logarithmic price scale",
                            ScaleMode::Percent => "Percent change from the first visible bar",
                        });
                }
                toolbar::divider(ui, &theme);
                toolbar::icon_toggle(
                    ui,
                    &mut tab.chart.show_volume,
                    Icon::ChartNoAxesColumn,
                    &theme,
                )
                .on_hover_text("Volume pane");
            });
        });
    }

    // ---- status bar -------------------------------------------------------

    fn status_bar(&self, ui: &mut egui::Ui) {
        let t = &self.theme;
        let tab = &self.tabs[self.active];
        ui.horizontal(|ui| {
            let (color, text) = match &tab.status {
                _ if tab.synthetic.is_some() => (
                    t.series_palette[1],
                    format!("Synthetic data ({} 1m bars)", tab.synthetic.unwrap_or(0)),
                ),
                ConnectionStatus::Connected => (t.up, tab.status.to_string()),
                ConnectionStatus::Connecting => (t.series_palette[1], tab.status.to_string()),
                ConnectionStatus::Disconnected { .. } => {
                    (t.down, format!("{} (retrying)", tab.status))
                }
            };
            let (dot, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
            ui.painter().circle_filled(dot.center(), 4.0, color);
            ui.label(text);

            ui.separator();
            match tab.series.last() {
                Some(c) => {
                    let d = tab.price_precision();
                    let color = if c.is_bullish() { t.up } else { t.down };
                    ui.label(RichText::new("Last").weak());
                    ui.label(RichText::new(tt_chart::scale::format_price(c.close, d)).color(color));
                }
                None => {
                    ui.label(RichText::new("Last —").weak());
                }
            }

            ui.separator();
            ui.label(RichText::new("Bars").weak());
            ui.label(RichText::new(tab.series.len().to_string()));
            if tab.backfill.loading {
                ui.label(RichText::new("loading older…").weak());
            } else if tab.backfill.exhausted {
                ui.label(RichText::new("(full history)").weak());
            } else if tab.synthetic.is_none() && tab.series.len() >= MAX_HISTORY_BARS {
                ui.label(RichText::new("(history limit)").weak());
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let tz = tab
                    .symbol_info
                    .as_ref()
                    .map_or("UTC", |i| i.timezone.as_str());
                ui.label(RichText::new(format!("{} · {tz}", tab.source_name(&self.shared))).weak());
                if let Some(i) = &tab.symbol_info {
                    ui.separator();
                    ui.label(
                        RichText::new(format!("{}/{}  tick {}", i.base, i.quote, i.tick_size))
                            .weak(),
                    );
                }
            });
        });
    }

    fn perf_overlay(&mut self, ctx: &egui::Context) {
        if !self.perf.visible {
            return;
        }
        let stats = self.tabs[self.active].chart.stats();
        let eps = self.perf.events.per_second(Instant::now());
        let p = &self.perf;
        let frame = egui::Frame::popup(&ctx.global_style()).fill(self.theme.panel_alt);
        egui::Area::new(egui::Id::unique("perf_overlay"))
            .anchor(egui::Align2::RIGHT_TOP, vec2(-90.0, 96.0))
            .interactable(false)
            .show(ctx, |ui| {
                frame.show(ui, |ui| {
                    ui.label(RichText::new("Performance (F12)").strong());
                    egui::Grid::new("perf_grid").num_columns(2).show(ui, |ui| {
                        let ms = |s: &crate::perf::Samples| {
                            format!("{:6.2} avg  {:6.2} max ms", s.avg(), s.max())
                        };
                        let row = |ui: &mut egui::Ui, k: &str, v: String| {
                            ui.label(RichText::new(k).weak());
                            ui.label(RichText::new(v).monospace());
                            ui.end_row();
                        };
                        row(ui, "Frame CPU", ms(&p.frame));
                        row(ui, "UI build", ms(&p.ui));
                        row(ui, "Chart draw", ms(&p.chart));
                        row(ui, "Visible bars", stats.visible_bars.to_string());
                        row(ui, "Total bars", stats.total_bars.to_string());
                        row(ui, "Events/s", eps.to_string());
                    });
                    ui.label(
                        RichText::new(format!(
                            "last {} frames; repainting continuously",
                            p.frame.len()
                        ))
                        .small()
                        .weak(),
                    );
                });
            });
        // Keep measuring while the overlay is open.
        ctx.request_repaint();
    }
}

fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// A stable badge color per symbol, from the series palette.
fn badge_color(theme: &Theme, symbol: &str) -> Color32 {
    let h = symbol
        .bytes()
        .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    theme.series_palette[h as usize % theme.series_palette.len()]
}

/// A color picker with a "reset to palette default" button.
fn color_row(ui: &mut egui::Ui, label: &str, value: &mut Option<Color32>, current: Color32) {
    ui.horizontal(|ui| {
        ui.label(label);
        let mut c = value.unwrap_or(current);
        if egui::color_picker::color_edit_button_srgba(
            ui,
            &mut c,
            egui::color_picker::Alpha::Opaque,
        )
        .changed()
        {
            *value = Some(c);
        }
        if value.is_some() && ui.small_button("Reset").clicked() {
            *value = None;
        }
    });
}

impl eframe::App for TradeTermApp {
    fn on_exit(&mut self) {
        let snapshot = self.settings_snapshot();
        self.settings_store.flush(&snapshot);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.theme.bg.to_normalized_gamma_f32()
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_hotkeys(ctx);
        let shared = self.shared.clone();
        let events: u32 = self.tabs.iter_mut().map(|t| t.drain_events(&shared)).sum();
        self.perf.events.add(Instant::now(), events);
        self.theme = tt_theme::current(ctx, self.overrides);

        let snapshot = self.settings_snapshot();
        if let Some(wait) = self.settings_store.tick(&snapshot, Instant::now()) {
            ctx.request_repaint_after(wait);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ui_started = Instant::now();
        if let Some(cpu) = frame.info().cpu_usage {
            self.perf.frame.push(cpu * 1000.0);
        }
        // Keep everything on-screen when maximized without OS decorations.
        let inset = chrome::maximized_overhang(ui.ctx());
        let content = ui.max_rect().shrink(inset);
        ui.scope_builder(egui::UiBuilder::new().max_rect(content), |ui| {
            self.window_contents(ui);
        });

        let ctx = ui.ctx().clone();
        self.perf_overlay(&ctx);
        // Last, so they sit on top of everything else.
        chrome::resize_handles(ui);
        chrome::window_border(&ctx, &self.theme);
        self.perf
            .ui
            .push(ui_started.elapsed().as_secs_f32() * 1000.0);
    }
}

impl TradeTermApp {
    /// Title bar, toolbar, status bar and the active chart.
    fn window_contents(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("title_bar")
            .exact_size(TITLE_BAR_HEIGHT)
            .frame(egui::Frame::new().fill(self.theme.chrome))
            .show_separator_line(false)
            .show(ui, |ui| self.title_bar(ui));

        let bar = self
            .theme
            .bar_frame()
            .stroke(Stroke::NONE)
            .inner_margin(egui::Margin::symmetric(10, 5));
        let toolbar_frame = bar.inner_margin(egui::Margin::symmetric(8, 4));
        egui::Panel::top("toolbar")
            .frame(toolbar_frame)
            .show_separator_line(true)
            .show(ui, |ui| self.toolbar(ui));

        egui::Panel::bottom("status_bar")
            .frame(bar)
            .show_separator_line(true)
            .show(ui, |ui| self.status_bar(ui));

        let theme = self.theme.clone();
        let shared = self.shared.clone();
        let tab = &mut self.tabs[self.active];
        let title = format!(
            "{} · {} · {}",
            tab.display_symbol(),
            tab.timeframe,
            tab.source_name(&shared)
        );
        let tick_size = tab.symbol_info.as_ref().map(|i| i.tick_size);
        // Live tabs show the bar-close countdown, which needs a tick per second.
        let live = tab.synthetic.is_none();
        let now_ms = live.then(now_unix_ms);
        if live {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(
                    1000 - (now_unix_ms().rem_euclid(1000)) as u64,
                ));
        }
        let bar_interval_ms = Some(tab.timeframe.duration_ms());
        let out = egui::CentralPanel::no_frame()
            .show(ui, |ui| {
                let data = ChartData {
                    series: &tab.series,
                    title: &title,
                    tick_size,
                    now_ms,
                    bar_interval_ms,
                };
                tab.chart.show(ui, data, &theme)
            })
            .inner;
        if out.needs_older_history {
            tab.request_backfill(&shared);
        }
        self.perf
            .chart
            .push(tab.chart.stats().draw_time.as_secs_f32() * 1000.0);
    }
}

impl Drop for TradeTermApp {
    fn drop(&mut self) {
        let snapshot = self.settings_snapshot();
        self.settings_store.flush(&snapshot);
        // Abort every stream before the runtime field is dropped.
        self.tabs.clear();
    }
}
