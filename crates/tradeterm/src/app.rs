use crate::perf::PerfStats;
use crate::runtime::BackgroundRuntime;
use crate::search::{SearchOutcome, SymbolSearch};
use crate::settings::{AppSettings, SettingsStore};
use crossbeam_channel::Receiver;
use egui::{Align, Color32, Key, KeyboardShortcut, Layout, Modifiers, RichText, Sense, vec2};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;
use tracing::{error, info, warn};
use tt_brokers::binance::MAX_KLINES;
use tt_brokers::{BinanceSource, MarketDataSource};
use tt_chart::{ChartData, ChartType, ChartView, ScaleMode};
use tt_core::{
    CandleSeries, ConnectionStatus, EventSink, MarketEvent, Symbol, SymbolInfo, Timeframe,
};
use tt_theme::{Theme, ThemeMode, ThemeOverrides};

const TOGGLE_THEME: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::T);
/// Backfill stops once this many bars are loaded, so zooming far out can't
/// page through years of 1m data.
const MAX_HISTORY_BARS: usize = 250_000;
/// Wait before retrying a failed backfill request.
const BACKFILL_RETRY: Duration = Duration::from_secs(5);

/// Loading of older history when the user scrolls back.
#[derive(Debug, Default)]
struct Backfill {
    loading: bool,
    /// The venue has no bars older than what we hold.
    exhausted: bool,
    retry_at: Option<Instant>,
}

pub struct TradeTermApp {
    ctx: egui::Context,
    // Declared before `runtime` so task handles are dropped first.
    stream: Option<JoinHandle<()>>,
    source: Arc<BinanceSource>,
    rx: Receiver<MarketEvent>,
    /// Sink of the current stream, for side requests (symbol info, backfill).
    sink: Option<EventSink>,

    symbol: Symbol,
    symbol_info: Option<SymbolInfo>,
    search: SymbolSearch,
    timeframe: Timeframe,

    series: CandleSeries,
    status: ConnectionStatus,
    backfill: Backfill,
    chart: ChartView,

    theme_mode: ThemeMode,
    overrides: ThemeOverrides,
    /// Resolved palette for the current frame.
    theme: Theme,

    settings_store: SettingsStore,
    perf: PerfStats,
    runtime: BackgroundRuntime,
}

impl TradeTermApp {
    pub fn new(
        ctx: egui::Context,
        runtime: BackgroundRuntime,
        settings: AppSettings,
        settings_store: SettingsStore,
    ) -> anyhow::Result<Self> {
        let source = Arc::new(BinanceSource::new()?);
        let (_, rx) = crossbeam_channel::unbounded();
        let theme_mode = settings.theme_mode;
        let overrides = settings.overrides();
        let mut symbol = Symbol::new(&settings.last_symbol);
        if !symbol.is_valid() {
            symbol = Symbol::new(AppSettings::default().last_symbol);
        }
        let mut chart = ChartView::default();
        chart.chart_type = settings.chart_type;
        chart.scale_mode = settings.scale_mode;
        chart.show_volume = settings.show_volume;
        tt_theme::install(&ctx, theme_mode, overrides);
        let theme = tt_theme::current(&ctx, overrides);

        // Warm the symbol list so the first search is instant.
        let warm = source.clone();
        runtime.handle().spawn(async move {
            if let Err(e) = warm.search_symbols("").await {
                warn!("could not preload symbol list: {e:#}");
            }
        });

        let mut app = Self {
            ctx,
            stream: None,
            source,
            rx,
            sink: None,
            search: SymbolSearch::new(symbol.as_str()),
            symbol,
            symbol_info: None,
            timeframe: settings.last_timeframe,
            series: CandleSeries::new(),
            status: ConnectionStatus::Connecting,
            backfill: Backfill::default(),
            chart,
            theme_mode,
            overrides,
            theme,
            settings_store,
            perf: PerfStats::default(),
            runtime,
        };
        app.restart_stream();
        Ok(app)
    }

    /// Cancels the current stream and starts one for the current symbol/timeframe.
    fn restart_stream(&mut self) {
        if let Some(task) = self.stream.take() {
            task.abort();
        }
        // A fresh channel per stream: dropping the old receiver makes any
        // in-flight sends from old tasks fail, so stale data can't leak in.
        let (tx, rx) = crossbeam_channel::unbounded();
        self.rx = rx;
        self.series.clear();
        self.status = ConnectionStatus::Connecting;
        self.backfill = Backfill::default();
        self.symbol_info = None;
        self.chart.reset_view();

        let ctx = self.ctx.clone();
        let sink = EventSink::new(tx, move || ctx.request_repaint());
        self.sink = Some(sink.clone());
        let (symbol, tf) = (self.symbol.clone(), self.timeframe);
        info!(%symbol, %tf, source = self.source.name(), "starting stream");

        // Metadata in parallel with the stream; the chart works without it.
        let source = self.source.clone();
        let (info_sink, info_symbol) = (sink.clone(), symbol.clone());
        self.runtime.handle().spawn(async move {
            match source.symbol_info(&info_symbol).await {
                Ok(info) => {
                    info_sink.send(MarketEvent::SymbolInfo(info));
                }
                Err(e) => warn!(symbol = %info_symbol, "symbol info unavailable: {e:#}"),
            }
        });

        let source = self.source.clone();
        self.stream = Some(self.runtime.handle().spawn(async move {
            if let Err(e) = source.run(symbol, tf, sink).await {
                error!("market data stream failed: {e:#}");
            }
        }));
        self.ctx.request_repaint();
    }

