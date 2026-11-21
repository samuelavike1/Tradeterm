//! Local price alerts, TradingView's "Create alert" on Price (docs/specs/alerts.md).
//!
//! An [`Alert`] watches one symbol: a [`Condition`] (operator + a value, a
//! channel or a drawn line), a [`Trigger`] frequency, an optional
//! expiration, a name, a message with `{{placeholders}}` and its
//! notifications. There is no server: alerts are evaluated while the app
//! runs, by the [`Evaluator`] on the alert engine's task
//! ([`crate::alert_engine`]), never on the UI thread.
//!
//! Evaluation is O(1) per tick per alert: the symbol's alerts are found by
//! a hash lookup and each keeps a fixed run state (previous price, current
//! bar, a ring of the last `bars` closes for the "Moving" operators).
//!
//! Persisted in `alerts.json` next to `settings.json` ([`AlertsFile`]):
//! loading is tolerant (a missing file is empty, an unparseable one is moved
//! aside, an alert this build can't read is kept as raw JSON and written
//! back).

use crate::fsutil;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use tracing::warn;
use tt_core::{TimeUnit, Timeframe};

/// File name, next to `settings.json`.
pub const ALERTS_FILE: &str = "alerts.json";
/// Current [`AlertsFile::version`].
pub const ALERTS_VERSION: u32 = 1;
/// Log entries kept (oldest dropped first).
pub const MAX_LOG: usize = 500;
/// Longest "Moving … in N bars" lookback.
pub const MAX_BARS: u32 = 1000;
const MINUTE_MS: i64 = 60_000;

// ---- model ------------------------------------------------------------------

/// TradingView's operators on "Price" (in its menu order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Operator {
    #[default]
    Crossing,
    CrossingUp,
    CrossingDown,
    GreaterThan,
    LessThan,
    EnteringChannel,
    ExitingChannel,
    InsideChannel,
    OutsideChannel,
    MovingUp,
    MovingDown,
    MovingUpPercent,
    MovingDownPercent,
}

impl Operator {
    pub const ALL: [Operator; 13] = [
        Operator::Crossing,
        Operator::CrossingUp,
        Operator::CrossingDown,
        Operator::GreaterThan,
        Operator::LessThan,
        Operator::EnteringChannel,
        Operator::ExitingChannel,
        Operator::InsideChannel,
        Operator::OutsideChannel,
        Operator::MovingUp,
        Operator::MovingDown,
        Operator::MovingUpPercent,
        Operator::MovingDownPercent,
    ];

    /// The three crossing operators: what a drawn line offers.
    pub const LINE: [Operator; 3] = [
        Operator::Crossing,
        Operator::CrossingUp,
        Operator::CrossingDown,
    ];

    /// TradingView's label.
    pub fn label(self) -> &'static str {
        match self {
            Operator::Crossing => "Crossing",
            Operator::CrossingUp => "Crossing Up",
            Operator::CrossingDown => "Crossing Down",
            Operator::GreaterThan => "Greater Than",
            Operator::LessThan => "Less Than",
            Operator::EnteringChannel => "Entering Channel",
            Operator::ExitingChannel => "Exiting Channel",
            Operator::InsideChannel => "Inside Channel",
            Operator::OutsideChannel => "Outside Channel",
            Operator::MovingUp => "Moving Up",
            Operator::MovingDown => "Moving Down",
            Operator::MovingUpPercent => "Moving Up %",
            Operator::MovingDownPercent => "Moving Down %",
        }
    }

    /// Upper and lower bound.
    pub fn is_channel(self) -> bool {
        matches!(
            self,
            Operator::EnteringChannel
                | Operator::ExitingChannel
                | Operator::InsideChannel
                | Operator::OutsideChannel
        )
    }

    /// An amount within a number of bars.
    pub fn is_moving(self) -> bool {
        matches!(
            self,
            Operator::MovingUp
                | Operator::MovingDown
                | Operator::MovingUpPercent
                | Operator::MovingDownPercent
        )
    }

    /// The first three (TradingView shows them before "Show more").
    pub fn is_basic(self) -> bool {
        Self::LINE.contains(&self)
    }
}

