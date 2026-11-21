//! Alerts in the app (docs/specs/alerts.md): the [`AlertCenter`] entity
//! (alerts, log, `alerts.json`, the background [`AlertEngine`]) shared
//! through the [`AlertsGlobal`] global, notifications, and the glue to the
//! chart tabs: tab prices into the engine, alert lines on the charts,
//! chart events (alert line click / drag, the price scale's "+", "Add
//! alert" on a drawing).
//!
//! UI-thread cost per data wake-up: one pass over the tabs (a set lookup
//! per tab; a channel send when its symbol has alerts). Conditions are
//! evaluated on the engine's task.

use crate::app::{TabChart, TradeTerm};
use gpui_kit::component::WindowExt as _;
use gpui_kit::*;
use std::collections::HashSet;
use std::path::Path;
use tt_chart::alert_lines::AlertLine;
use tt_chart_gpui::ChartEvent;
use tt_core::Timeframe;
use tt_session::alert_engine::{AlertEngine, now_ms};
use tt_session::alerts::{
    Alert, AlertStatus, AlertsFile, Condition, FiredKind, LineKind, LineRef, LogEntry, Operator,
    Target,
};
use tt_session::fsutil::DebouncedFile;
use tt_session::tab::Shared;

/// The chart an alert is created from (defaults of a new alert).
#[derive(Debug, Clone, PartialEq)]
pub struct ChartContext {
    pub symbol: String,
    pub price: Option<f64>,
    pub interval: Timeframe,
    pub precision: usize,
}

/// Something the app should do for the alerts UI.
#[derive(Debug, Clone, PartialEq)]
pub enum AlertCenterEvent {
    /// Show this symbol on the active chart (a click on an alert or log
    /// row).
    ShowSymbol(String),
}

/// Alerts, their log and the engine. One per app, see [`center`].
pub struct AlertCenter {
    file: AlertsFile,
    writer: DebouncedFile<AlertsFile>,
    engine: AlertEngine,
    window: Option<AnyWindowHandle>,
    /// The active chart (set by the app on every data wake-up).
    pub chart: Option<ChartContext>,
    /// Bumped on every change (panel caches, chart lines).
    pub generation: u64,
    _quit: Subscription,
}

impl EventEmitter<AlertCenterEvent> for AlertCenter {}

/// The app's [`AlertCenter`].
pub struct AlertsGlobal(pub Entity<AlertCenter>);

impl Global for AlertsGlobal {}

/// The app's alert center (installed by the app window).
pub fn center(cx: &App) -> Option<Entity<AlertCenter>> {
    cx.try_global::<AlertsGlobal>().map(|g| g.0.clone())
}

impl AlertCenter {
    /// Loads `alerts.json` next to the settings, starts the engine and
    /// installs the global.
    pub fn install(
        settings_path: Option<&Path>,
        shared: &Shared,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let path = settings_path.map(AlertsFile::path_next_to);
        let file = AlertsFile::load(path.as_deref());
        let writer = DebouncedFile::new(path, std::time::Duration::ZERO, Some(&file));
        let mut engine =
            AlertEngine::start(&shared.handle, shared.source.clone(), shared.wake.clone());
        engine.set_alerts(&file.alerts);
        let handle = window.window_handle();
        let center = cx.new(|cx| Self {
            file,
            writer,
            engine,
            window: Some(handle),
            chart: None,
            generation: 0,
            _quit: cx.on_app_quit(|this: &mut Self, _| {
                this.writer.wait_idle();
                async {}
            }),
        });
        cx.set_global(AlertsGlobal(center.clone()));
        center
    }

    pub fn alerts(&self) -> &[Alert] {
        &self.file.alerts
    }

    pub fn alert(&self, id: u64) -> Option<&Alert> {
        self.file.alerts.iter().find(|a| a.id == id)
    }

    /// Oldest first.
    pub fn log(&self) -> &[LogEntry] {
        &self.file.log
    }