    /// Requests the page of bars before the oldest one we hold.
    fn request_backfill(&mut self) {
        let b = &self.backfill;
        let cooling = b.retry_at.is_some_and(|t| Instant::now() < t);
        if b.loading || b.exhausted || cooling || self.series.len() >= MAX_HISTORY_BARS {
            return;
        }
        let (Some(sink), Some(&oldest)) = (self.sink.clone(), self.series.open_times().first())
        else {
            return;
        };
        self.backfill.loading = true;
        let source = self.source.clone();
        let (symbol, tf) = (self.symbol.clone(), self.timeframe);
        info!(%symbol, %tf, before = oldest, "loading older history");
        self.runtime.handle().spawn(async move {
            let event = match source
                .fetch_history(&symbol, tf, Some(oldest), MAX_KLINES)
                .await
            {
                Ok(candles) => MarketEvent::OlderHistory {
                    exhausted: candles.len() < MAX_KLINES,
                    candles,
                },
                Err(e) => {
                    warn!("backfill failed: {e:#}");
                    MarketEvent::OlderHistory {
                        candles: Vec::new(),
                        exhausted: false,
                    }
                }
            };
            sink.send(event);
        });
    }

    /// Applies every pending event. Called once per frame; never blocks.
    fn drain_events(&mut self) {
        let mut count = 0;
        for event in self.rx.try_iter() {
            count += 1;
            match event {
                // Reconnects deliver recent history; keep backfilled bars before it.
                MarketEvent::History(candles) => self.series.replace_tail(candles),
                MarketEvent::CandleUpdate { candle, .. } => {
                    self.series.upsert(candle);
                }
                MarketEvent::Status(s) => self.status = s,
                MarketEvent::SymbolInfo(info) => self.symbol_info = Some(info),
                MarketEvent::OlderHistory { candles, exhausted } => {
                    self.backfill.loading = false;
                    if candles.is_empty() && !exhausted {
                        self.backfill.retry_at = Some(Instant::now() + BACKFILL_RETRY);
                        self.ctx.request_repaint_after(BACKFILL_RETRY);
                    } else {
                        let added = self.series.prepend(candles);
                        self.backfill.exhausted = exhausted || added == 0;
                    }
                }
            }
        }
        self.perf.events.add(Instant::now(), count);
    }

    /// Current persisted state, derived from the live app state.
    fn settings_snapshot(&self) -> AppSettings {
        let mut s = AppSettings {
            theme_mode: self.theme_mode,
            last_symbol: self.symbol.to_string(),
            last_timeframe: self.timeframe,
            chart_type: self.chart.chart_type,
            scale_mode: self.chart.scale_mode,
            show_volume: self.chart.show_volume,
            ..AppSettings::default()
        };
        s.set_overrides(self.overrides);
        s
    }

    fn set_theme(&mut self, mode: ThemeMode, overrides: ThemeOverrides) {
        self.theme_mode = mode;
        self.overrides = overrides;
        tt_theme::install(&self.ctx, mode, overrides);
    }