/// A drawn line as a function of time: horizontal line / ray, trend line,
/// ray, extended line. Points are kept in time order.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LineRef {
    /// The drawing it follows (0: none).
    pub drawing_id: u64,
    pub t1: i64,
    pub p1: f64,
    pub t2: i64,
    pub p2: f64,
    pub extend_left: bool,
    pub extend_right: bool,
    /// TradingView's name for the drawing ("Horizontal Line", "Trend Line").
    pub kind: LineKind,
}

impl Default for LineRef {
    fn default() -> Self {
        Self::horizontal(0, 0.0)
    }
}

/// Which drawing a [`LineRef`] came from (labels only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LineKind {
    #[default]
    HorizontalLine,
    HorizontalRay,
    TrendLine,
    Ray,
    ExtendedLine,
}

impl LineKind {
    pub fn label(self) -> &'static str {
        match self {
            LineKind::HorizontalLine => "Horizontal Line",
            LineKind::HorizontalRay => "Horizontal Ray",
            LineKind::TrendLine => "Trend Line",
            LineKind::Ray => "Ray",
            LineKind::ExtendedLine => "Extended Line",
        }
    }
}

impl LineRef {
    /// A horizontal line at `price` (any time).
    pub fn horizontal(drawing_id: u64, price: f64) -> Self {
        Self {
            drawing_id,
            t1: 0,
            p1: price,
            t2: 0,
            p2: price,
            extend_left: true,
            extend_right: true,
            kind: LineKind::HorizontalLine,
        }
    }

    /// A line through two points, sorted by time. `extend_left` /
    /// `extend_right` are screen directions (earlier / later in time), as
    /// on the drawings.
    pub fn through(
        drawing_id: u64,
        kind: LineKind,
        (t1, p1): (i64, f64),
        (t2, p2): (i64, f64),
        extend_left: bool,
        extend_right: bool,
    ) -> Self {
        let (a, b) = if t2 < t1 {
            ((t2, p2), (t1, p1))
        } else {
            ((t1, p1), (t2, p2))
        };
        Self {
            drawing_id,
            t1: a.0,
            p1: a.1,
            t2: b.0,
            p2: b.1,
            extend_left,
            extend_right,
            kind,
        }
    }

    /// The line's price at `t`, `None` outside its time span.
    pub fn value_at(&self, t: i64) -> Option<f64> {
        if (t < self.t1 && !self.extend_left) || (t > self.t2 && !self.extend_right) {
            return None;
        }
        if self.t2 == self.t1 {
            return Some(self.p1);
        }
        let f = (t - self.t1) as f64 / (self.t2 - self.t1) as f64;
        Some(self.p1 + (self.p2 - self.p1) * f)
    }

    /// Flat everywhere (an on-chart alert line can be drawn for it).
    pub fn is_flat(&self) -> bool {
        self.p1 == self.p2
    }
}

/// What the price is compared with.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Target {
    /// A price (channel: the upper bound; moving: the amount).
    Value(f64),
    /// A drawn line.
    Line(LineRef),
}

impl Default for Target {
    fn default() -> Self {
        Target::Value(0.0)
    }
}

/// Operator and its operands.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Condition {
    pub op: Operator,
    pub target: Target,
    /// Channel operators: the lower bound.
    pub lower: f64,
    /// Moving operators: "in N bars".
    pub bars: u32,
}

impl Default for Condition {
    fn default() -> Self {
        Self {
            op: Operator::Crossing,
            target: Target::Value(0.0),
            lower: 0.0,
            bars: 1,
        }
    }
}

impl Condition {
    pub fn value(op: Operator, v: f64) -> Self {
        Self {
            op,
            target: Target::Value(v),
            ..Self::default()
        }
    }

    /// The target's price at `t` (a line may not span `t`).
    pub fn value_at(&self, t: i64) -> Option<f64> {
        match self.target {
            Target::Value(v) => Some(v),
            Target::Line(l) => l.value_at(t),
        }
    }

    /// `(lower, upper)` of a channel.
    fn bounds(&self, t: i64) -> Option<(f64, f64)> {
        let v = self.value_at(t)?;
        Some((v.min(self.lower), v.max(self.lower)))
    }