    /// Saves, re-arms the engine and repaints observers.
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        self.engine.set_alerts(&self.file.alerts);
        self.writer.mark_changed(std::time::Instant::now());
        self.writer.save(self.file.clone());
        cx.notify();
    }

    /// Adds (`id == 0`) or replaces an alert; returns its id. Saving an
    /// alert makes it active again (TradingView restarts edited alerts).
    pub fn upsert(&mut self, mut a: Alert, cx: &mut Context<Self>) -> u64 {
        a.status = AlertStatus::Active;
        if a.id == 0 {
            a.id = self.file.take_id();
            a.created_ms = now_ms();
            self.file.alerts.push(a.clone());
        } else if let Some(old) = self.file.alerts.iter_mut().find(|o| o.id == a.id) {
            *old = a.clone();
        } else {
            self.file.alerts.push(a.clone());
        }
        self.changed(cx);
        a.id
    }

    pub fn remove(&mut self, id: u64, cx: &mut Context<Self>) {
        self.file.alerts.retain(|a| a.id != id);
        self.changed(cx);
    }

    /// Stop (`false`) or restart (`true`).
    pub fn set_active(&mut self, id: u64, on: bool, cx: &mut Context<Self>) {
        if let Some(a) = self.file.alerts.iter_mut().find(|a| a.id == id) {
            a.status = if on {
                AlertStatus::Active
            } else {
                AlertStatus::Stopped
            };
            if on && a.expiration.is_some_and(|e| e <= now_ms()) {
                // Restarting an expired alert drops the past expiration.
                a.expiration = None;
            }
            self.changed(cx);
        }
    }

    /// A copy right after the original ("Clone").
    pub fn duplicate(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(i) = self.file.alerts.iter().position(|a| a.id == id) else {
            return;
        };
        let mut a = self.file.alerts[i].clone();
        a.id = self.file.take_id();
        a.created_ms = now_ms();
        a.status = AlertStatus::Active;
        a.last_triggered_ms = None;
        a.trigger_count = 0;
        self.file.alerts.insert(i + 1, a);
        self.changed(cx);
    }

    /// A dragged alert line: the level moves to `price` (rounded to the
    /// alert's decimals). Channels move the bound that was dragged
    /// (nearest), lines lose their drawing link.
    pub fn move_level(&mut self, id: u64, price: f64, from: f64, cx: &mut Context<Self>) {
        let Some(a) = self.file.alerts.iter_mut().find(|a| a.id == id) else {
            return;
        };
        let price = round_to(price, a.precision);
        // TradingView's automatic message follows the level.
        let auto = a.message == a.default_message();
        let c = &mut a.condition;
        match c.target {
            Target::Value(v) if c.op.is_channel() && (c.lower - from).abs() < (v - from).abs() => {
                c.lower = price
            }
            Target::Value(_) => c.target = Target::Value(price),
            Target::Line(l) if l.is_flat() => {
                c.target = Target::Line(LineRef {
                    drawing_id: 0,
                    ..LineRef::horizontal(0, price)
                })
            }
            Target::Line(_) => return,
        }
        if auto {
            a.message = a.default_message();
        }
        a.status = AlertStatus::Active;
        self.changed(cx);
    }

    pub fn restart_all_inactive(&mut self, cx: &mut Context<Self>) {
        let now = now_ms();
        for a in &mut self.file.alerts {
            if !a.status.is_active() {
                a.status = AlertStatus::Active;
                if a.expiration.is_some_and(|e| e <= now) {
                    a.expiration = None;
                }
            }
        }
        self.changed(cx);
    }

    pub fn stop_all(&mut self, cx: &mut Context<Self>) {
        for a in &mut self.file.alerts {
            if a.status.is_active() {
                a.status = AlertStatus::Stopped;
            }
        }
        self.changed(cx);
    }

    pub fn delete_all_inactive(&mut self, cx: &mut Context<Self>) {
        self.file.alerts.retain(|a| a.status.is_active());
        self.changed(cx);
    }

    pub fn clear_log(&mut self, cx: &mut Context<Self>) {
        self.file.log.clear();
        self.changed(cx);
    }

    /// Removes log entry `index` (oldest first).
    pub fn remove_log_entry(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.file.log.len() {
            self.file.log.remove(index);
            self.changed(cx);
        }
    }

    /// Asks the app to show `symbol`.
    pub fn show_symbol(&mut self, symbol: String, cx: &mut Context<Self>) {
        cx.emit(AlertCenterEvent::ShowSymbol(symbol));
    }

    /// Applies what the engine reported: statuses, the log, notifications.
    pub fn drain(&mut self, cx: &mut Context<Self>) {
        let fired = self.engine.drain();
        if fired.is_empty() {
            return;
        }
        let mut notes = Vec::new();
        for f in fired {
            let Some(a) = self.file.alerts.iter_mut().find(|a| a.id == f.alert_id) else {
                continue;
            };
            match f.kind {
                FiredKind::Expired => a.status = AlertStatus::Expired,
                FiredKind::Triggered { last } => {
                    a.last_triggered_ms = Some(f.time_ms);
                    a.trigger_count += 1;
                    if last {
                        a.status = AlertStatus::Triggered;
                    }
                    let entry = LogEntry {
                        alert_id: a.id,
                        time_ms: f.time_ms,
                        symbol: a.symbol.clone(),
                        title: a.title(),
                        message: f.message.clone(),
                        price: f.price,
                        precision: a.precision,
                    };
                    notes.push((a.notify, entry.clone()));
                    self.file.push_log(entry);
                }
            }
        }
        self.changed(cx);
        let Some(handle) = self.window else { return };
        if notes.iter().any(|(n, _)| n.sound) {
            play_sound();
        }
        cx.defer(move |cx| {
            handle
                .update(cx, |_, window, cx| {
                    for (notify, e) in notes {
                        if notify.toast {
                            let text = if e.message.is_empty() {
                                format!("Alert on {}", e.symbol)
                            } else {
                                format!("Alert on {}: {}", e.symbol, e.message)
                            };
                            window.push_notification(text, cx);
                        }
                        if notify.popup {
                            crate::alert_dialog::open_popup(e, window, cx);
                        }
                    }
                })
                .ok();
        });
    }
}

