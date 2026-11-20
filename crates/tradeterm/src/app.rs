use crate::runtime::BackgroundRuntime;
use crate::theme::{self, palette};
use crossbeam_channel::Receiver;
use egui::{Align, Layout, RichText, Sense, TextEdit, vec2};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{error, info};
use tt_brokers::{BinanceSource, MarketDataSource};
use tt_chart::ChartView;
use tt_core::{CandleSeries, ConnectionStatus, EventSink, MarketEvent, Symbol, Timeframe};

const DEFAULT_SYMBOL: &str = "BTCUSDT";

pub struct TradeTermApp {
    ctx: egui::Context,
    // Declared before `runtime` so the task handle is dropped first.
    stream: Option<JoinHandle<()>>,
    source: Arc<BinanceSource>,
    rx: Receiver<MarketEvent>,

    symbol: Symbol,
    symbol_input: String,
    timeframe: Timeframe,

    series: CandleSeries,
    status: ConnectionStatus,
    chart: ChartView,

    runtime: BackgroundRuntime,
}

impl TradeTermApp {
    pub fn new(ctx: egui::Context, runtime: BackgroundRuntime) -> anyhow::Result<Self> {
        let source = Arc::new(BinanceSource::new()?);
        let (_, rx) = crossbeam_channel::unbounded();
        let mut app = Self {
            ctx,
            stream: None,
            source,
            rx,
            symbol: Symbol::new(DEFAULT_SYMBOL),
            symbol_input: DEFAULT_SYMBOL.to_owned(),
            timeframe: Timeframe::M1,
            series: CandleSeries::new(),
            status: ConnectionStatus::Connecting,
            chart: ChartView::new(theme::chart_style()),
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
        // in-flight sends from the old task fail, so stale data can't leak in.
        let (tx, rx) = crossbeam_channel::unbounded();
        self.rx = rx;
        self.series.clear();
        self.status = ConnectionStatus::Connecting;
        self.chart.reset_view();

        let ctx = self.ctx.clone();
        let sink = EventSink::new(tx, move || ctx.request_repaint());
        let source = self.source.clone();
        let (symbol, tf) = (self.symbol.clone(), self.timeframe);
        info!(%symbol, %tf, source = source.name(), "starting stream");

        self.stream = Some(self.runtime.handle().spawn(async move {
            if let Err(e) = source.run(symbol, tf, sink).await {
                error!("market data stream failed: {e:#}");
            }
        }));
        self.ctx.request_repaint();
    }

    /// Applies every pending event. Called once per frame; never blocks.
    fn drain_events(&mut self) {
        for event in self.rx.try_iter() {
            match event {
                MarketEvent::History(candles) => self.series.set_history(candles),
                MarketEvent::CandleUpdate { candle, .. } => {
                    self.series.upsert(candle);
                }
                MarketEvent::Status(s) => self.status = s,
            }
        }
    }

    fn apply_symbol_input(&mut self) {
        let sym = Symbol::new(&self.symbol_input);
        if sym.is_valid() && sym != self.symbol {
            self.symbol = sym;
            self.restart_stream();
        }
        self.symbol_input = self.symbol.to_string();
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("tradeterm")
                    .strong()
                    .color(palette::TEXT_STRONG),
            );
            ui.separator();

            let edit = ui.add(
                TextEdit::singleline(&mut self.symbol_input)
                    .desired_width(110.0)
                    .hint_text("Symbol")
                    .char_limit(20),
            );
            if edit.changed() {
                self.symbol_input = self
                    .symbol_input
                    .chars()
                    .filter(char::is_ascii_alphanumeric)
                    .map(|c| c.to_ascii_uppercase())
                    .collect();
            }
            // Enter (or clicking away) applies the symbol; invalid input reverts.
            if edit.lost_focus() {
                self.apply_symbol_input();
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
        });
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (color, text) = match &self.status {
                ConnectionStatus::Connected => (palette::UP, self.status.to_string()),
                ConnectionStatus::Connecting => (palette::WARN, self.status.to_string()),
                ConnectionStatus::Disconnected { .. } => {
                    (palette::DOWN, format!("{} (retrying)", self.status))
                }
            };
            let (dot, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
            ui.painter().circle_filled(dot.center(), 4.0, color);
            ui.label(RichText::new(text).color(palette::TEXT));

            ui.separator();
            match self.series.last() {
                Some(c) => {
                    let d = tt_chart::scale::price_decimals(c.close);
                    let color = if c.is_bullish() {
                        palette::UP
                    } else {
                        palette::DOWN
                    };
                    ui.label(RichText::new("Last").color(palette::TEXT_WEAK));
                    ui.label(
                        RichText::new(format!("{:.d$}", c.close))
                            .monospace()
                            .color(color),
                    );
                }
                None => {
                    ui.label(RichText::new("Last —").color(palette::TEXT_WEAK));
                }
            }

            ui.separator();
            ui.label(RichText::new("Bars").color(palette::TEXT_WEAK));
            ui.label(RichText::new(self.series.len().to_string()).monospace());

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{} · UTC", self.source.name()))
                        .color(palette::TEXT_WEAK),
                );
            });
        });
    }
}

impl eframe::App for TradeTermApp {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("top_bar")
            .frame(theme::bar_frame())
            .show_separator_line(false)
            .show(ui, |ui| self.top_bar(ui));

        egui::Panel::bottom("status_bar")
            .frame(theme::bar_frame())
            .show_separator_line(false)
            .show(ui, |ui| self.status_bar(ui));

        let title = format!(
            "{} · {} · {}",
            self.symbol,
            self.timeframe,
            self.source.name()
        );
        egui::CentralPanel::no_frame().show(ui, |ui| {
            self.chart.show(ui, &self.series, &title);
        });
    }
}

impl Drop for TradeTermApp {
    fn drop(&mut self) {
        if let Some(task) = self.stream.take() {
            task.abort();
        }
    }
}