    /// Whether the condition holds for a move from `prev` to `cur` at time
    /// `t`; `reference` is the close `bars` bars ago (Moving operators).
    /// Crossings need a previous price and compare like Pine's
    /// `ta.crossover` / `ta.crossunder` (`prev <= v < cur`), so a jump over
    /// the level between two ticks (a gap) still counts.
    pub fn met(&self, prev: Option<f64>, cur: f64, t: i64, reference: Option<f64>) -> bool {
        let inside = |x: f64, (lo, hi): (f64, f64)| lo <= x && x <= hi;
        match self.op {
            Operator::Crossing | Operator::CrossingUp | Operator::CrossingDown => {
                let (Some(p), Some(v)) = (prev, self.value_at(t)) else {
                    return false;
                };
                let up = p <= v && cur > v;
                let down = p >= v && cur < v;
                match self.op {
                    Operator::CrossingUp => up,
                    Operator::CrossingDown => down,
                    _ => up || down,
                }
            }
            Operator::GreaterThan => self.value_at(t).is_some_and(|v| cur > v),
            Operator::LessThan => self.value_at(t).is_some_and(|v| cur < v),
            Operator::InsideChannel => self.bounds(t).is_some_and(|b| inside(cur, b)),
            Operator::OutsideChannel => self.bounds(t).is_some_and(|b| !inside(cur, b)),
            Operator::EnteringChannel => match (prev, self.bounds(t)) {
                (Some(p), Some(b)) => !inside(p, b) && inside(cur, b),
                _ => false,
            },
            Operator::ExitingChannel => match (prev, self.bounds(t)) {
                (Some(p), Some(b)) => inside(p, b) && !inside(cur, b),
                _ => false,
            },
            Operator::MovingUp
            | Operator::MovingDown
            | Operator::MovingUpPercent
            | Operator::MovingDownPercent => {
                let (Some(r), Target::Value(amount)) = (reference, self.target) else {
                    return false;
                };
                let change = match self.op {
                    Operator::MovingUp => cur - r,
                    Operator::MovingDown => r - cur,
                    Operator::MovingUpPercent if r != 0.0 => (cur - r) / r * 100.0,
                    Operator::MovingDownPercent if r != 0.0 => (r - cur) / r * 100.0,
                    _ => return false,
                };
                change >= amount
            }
        }
    }

    /// TradingView's condition text: "Crossing 57,667.32", "Entering
    /// Channel (Upper Bound: …, Lower Bound: …)", "Moving Up 1.00 in 1
    /// bar", "Crossing Trend Line".
    pub fn summary(&self, precision: usize) -> String {
        let price = |v: f64| tt_chart::scale::format_price(v, precision);
        let op = self.op.label();
        match self.target {
            Target::Line(l) => format!("{op} {}", l.kind.label()),
            Target::Value(v) if self.op.is_channel() => format!(
                "{op} (Upper Bound: {}, Lower Bound: {})",
                price(v.max(self.lower)),
                price(v.min(self.lower))
            ),
            Target::Value(v) if self.op.is_moving() => {
                let bars = self.bars.max(1);
                let unit = if bars == 1 { "bar" } else { "bars" };
                format!(
                    "{op} {} in {bars} {unit}",
                    tt_chart::scale::format_price(v, 2)
                )
            }
            Target::Value(v) => format!("{op} {}", price(v)),
        }
    }

    /// Prices to mark on the chart (value, or both channel bounds); none
    /// for Moving operators and sloped lines.
    pub fn levels(&self) -> Vec<f64> {
        match self.target {
            _ if self.op.is_moving() => Vec::new(),
            Target::Value(v) if self.op.is_channel() => vec![v, self.lower],
            Target::Value(v) => vec![v],
            Target::Line(l) if l.is_flat() => vec![l.p1],
            Target::Line(_) => Vec::new(),
        }
    }
}

/// How often an alert may fire (TradingView's Trigger).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Trigger {
    #[default]
    OnlyOnce,
    OncePerBar,
    OncePerBarClose,
    OncePerMinute,
}