/// `price` rounded to `decimals`.
pub fn round_to(price: f64, decimals: usize) -> f64 {
    let f = 10f64.powi(decimals.min(12) as i32);
    (price * f).round() / f
}

/// A short system sound (Windows: the "asterisk" sound; elsewhere none).
pub fn play_sound() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
        use windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONASTERISK;
        // SAFETY: plain Win32 call without pointers.
        unsafe {
            MessageBeep(MB_ICONASTERISK);
        }
    }
}

/// A new alert on the active chart (as last reported to the alert center).
pub fn new_draft(price: Option<f64>, cx: &App) -> Alert {
    draft_for(center(cx).and_then(|c| c.read(cx).chart.clone()), price)
}

/// A new alert on `ctx`: Crossing at `price` (default: the last price),
/// the chart's interval, TradingView's defaults (Once only, 1 month).
fn draft_for(ctx: Option<ChartContext>, price: Option<f64>) -> Alert {
    let ctx = ctx.unwrap_or(ChartContext {
        symbol: "BTCUSDT".to_owned(),
        price: None,
        interval: Timeframe::M1,
        precision: 2,
    });
    let v = round_to(price.or(ctx.price).unwrap_or(0.0), ctx.precision);
    let mut a = Alert {
        symbol: ctx.symbol,
        interval: ctx.interval,
        precision: ctx.precision,
        condition: Condition::value(Operator::Crossing, v),
        expiration: tt_session::alerts::ExpiryPreset::OneMonth.resolve(now_ms()),
        ..Alert::default()
    };
    a.message = a.default_message();
    a
}

/// An alert that follows a line drawing (`None`: not a line tool).
pub fn line_ref(d: &tt_chart::Drawing) -> Option<LineRef> {
    use tt_chart::DrawingTool as T;
    let a = d.points.first()?;
    let (left, right) = match &d.props {
        tt_chart::Props::Line(p) => (p.extend.left, p.extend.right),
        _ => (false, false),
    };
    let line = match d.tool {
        T::HorizontalLine => LineRef::horizontal(d.id, a.price),
        T::HorizontalRay => LineRef::through(
            d.id,
            LineKind::HorizontalRay,
            (a.time_ms, a.price),
            (a.time_ms, a.price),
            false,
            true,
        ),
        T::TrendLine | T::Ray | T::ExtendedLine => {
            let b = d.points.get(1)?;
            let kind = match d.tool {
                T::Ray => LineKind::Ray,
                T::ExtendedLine => LineKind::ExtendedLine,
                _ => LineKind::TrendLine,
            };
            LineRef::through(
                d.id,
                kind,
                (a.time_ms, a.price),
                (b.time_ms, b.price),
                left,
                right,
            )
        }
        _ => return None,
    };
    Some(line)
}