    fn handle_hotkeys(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_shortcut(&TOGGLE_THEME)) {
            let next = if ctx.theme() == egui::Theme::Dark {
                ThemeMode::Light
            } else {
                ThemeMode::Dark
            };
            self.set_theme(next, self.overrides);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F12)) {
            self.perf.visible = !self.perf.visible;
        }
    }

    fn perf_overlay(&mut self, ctx: &egui::Context) {
        if !self.perf.visible {
            return;
        }
        let stats = self.chart.stats();
        let eps = self.perf.events.per_second(Instant::now());
        let p = &self.perf;
        let frame = egui::Frame::popup(&ctx.global_style()).fill(self.theme.panel_alt);
        egui::Area::new(egui::Id::unique("perf_overlay"))
            .anchor(egui::Align2::RIGHT_TOP, vec2(-90.0, 52.0))
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

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("tradeterm").strong());
            ui.separator();

            self.search.poll();
            let source = self.source.clone();
            let handle = self.runtime.handle().clone();
            let ctx = self.ctx.clone();
            let outcome = self.search.ui(ui, move |query, reply| {
                handle.spawn(async move {
                    let result = source
                        .search_symbols(&query)
                        .await
                        .map_err(|e| format!("{e:#}"));
                    let _ = reply.send((query, result));
                    ctx.request_repaint();
                });
            });
            match outcome {
                Some(SearchOutcome::Submit(text)) => {
                    let sym = Symbol::new(&text);
                    if sym.is_valid() && sym != self.symbol {
                        self.symbol = sym;
                        self.restart_stream();
                    }
                    self.search.input = self.symbol.to_string();
                }
                Some(SearchOutcome::Cancel) => self.search.input = self.symbol.to_string(),
                None => {}
            }

            ui.separator();
            for tf in Timeframe::ALL {
                if ui
                    .selectable_label(self.timeframe == tf, tf.label())
                    .clicked()
                    && self.timeframe != tf
                {
                    self.timeframe = tf;
                    self.restart_stream();
                }
            }

            ui.separator();
            egui::ComboBox::from_id_salt("chart_type")
                .selected_text(self.chart.chart_type.label())
                .width(130.0)
                .show_ui(ui, |ui| {
                    for t in ChartType::ALL {
                        ui.selectable_value(&mut self.chart.chart_type, t, t.label());
                    }
                });
            ui.toggle_value(&mut self.chart.show_volume, "Vol")
                .on_hover_text("Show the volume pane");

            ui.separator();
            for mode in ScaleMode::ALL {
                ui.selectable_value(&mut self.chart.scale_mode, mode, mode.label())
                    .on_hover_text(match mode {
                        ScaleMode::Linear => "Linear price scale",
                        ScaleMode::Logarithmic => "Logarithmic price scale",
                        ScaleMode::Percent => "Percent change from the first visible bar",
                    });
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.menu_button("⚙ Settings", |ui| self.settings_menu(ui));
            });
        });
    }

    fn settings_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(220.0);
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
    }

    fn price_precision(&self) -> usize {
        match &self.symbol_info {
            Some(i) => i.price_precision,
            None => self
                .series
                .last()
                .map_or(2, |c| tt_chart::scale::price_decimals(c.close)),
        }
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        let t = &self.theme;
        ui.horizontal(|ui| {
            let (color, text) = match &self.status {
                ConnectionStatus::Connected => (t.up, self.status.to_string()),
                ConnectionStatus::Connecting => (t.accent, self.status.to_string()),
                ConnectionStatus::Disconnected { .. } => {
                    (t.down, format!("{} (retrying)", self.status))
                }
            };
            let (dot, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
            ui.painter().circle_filled(dot.center(), 4.0, color);
            ui.label(text);

            ui.separator();
            match self.series.last() {
                Some(c) => {
                    let d = self.price_precision();
                    let color = if c.is_bullish() { t.up } else { t.down };
                    ui.label(RichText::new("Last").weak());
                    ui.label(
                        RichText::new(format!("{:.d$}", c.close))
                            .monospace()
                            .color(color),
                    );
                }
                None => {
                    ui.label(RichText::new("Last —").weak());
                }
            }

            ui.separator();
            ui.label(RichText::new("Bars").weak());
            ui.label(RichText::new(self.series.len().to_string()).monospace());
            if self.backfill.loading {
                ui.label(RichText::new("loading older…").weak());
            } else if self.backfill.exhausted {
                ui.label(RichText::new("(full history)").weak());
            } else if self.series.len() >= MAX_HISTORY_BARS {
                ui.label(RichText::new("(history limit)").weak());
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let tz = self
                    .symbol_info
                    .as_ref()
                    .map_or("UTC", |i| i.timezone.as_str());
                ui.label(RichText::new(format!("{} · {tz}", self.source.name())).weak());
                if let Some(i) = &self.symbol_info {
                    ui.separator();
                    ui.label(
                        RichText::new(format!("{}/{}  tick {}", i.base, i.quote, i.tick_size))
                            .weak(),
                    );
                }
            });
        });
    }
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

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_hotkeys(ctx);
        self.drain_events();
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
        let bar = self.theme.bar_frame();
        egui::Panel::top("top_bar")
            .frame(bar)
            .show_separator_line(false)
            .show(ui, |ui| self.top_bar(ui));

        egui::Panel::bottom("status_bar")
            .frame(bar)
            .show_separator_line(false)
            .show(ui, |ui| self.status_bar(ui));

        let title = format!(
            "{} · {} · {}",
            self.symbol,
            self.timeframe,
            self.source.name()
        );
        let theme = self.theme.clone();
        let tick_size = self.symbol_info.as_ref().map(|i| i.tick_size);
        let out = egui::CentralPanel::no_frame()
            .show(ui, |ui| {
                let data = ChartData {
                    series: &self.series,
                    title: &title,
                    tick_size,
                };
                self.chart.show(ui, data, &theme)
            })
            .inner;
        if out.needs_older_history {
            self.request_backfill();
        }
        self.perf
            .chart
            .push(self.chart.stats().draw_time.as_secs_f32() * 1000.0);

        let ctx = ui.ctx().clone();
        self.perf_overlay(&ctx);
        self.perf
            .ui
            .push(ui_started.elapsed().as_secs_f32() * 1000.0);
    }
}

impl Drop for TradeTermApp {
    fn drop(&mut self) {
        let snapshot = self.settings_snapshot();
        self.settings_store.flush(&snapshot);
        if let Some(task) = self.stream.take() {
            task.abort();
        }
    }
}