impl Trigger {
    pub const ALL: [Trigger; 4] = [
        Trigger::OnlyOnce,
        Trigger::OncePerBar,
        Trigger::OncePerBarClose,
        Trigger::OncePerMinute,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Trigger::OnlyOnce => "Once only",
            Trigger::OncePerBar => "Once per bar",
            Trigger::OncePerBarClose => "Once per bar close",
            Trigger::OncePerMinute => "Once per minute",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Trigger::OnlyOnce => "Triggers once when condition is met",
            Trigger::OncePerBar => "Triggers once per bar when condition is met",
            Trigger::OncePerBarClose => "Triggers every time condition is met at bar close",
            Trigger::OncePerMinute => "Triggers once per minute while condition remains met",
        }
    }
}

/// Notifications (TradingView's toast, sound; plus a popup dialog).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Notify {
    /// A dialog with the message, until dismissed.
    pub popup: bool,
    /// A toast in the window corner.
    pub toast: bool,
    pub sound: bool,
}

impl Default for Notify {
    fn default() -> Self {
        Self {
            popup: false,
            toast: true,
            sound: false,
        }
    }
}

impl Notify {
    /// "Toasts, Sound" (the dialog's Notifications row).
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if self.popup {
            parts.push("Popup");
        }
        if self.toast {
            parts.push("Toasts");
        }
        if self.sound {
            parts.push("Sound");
        }
        if parts.is_empty() {
            "None".to_owned()
        } else {
            parts.join(", ")
        }
    }
}

/// Where an alert stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum AlertStatus {
    #[default]
    Active,
    /// Stopped by the user.
    Stopped,
    /// An "Once only" alert that fired.
    Triggered,
    /// Its expiration passed.
    Expired,
}

impl AlertStatus {
    pub fn is_active(self) -> bool {
        self == AlertStatus::Active
    }

    /// TradingView's status line.
    pub fn label(self) -> &'static str {
        match self {
            AlertStatus::Active => "Active",
            AlertStatus::Stopped => "Stopped manually",
            AlertStatus::Triggered => "Stopped — Triggered",
            AlertStatus::Expired => "Stopped — Expired",
        }
    }
}

/// One alert.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Alert {
    pub id: u64,
    /// Exchange symbol, e.g. `BTCUSDT`.
    pub symbol: String,
    /// Bar interval of the per-bar triggers and Moving operators
    /// (TradingView: the chart's interval when created).
    pub interval: Timeframe,
    pub condition: Condition,
    pub trigger: Trigger,
    /// Unix ms; `None`: open-ended.
    pub expiration: Option<i64>,
    /// Empty: the panel shows the condition.
    pub name: String,
    /// With `{{placeholders}}`.
    pub message: String,
    pub notify: Notify,
    pub status: AlertStatus,
    pub created_ms: i64,
    pub last_triggered_ms: Option<i64>,
    pub trigger_count: u32,
    /// Price decimals of the symbol (labels, messages).
    pub precision: usize,
}

impl Default for Alert {
    fn default() -> Self {
        Self {
            id: 0,
            symbol: String::new(),
            interval: Timeframe::M1,
            condition: Condition::default(),
            trigger: Trigger::OnlyOnce,
            expiration: None,
            name: String::new(),
            message: String::new(),
            notify: Notify::default(),
            status: AlertStatus::Active,
            created_ms: 0,
            last_triggered_ms: None,
            trigger_count: 0,
            precision: 2,
        }
    }
}

impl Alert {
    /// TradingView's default message: "BTCUSDT Crossing 57,667.32";
    /// Moving operators name the interval: "BTCUSDT, 30 Moving Up 1.00 in 1
    /// bar".
    pub fn default_message(&self) -> String {
        let c = self.condition.summary(self.precision);
        if self.condition.op.is_moving() {
            format!(
                "{}, {} {c}",
                self.symbol,
                interval_placeholder(self.interval)
            )
        } else {
            format!("{} {c}", self.symbol)
        }
    }

    /// Name, else the condition ("Crossing 57,667.32").
    pub fn title(&self) -> String {
        if self.name.trim().is_empty() {
            self.condition.summary(self.precision)
        } else {
            self.name.clone()
        }
    }

    /// The parts the evaluator depends on are equal (status, counters and
    /// text don't reset the run state).
    fn same_rule(&self, o: &Alert) -> bool {
        self.symbol == o.symbol
            && self.interval == o.interval
            && self.condition == o.condition
            && self.trigger == o.trigger
            && self.expiration == o.expiration
    }
}