impl TradeTerm {
    /// Installs the alert center and wires it to this window.
    pub(crate) fn init_alerts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path = self.settings_path();
        let center = AlertCenter::install(path.as_deref(), &self.shared, window, cx);
        let sub = cx.subscribe(&center, |app, _, ev: &AlertCenterEvent, cx| match ev {
            AlertCenterEvent::ShowSymbol(s) => {
                let current = app
                    .tab_chart()
                    .map(|c| c.read(cx).state.read(cx).source.tab.symbol.to_string());
                if current.as_deref() != Some(s.as_str()) {
                    app.set_symbol(tt_core::Symbol::new(s), None, cx);
                }
            }
        });
        self._subscriptions.push(sub);
        let obs = cx.observe(&center, |app, _, cx| app.sync_alert_lines(cx));
        self._subscriptions.push(obs);
        // Until the right sidebar mounts the panel: `TRADETERM_ALERTS_PANEL=1`
        // shows it in a window of its own (screenshots, testing).
        if std::env::var_os("TRADETERM_ALERTS_PANEL").is_some() {
            cx.defer(|cx| {
                let opts = WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(1460.), px(40.)),
                        size(px(tt_ui::tokens::AL_W_PANEL), px(820.)),
                    ))),
                    ..Default::default()
                };
                gpui_kit::open_window(opts, cx, |window, cx| {
                    cx.new(|cx| crate::alerts_panel::AlertsPanel::new(window, cx))
                })
                .ok();
            });
        }
    }

    /// After market data was applied: tab prices into the engine, the
    /// active chart's context, alert lines, fired alerts.
    pub(crate) fn alerts_after_drain(&mut self, cx: &mut Context<Self>) {
        let Some(center) = center(cx) else { return };
        let mut symbols = HashSet::new();
        let mut active = None;
        for (i, chart) in self.tabs.iter().enumerate() {
            let tab = &chart.read(cx).state.read(cx).source.tab;
            if tab.synthetic.is_some() {
                continue;
            }
            let symbol = tab.symbol.as_str();
            let price = tab.series.last().map(|c| c.close);
            if let Some(p) = price {
                center.read(cx).engine.tab_price(symbol, p);
            }
            symbols.insert(symbol.to_owned());
            if i == self.active {
                active = Some(ChartContext {
                    symbol: symbol.to_owned(),
                    price,
                    interval: tab.timeframe,
                    precision: tab.price_precision(),
                });
            }
        }
        center.update(cx, |c, cx| {
            c.engine.set_tab_symbols(symbols);
            c.chart = active;
            c.drain(cx);
        });
        self.sync_alert_lines(cx);
    }

    /// Puts each tab's alert levels on its chart (only when they changed).
    pub(crate) fn sync_alert_lines(&mut self, cx: &mut Context<Self>) {
        let Some(center) = center(cx) else { return };
        for chart in self.tabs.clone() {
            let lines: Vec<AlertLine> = {
                let tab = &chart.read(cx).state.read(cx).source.tab;
                if tab.synthetic.is_some() {
                    Vec::new()
                } else {
                    let symbol = tab.symbol.as_str();
                    center
                        .read(cx)
                        .alerts()
                        .iter()
                        .filter(|a| a.symbol == symbol)
                        .flat_map(|a| {
                            let active = a.status.is_active();
                            a.condition
                                .levels()
                                .into_iter()
                                .map(move |price| AlertLine {
                                    id: a.id,
                                    price,
                                    active,
                                })
                        })
                        .collect()
                }
            };
            let same = chart.read(cx).state.read(cx).source.tab.chart.alert_lines == lines;
            if !same {
                chart.update(cx, |c, cx| {
                    c.update_state(cx, |s| s.source.tab.chart.alert_lines = lines)
                });
            }
        }
    }

    /// Chart events of the alerts (line click / drag, "+", drawing alert).
    pub(crate) fn on_alert_chart_event(
        &mut self,
        chart: &TabChart,
        ev: &ChartEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(center) = center(cx) else { return };
        match *ev {
            ChartEvent::AlertClicked(id) => {
                if let Some(a) = center.read(cx).alert(id).cloned() {
                    crate::alert_dialog::AlertDialog::open(a, false, window, cx);
                }
            }
            ChartEvent::AlertMoved { id, from, price } => {
                center.update(cx, |c, cx| c.move_level(id, price, from, cx));
                // The engine's state changed; the lines follow from it.
                self.sync_alert_lines(cx);
            }
            ChartEvent::AddAlertAt(price) => {
                let draft = self.new_alert_draft(Some(price), cx);
                crate::alert_dialog::AlertDialog::open(draft, true, window, cx);
            }
            ChartEvent::AddDrawingAlert(drawing) => {
                let line = chart
                    .read(cx)
                    .state
                    .read(cx)
                    .source
                    .tab
                    .chart
                    .drawings
                    .items
                    .iter()
                    .find(|d| d.id == drawing)
                    .and_then(line_ref);
                if let Some(line) = line {
                    let mut draft = self.new_alert_draft(None, cx);
                    draft.condition = Condition {
                        op: Operator::Crossing,
                        target: Target::Line(line),
                        ..Condition::default()
                    };
                    draft.message = draft.default_message();
                    crate::alert_dialog::AlertDialog::open(draft, true, window, cx);
                }
            }
            _ => {}
        }
    }

    /// Drawings changed on `chart`: alerts that follow its line drawings
    /// take their new geometry.
    pub(crate) fn sync_drawing_alerts(&mut self, chart: &TabChart, cx: &mut Context<Self>) {
        let Some(center) = center(cx) else { return };
        let updates: Vec<(u64, LineRef)> = {
            let tab = &chart.read(cx).state.read(cx).source.tab;
            let symbol = tab.symbol.as_str();
            center
                .read(cx)
                .alerts()
                .iter()
                .filter(|a| a.symbol == symbol)
                .filter_map(|a| match a.condition.target {
                    Target::Line(l) if l.drawing_id != 0 => {
                        let d = tab
                            .chart
                            .drawings
                            .items
                            .iter()
                            .find(|d| d.id == l.drawing_id)?;
                        let new = line_ref(d)?;
                        (new != l).then_some((a.id, new))
                    }
                    _ => None,
                })
                .collect()
        };
        if updates.is_empty() {
            return;
        }
        center.update(cx, |c, cx| {
            for (id, line) in updates {
                if let Some(mut a) = c.alert(id).cloned() {
                    let auto = a.message == a.default_message();
                    a.condition.target = Target::Line(line);
                    if auto {
                        a.message = a.default_message();
                    }
                    let keep = a.status;
                    c.upsert(a, cx);
                    if !keep.is_active() {
                        c.set_active(id, false, cx);
                    }
                }
            }
        });
    }

    /// A new alert on the active chart: Crossing at `price` (default: the
    /// last price), the chart's interval, TradingView's defaults (Once
    /// only, 1 month).
    pub(crate) fn new_alert_draft(&self, price: Option<f64>, cx: &App) -> Alert {
        let ctx = self.tab_chart().map(|c| {
            let t = &c.read(cx).state.read(cx).source.tab;
            ChartContext {
                symbol: t.symbol.to_string(),
                price: t.series.last().map(|c| c.close),
                interval: t.timeframe,
                precision: t.price_precision(),
            }
        });
        draft_for(ctx, price)
    }

    /// Alt + A, the app bar's "Alert", the panel's "+".
    pub(crate) fn open_create_alert(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let draft = self.new_alert_draft(None, cx);
        crate::alert_dialog::AlertDialog::open(draft, true, window, cx);
    }
}