/// A triggered alert in the Log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LogEntry {
    pub alert_id: u64,
    pub time_ms: i64,
    pub symbol: String,
    /// The alert's title when it fired.
    pub title: String,
    /// The formatted message.
    pub message: String,
    pub price: f64,
    pub precision: usize,
}

impl Default for LogEntry {
    fn default() -> Self {
        Self {
            alert_id: 0,
            time_ms: 0,
            symbol: String::new(),
            title: String::new(),
            message: String::new(),
            price: 0.0,
            precision: 2,
        }
    }
}

// ---- placeholders -------------------------------------------------------------

/// Values for [`format_message`].
#[derive(Debug, Clone, Copy)]
pub struct Placeholders<'a> {
    pub ticker: &'a str,
    pub exchange: &'a str,
    pub interval: Timeframe,
    /// Open time of the alert's current bar.
    pub time_ms: i64,
    pub now_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    /// `None`: not known (printed as `NaN`, like TradingView).
    pub volume: Option<f64>,
    pub precision: usize,
}

/// The placeholders TradingView's "Add placeholder" offers that we fill:
/// (label, text).
pub const PLACEHOLDERS: [(&str, &str); 10] = [
    ("Ticker", "{{ticker}}"),
    ("Exchange", "{{exchange}}"),
    ("Interval", "{{interval}}"),
    ("Time", "{{time}}"),
    ("Timenow", "{{timenow}}"),
    ("Open", "{{open}}"),
    ("Close", "{{close}}"),
    ("High", "{{high}}"),
    ("Low", "{{low}}"),
    ("Volume", "{{volume}}"),
];

/// TradingView's `{{interval}}`: minutes as a number ("1", "240"), else a
/// count and unit ("30S", "1D", "1W", "1M").
pub fn interval_placeholder(tf: Timeframe) -> String {
    let n = tf.count();
    match tf.unit() {
        TimeUnit::Second => format!("{n}S"),
        TimeUnit::Minute => n.to_string(),
        TimeUnit::Hour => (n * 60).to_string(),
        TimeUnit::Day => format!("{n}D"),
        TimeUnit::Week => format!("{n}W"),
        TimeUnit::Month => format!("{n}M"),
    }
}

/// UTC time like TradingView's `{{time}}`: `2019-08-27T09:56:00Z`.
pub fn format_utc(ms: i64) -> String {
    match jiff::Timestamp::from_millisecond(ms) {
        Ok(t) => t.strftime("%Y-%m-%dT%H:%M:%SZ").to_string(),
        Err(_) => String::new(),
    }
}

/// Replaces `{{name}}` placeholders (case-insensitive, spaces allowed
/// inside the braces); unknown ones stay as written.
pub fn format_message(template: &str, v: &Placeholders<'_>) -> String {
    let num = |x: f64| format!("{x:.p$}", p = v.precision);
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[start..]);
            return out;
        };
        let key = after[..end].trim().to_ascii_lowercase();
        let value = match key.as_str() {
            "ticker" => Some(v.ticker.to_owned()),
            "exchange" => Some(v.exchange.to_owned()),
            "interval" => Some(interval_placeholder(v.interval)),
            "time" => Some(format_utc(v.time_ms)),
            "timenow" => Some(format_utc(v.now_ms)),
            "open" => Some(num(v.open)),
            "high" => Some(num(v.high)),
            "low" => Some(num(v.low)),
            "close" => Some(num(v.close)),
            "volume" => Some(v.volume.map_or_else(|| "NaN".to_owned(), |x| x.to_string())),
            _ => None,
        };
        match value {
            Some(s) => out.push_str(&s),
            None => out.push_str(&rest[start..start + 2 + end + 2]),
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

// ---- expiration (local time) ------------------------------------------------

/// TradingView's expiration presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpiryPreset {
    OpenEnded,
    EndOfDay,
    OneWeek,
    OneMonth,
}

impl ExpiryPreset {
    pub const ALL: [ExpiryPreset; 4] = [
        ExpiryPreset::OpenEnded,
        ExpiryPreset::EndOfDay,
        ExpiryPreset::OneWeek,
        ExpiryPreset::OneMonth,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ExpiryPreset::OpenEnded => "Open-ended",
            ExpiryPreset::EndOfDay => "End of day",
            ExpiryPreset::OneWeek => "1 week",
            ExpiryPreset::OneMonth => "1 month",
        }
    }

    /// The expiration from `now_ms`, in the system time zone.
    pub fn resolve(self, now_ms: i64) -> Option<i64> {
        use jiff::ToSpan as _;
        let tz = jiff::tz::TimeZone::system();
        let now = jiff::Timestamp::from_millisecond(now_ms)
            .ok()?
            .to_zoned_datetime(tz.clone());
        let z = match self {
            ExpiryPreset::OpenEnded => return None,
            ExpiryPreset::EndOfDay => now.date().at(23, 59, 0, 0).to_zoned_datetime(tz).ok()?,
            ExpiryPreset::OneWeek => now.checked_add(1.week()).ok()?,
            ExpiryPreset::OneMonth => now.checked_add(1.month()).ok()?,
        };
        Some(z.timestamp().as_millisecond())
    }
}

/// "December 21, 2026 at 15:41" (system time zone), TradingView's
/// expiration text.
pub fn format_local_long(ms: i64) -> String {
    match jiff::Timestamp::from_millisecond(ms) {
        Ok(t) => t
            .to_zoned_datetime(jiff::tz::TimeZone::system())
            .strftime("%B %-d, %Y at %H:%M")
            .to_string(),
        Err(_) => String::new(),
    }
}

/// "Nov 21, 23:59" (system time zone): preset hints, log rows.
pub fn format_local_short(ms: i64) -> String {
    match jiff::Timestamp::from_millisecond(ms) {
        Ok(t) => t
            .to_zoned_datetime(jiff::tz::TimeZone::system())
            .strftime("%b %-d, %H:%M")
            .to_string(),
        Err(_) => String::new(),
    }
}

/// "2026-12-21 15:41" (system time zone), the custom date field.
pub fn format_local_input(ms: i64) -> String {
    match jiff::Timestamp::from_millisecond(ms) {
        Ok(t) => t
            .to_zoned_datetime(jiff::tz::TimeZone::system())
            .strftime("%Y-%m-%d %H:%M")
            .to_string(),
        Err(_) => String::new(),
    }
}

/// Parses "2026-12-21 15:41" (or "2026-12-21") in the system time zone.
pub fn parse_local_input(s: &str) -> Option<i64> {
    let s = s.trim();
    let dt = jiff::civil::DateTime::strptime("%Y-%m-%d %H:%M", s)
        .or_else(|_| jiff::civil::Date::strptime("%Y-%m-%d", s).map(|d| d.at(23, 59, 0, 0)))
        .ok()?;
    let z = dt.to_zoned_datetime(jiff::tz::TimeZone::system()).ok()?;
    Some(z.timestamp().as_millisecond())
}

// ---- evaluation ------------------------------------------------------------

/// What happened to an alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FiredKind {
    /// The condition fired; `last`: an "Once only" alert, now done.
    Triggered { last: bool },
    /// The expiration passed; the alert is done.
    Expired,
}

/// An alert event for the UI (log, notifications, status).
#[derive(Debug, Clone, PartialEq)]
pub struct Fired {
    pub alert_id: u64,
    pub kind: FiredKind,
    pub time_ms: i64,
    pub price: f64,
    /// The formatted message (empty for [`FiredKind::Expired`]).
    pub message: String,
}

/// Per-alert evaluation state (not persisted).
#[derive(Debug, Clone, Default)]
struct Run {
    /// Last price seen.
    prev: Option<f64>,
    /// Open time of the current bar (`alert.interval`).
    bar: Option<i64>,
    /// Open, high, low, close of the current bar (from the ticks seen).
    ohlc: [f64; 4],
    /// Close of the last finished bar.
    prev_close: Option<f64>,
    /// Closes of the last `bars` finished bars, oldest first.
    closes: VecDeque<f64>,
    fired_bar: Option<i64>,
    fired_ms: Option<i64>,
    /// Fired (once only) or expired: never evaluated again.
    done: bool,
}

/// Evaluates the active alerts on price ticks; see the module docs.
#[derive(Debug, Default)]
pub struct Evaluator {
    items: Vec<(Alert, Run)>,
    by_symbol: HashMap<String, Vec<usize>>,
}

impl Evaluator {
    /// Replaces the alert set with the active ones of `alerts`. An alert
    /// whose rule did not change keeps its state (no re-fire after an edit
    /// of its name, say).
    pub fn set_alerts(&mut self, alerts: impl IntoIterator<Item = Alert>) {
        let mut old: HashMap<u64, (Alert, Run)> =
            self.items.drain(..).map(|(a, r)| (a.id, (a, r))).collect();
        for a in alerts.into_iter().filter(|a| a.status.is_active()) {
            let run = match old.remove(&a.id) {
                Some((o, r)) if o.same_rule(&a) => r,
                _ => Run::default(),
            };
            self.items.push((a, run));
        }
        self.by_symbol.clear();
        for (i, (a, _)) in self.items.iter().enumerate() {
            self.by_symbol.entry(a.symbol.clone()).or_default().push(i);
        }
    }

    /// Symbols with alerts still to evaluate.
    pub fn symbols(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .by_symbol
            .iter()
            .filter(|(_, ids)| ids.iter().any(|&i| !self.items[i].1.done))
            .map(|(s, _)| s.clone())
            .collect();
        v.sort();
        v
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// A trade / last price of `symbol` at `now` (Unix ms).
    pub fn on_tick(&mut self, symbol: &str, price: f64, now: i64, out: &mut Vec<Fired>) {
        if !price.is_finite() {
            return;
        }
        let Some(ids) = self.by_symbol.get(symbol) else {
            return;
        };
        for &i in ids {
            let (alert, run) = &mut self.items[i];
            step(alert, run, Some(price), now, out);
        }
    }

    /// Clock tick (about once a second): expirations and bar closes
    /// without a trade.
    pub fn on_timer(&mut self, now: i64, out: &mut Vec<Fired>) {
        for (alert, run) in &mut self.items {
            step(alert, run, None, now, out);
        }
    }
}

/// One alert, one event: a tick (`Some(price)`) or the clock (`None`).
fn step(alert: &Alert, run: &mut Run, price: Option<f64>, now: i64, out: &mut Vec<Fired>) {
    if run.done {
        return;
    }
    if alert.expiration.is_some_and(|e| now >= e) {
        run.done = true;
        out.push(Fired {
            alert_id: alert.id,
            kind: FiredKind::Expired,
            time_ms: now,
            price: run.prev.unwrap_or(f64::NAN),
            message: String::new(),
        });
        return;
    }
    let tf = alert.interval;
    let bar = tf.bucket_start(now);
    // Bar rollover: the previous bar closed at its last price.
    if let Some(b) = run.bar
        && bar > b
    {
        let close = run.ohlc[3];
        if alert.trigger == Trigger::OncePerBarClose {
            let reference = reference(alert, run);
            if alert.condition.met(run.prev_close, close, b, reference) {
                fire(alert, run, close, now, b, out);
                if run.done {
                    return;
                }
            }
        }
        run.prev_close = Some(close);
        let n = alert.condition.bars.clamp(1, MAX_BARS) as usize;
        run.closes.push_back(close);
        while run.closes.len() > n {
            run.closes.pop_front();
        }
        run.bar = None;
    }
    let price = match price {
        Some(p) => p,
        // The clock opens the next bar at the last price.
        None => match run.prev {
            Some(p) if run.bar.is_none() => {
                run.bar = Some(bar);
                run.ohlc = [p; 4];
                return;
            }
            _ => return,
        },
    };
    if run.bar.is_none() {
        run.bar = Some(bar);
        run.ohlc = [price; 4];
    }
    let o = &mut run.ohlc;
    o[1] = o[1].max(price);
    o[2] = o[2].min(price);
    o[3] = price;
    if alert.trigger != Trigger::OncePerBarClose {
        let reference = reference(alert, run);
        let allowed = match alert.trigger {
            Trigger::OnlyOnce | Trigger::OncePerBarClose => true,
            Trigger::OncePerBar => run.fired_bar != Some(bar),
            Trigger::OncePerMinute => run.fired_ms.is_none_or(|t| now - t >= MINUTE_MS),
        };
        if allowed && alert.condition.met(run.prev, price, now, reference) {
            fire(alert, run, price, now, bar, out);
        }
    }
    run.prev = Some(price);
}

/// The close `bars` bars before the current one (Moving operators).
fn reference(alert: &Alert, run: &Run) -> Option<f64> {
    let n = alert.condition.bars.clamp(1, MAX_BARS) as usize;
    (run.closes.len() >= n).then(|| run.closes[run.closes.len() - n])
}

fn fire(alert: &Alert, run: &mut Run, price: f64, now: i64, bar: i64, out: &mut Vec<Fired>) {
    let last = alert.trigger == Trigger::OnlyOnce;
    run.fired_bar = Some(bar);
    run.fired_ms = Some(now);
    run.done = last;
    let [open, high, low, close] = run.ohlc;
    let message = format_message(
        &alert.message,
        &Placeholders {
            ticker: &alert.symbol,
            exchange: "BINANCE",
            interval: alert.interval,
            time_ms: bar,
            now_ms: now,
            open,
            high,
            low,
            close: if alert.trigger == Trigger::OncePerBarClose {
                price
            } else {
                close
            },
            volume: None,
            precision: alert.precision,
        },
    );
    out.push(Fired {
        alert_id: alert.id,
        kind: FiredKind::Triggered { last },
        time_ms: now,
        price,
        message,
    });
}

// ---- storage ------------------------------------------------------------------

/// The file's content.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AlertsFile {
    pub next_id: u64,
    pub alerts: Vec<Alert>,
    /// Newest last.
    pub log: Vec<LogEntry>,
    /// Alerts this build could not read, written back unchanged.
    pub unknown: Vec<serde_json::Value>,
}

impl Serialize for AlertsFile {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut alerts: Vec<serde_json::Value> = self
            .alerts
            .iter()
            .filter_map(|a| serde_json::to_value(a).ok())
            .collect();
        alerts.extend(self.unknown.iter().cloned());
        serde_json::json!({
            "version": ALERTS_VERSION,
            "next_id": self.next_id,
            "alerts": alerts,
            "log": self.log,
        })
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for AlertsFile {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize, Default)]
        #[serde(default)]
        struct Wire {
            next_id: u64,
            alerts: Vec<serde_json::Value>,
            log: Vec<serde_json::Value>,
        }
        let w = Wire::deserialize(d)?;
        let mut out = AlertsFile::default();
        for raw in w.alerts {
            match serde_json::from_value::<Alert>(raw.clone()) {
                Ok(a) if a.id != 0 && !out.alerts.iter().any(|b| b.id == a.id) => {
                    out.alerts.push(a)
                }
                Ok(_) => {
                    warn!("skipping alert with a missing or duplicate id");
                    out.unknown.push(raw);
                }
                Err(e) => {
                    warn!("skipping unreadable alert: {e}");
                    out.unknown.push(raw);
                }
            }
        }
        out.log = w
            .log
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();
        let max = out.alerts.iter().map(|a| a.id).max().unwrap_or(0);
        out.next_id = w.next_id.max(max + 1).max(1);
        Ok(out)
    }
}

impl AlertsFile {
    /// `alerts.json` in the directory of `settings_path`.
    pub fn path_next_to(settings_path: &Path) -> PathBuf {
        settings_path.with_file_name(ALERTS_FILE)
    }

    /// Reads the file: missing → empty; unparseable → moved aside, empty.
    pub fn load(path: Option<&Path>) -> Self {
        let Some(path) = path else {
            return Self::default();
        };
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    warn!(path = %path.display(), "could not read alerts: {e}");
                }
                return Self::default();
            }
        };
        match serde_json::from_str::<AlertsFile>(&text) {
            Ok(f) => f,
            Err(e) => {
                warn!(path = %path.display(), "alerts file is corrupt ({e}); moving it aside");
                let _ = fsutil::backup_corrupt(path);
                Self::default()
            }
        }
    }

    /// A fresh id.
    pub fn take_id(&mut self) -> u64 {
        self.next_id = self.next_id.max(1);
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Appends to the log, dropping the oldest beyond [`MAX_LOG`].
    pub fn push_log(&mut self, e: LogEntry) {
        self.log.push(e);
        if self.log.len() > MAX_LOG {
            let extra = self.log.len() - MAX_LOG;
            self.log.drain(..extra);
        }
    }
}

#[cfg(test)]
mod tests;
