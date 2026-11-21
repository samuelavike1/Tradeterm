//! The settings dialog's forms, independent of any UI toolkit (spec 6.4).
//!
//! Each tool family describes its Style and Text tabs as a list of
//! [`Field`]s. A field addresses its value by a JSON pointer into the
//! serialized [`Props`] (`/line`, `/background/color`, …), so a family adds
//! a field by listing it here, and the dialog renders and edits it without
//! knowing the family. Fib and channel levels are edited through the typed
//! [`level`] helpers. None of this runs per frame.
//!
//! Also here: the Coordinates tab's time format and the Visibility tab's
//! row labels.

use super::model::{Anchor, DrawingTool, Rgba};
use super::props::{Level, Props, Stroke};
use crate::time_axis::SlotTimes;
use serde_json::Value;
use tt_core::{TimeUnit, civil_from_days, days_from_civil};

/// Font sizes offered (TradingView's).
pub const FONT_SIZES: [u8; 10] = [10, 11, 12, 14, 16, 20, 24, 28, 32, 40];

/// The control of a field.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldKind {
    /// A section header (`label` is the title; no value).
    Section,
    /// A checkbox (bool).
    Check,
    /// A color with opacity ([`Rgba`]).
    Color,
    /// A [`Stroke`]: color, width and style.
    Line,
    /// Width and style only (`{width, style}`: the Fib levels line).
    WidthStyle,
    /// One of fixed values: (saved string, label).
    Choice(&'static [(&'static str, &'static str)]),
    /// A number.
    Number { min: f64, max: f64, step: f64 },
    /// A `0.0..=1.0` fraction shown as a percent.
    Percent,
    /// A font size from [`FONT_SIZES`].
    FontSize,
    /// A `TextStyle`: color, size, bold, italic.
    TextStyle,
    /// The levels list (`/levels`): on, coefficient and color per level.
    Levels,
}

/// One row of a tab.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub label: &'static str,
    /// JSON pointer of the value in the serialized props.
    pub path: String,
    pub kind: FieldKind,
    /// A checkbox in the label column (JSON pointer of a bool), e.g. the
    /// Fib trend line's on/off before its line control.
    pub check: Option<String>,
    /// Nested option (indented).
    pub indent: bool,
}

impl Field {
    fn new(label: &'static str, path: &str, kind: FieldKind) -> Self {
        Self {
            label,
            path: path.to_owned(),
            kind,
            check: None,
            indent: false,
        }
    }

    fn check(label: &'static str, path: &str) -> Self {
        Self::new(label, path, FieldKind::Check)
    }

    fn section(title: &'static str) -> Self {
        Self::new(title, "", FieldKind::Section)
    }

    fn with_check(mut self, path: &str) -> Self {
        self.check = Some(path.to_owned());
        self
    }

    fn indent(mut self) -> Self {
        self.indent = true;
        self
    }
}

const ENDS: &[(&str, &str)] = &[("Normal", "Normal"), ("Arrow", "Arrow")];
const STATS_POSITION: &[(&str, &str)] =
    &[("Left", "Left"), ("Center", "Center"), ("Right", "Right")];
const STATS_POSITION_AUTO: &[(&str, &str)] = &[
    ("Left", "Left"),
    ("Center", "Center"),
    ("Right", "Right"),
    ("Auto", "Auto"),
];
const H_ALIGN: &[(&str, &str)] = &[("Left", "Left"), ("Center", "Center"), ("Right", "Right")];
const V_ALIGN: &[(&str, &str)] = &[("Top", "Top"), ("Middle", "Middle"), ("Bottom", "Bottom")];
const ORIENTATION: &[(&str, &str)] = &[("false", "Horizontal"), ("true", "Vertical")];
const LEVEL_LABELS: &[(&str, &str)] = &[("false", "Values"), ("true", "Percents")];

/// The stats a tool's checklist offers: (field, label).
fn stats_options(tool: DrawingTool) -> &'static [(&'static str, &'static str)] {
    use DrawingTool::*;
    const LINE: &[(&str, &str)] = &[
        ("price_range", "Price range"),
        ("percent", "Percent change"),
        ("pips", "Change in pips"),
        ("bars", "Bars range"),
        ("date_time", "Date/time range"),
        ("angle", "Angle"),
    ];
    const ANGLE: &[(&str, &str)] = &[
        ("price_range", "Price range"),
        ("percent", "Percent change"),
        ("pips", "Change in pips"),
        ("bars", "Bars range"),
    ];
    const PRICE: &[(&str, &str)] = &[
        ("price_range", "Price range"),
        ("percent", "Percent change"),
        ("pips", "Change in pips"),
    ];
    const DATE: &[(&str, &str)] = &[
        ("bars", "Bars range"),
        ("date_time", "Date/time range"),
        ("volume", "Volume"),
    ];
    const BOTH: &[(&str, &str)] = &[
        ("price_range", "Price range"),
        ("percent", "Percent change"),
        ("pips", "Change in pips"),
        ("bars", "Bars range"),
        ("date_time", "Date/time range"),
        ("volume", "Volume"),
    ];
    match tool {
        TrendAngle => ANGLE,
        PriceRange => PRICE,
        DateRange => DATE,
        DateAndPriceRange => BOTH,
        _ => LINE,
    }
}

fn stats_fields(tool: DrawingTool, out: &mut Vec<Field>) {
    out.push(Field::section("Stats"));
    for (key, label) in stats_options(tool) {
        out.push(Field::check(label, &format!("/stats/{key}")));
    }
    let positions = if tool == DrawingTool::TrendAngle {
        STATS_POSITION_AUTO
    } else {
        STATS_POSITION
    };
    if !matches!(
        tool,
        DrawingTool::PriceRange | DrawingTool::DateRange | DrawingTool::DateAndPriceRange
    ) {
        out.push(Field::new(
            "Stats position",
            "/stats/position",
            FieldKind::Choice(positions),
        ));
        out.push(Field::check("Always show stats", "/stats/always"));
    }
}

/// The Style tab of a drawing.
pub fn style_fields(tool: DrawingTool, props: &Props) -> Vec<Field> {
    use FieldKind::*;
    let mut f = Vec::new();
    match props {
        Props::Line(_) => {
            f.push(Field::new("Line", "/line", Line));
            if tool != DrawingTool::TrendAngle {
                f.push(Field::new("Left end", "/ends/left", Choice(ENDS)));
                f.push(Field::new("Right end", "/ends/right", Choice(ENDS)));
            }
            f.push(Field::check("Extend left", "/extend/left"));
            f.push(Field::check("Extend right", "/extend/right"));
            f.push(Field::check("Middle point", "/middle_point"));
            f.push(Field::check("Price labels", "/price_labels"));
            stats_fields(tool, &mut f);
        }
        Props::HLine(_) => {
            f.push(Field::new("Line", "/line", Line));
            f.push(Field::check("Price label", "/price_label"));
        }
        Props::VLine(_) => {
            f.push(Field::new("Line", "/line", Line));
            f.push(Field::check("Extend through all panes", "/extend"));
            f.push(Field::check("Time label", "/time_label"));
        }
        Props::Cross(_) => {
            f.push(Field::new("Line", "/line", Line));
            f.push(Field::check("Price label", "/price_label"));
            f.push(Field::check("Time label", "/time_label"));
        }
        Props::Channel(_) => {
            f.push(Field::new("Line", "/line", Line));
            f.push(
                Field::new("Background", "/background/color", Color).with_check("/background/on"),
            );
            f.push(Field::check("Extend left", "/extend/left"));
            f.push(Field::check("Extend right", "/extend/right"));
            f.push(Field::section("Levels"));
            f.push(Field::new("Levels", "/levels", Levels));
        }
        Props::Fib(_) => {
            f.push(Field::new("Trend line", "/trend_line", Line).with_check("/trend_line_on"));
            f.push(Field::new("Levels line", "/levels_line", WidthStyle));
            f.push(Field::check("Extend left", "/extend/left"));
            f.push(Field::check("Extend right", "/extend/right"));
            f.push(
                Field::new("Background", "/background_opacity", Percent)
                    .with_check("/background_on"),
            );
            f.push(Field::check("Reverse", "/reverse"));
            f.push(Field::check("Prices", "/show_prices"));
            f.push(
                Field::new("Levels", "/percents", Choice(LEVEL_LABELS)).with_check("/show_levels"),
            );
            f.push(Field::new("Labels horizontal", "/label_h", Choice(H_ALIGN)));
            f.push(Field::new("Labels vertical", "/label_v", Choice(V_ALIGN)));
            f.push(Field::new("Font size", "/font_size", FontSize));
            f.push(Field::check("Fib levels based on log scale", "/log_scale"));
            f.push(Field::section("Levels"));
            f.push(Field::new("Levels", "/levels", Levels));
        }
        Props::Shape(_) => {
            f.push(Field::new("Border", "/border", Line));
            f.push(
                Field::new("Background", "/background/color", Color).with_check("/background/on"),
            );
            f.push(Field::new("Middle line", "/middle_line", Line).with_check("/middle_line_on"));
            f.push(Field::check("Extend left", "/extend/left"));
            f.push(Field::check("Extend right", "/extend/right"));
        }
        Props::Brush(_) => {
            f.push(Field::new("Line", "/line", Line));
            f.push(Field::new("Left end", "/ends/left", Choice(ENDS)));
            f.push(Field::new("Right end", "/ends/right", Choice(ENDS)));
            f.push(
                Field::new("Background", "/background/color", Color).with_check("/background/on"),
            );
        }
        Props::Path(_) => {
            f.push(Field::new("Line", "/line", Line));
            f.push(Field::new("Left end", "/ends/left", Choice(ENDS)));
            f.push(Field::new("Right end", "/ends/right", Choice(ENDS)));
        }
        Props::TextBox(_) => {
            f.push(
                Field::new("Background", "/background/color", Color).with_check("/background/on"),
            );
            f.push(Field::new("Border", "/border", Line).with_check("/border_on"));
            if tool == DrawingTool::Note {
                f.push(Field::new("Label", "/marker", Color));
            }
            f.push(Field::check("Text wrap", "/wrap"));
            f.push(
                Field::new(
                    "Wrap width",
                    "/wrap_width",
                    Number {
                        min: 20.0,
                        max: 2000.0,
                        step: 10.0,
                    },
                )
                .indent(),
            );
        }
        Props::Position(_) => {
            f.push(Field::section("Inputs"));
            let num = |min: f64, max: f64, step: f64| Number { min, max, step };
            f.push(Field::new(
                "Account size",
                "/account_size",
                num(0.0, 1e12, 100.0),
            ));
            f.push(Field::new("Lot size", "/lot_size", num(0.0, 1e9, 1.0)));
            f.push(Field::new("Risk", "/risk", num(0.0, 1e12, 1.0)));
            f.push(Field::new(
                "Risk unit",
                "/risk_percent",
                Choice(&[("true", "%"), ("false", "Currency")]),
            ));
            f.push(Field::new("Leverage", "/leverage", num(1.0, 1000.0, 1.0)));
            f.push(Field::section("Style"));
            f.push(Field::new("Lines", "/line", Line));
            f.push(Field::new("Stop color", "/stop_color", Color));
            f.push(Field::new("Target color", "/target_color", Color));
            f.push(Field::new("Text color", "/text_color", Color));
            f.push(Field::new("Font size", "/font_size", FontSize));
            f.push(Field::check("Compact stats mode", "/compact"));
            f.push(Field::check("Price labels", "/price_labels"));
            f.push(Field::check("Always show stats", "/always_stats"));
        }
        Props::Range(_) => {
            f.push(Field::new("Line", "/line", Line));
            f.push(Field::new("Border", "/border", Line).with_check("/border_on"));
            f.push(
                Field::new("Background", "/background/color", Color).with_check("/background/on"),
            );
            f.push(Field::new("Down color", "/down_color", Color));
            match tool {
                DrawingTool::PriceRange => {
                    f.push(Field::check("Extend left", "/extend/left"));
                    f.push(Field::check("Extend right", "/extend/right"));
                }
                DrawingTool::DateRange => {
                    // Extend's left / right are top / bottom here.
                    f.push(Field::check("Extend top", "/extend/left"));
                    f.push(Field::check("Extend bottom", "/extend/right"));
                }
                _ => {}
            }
            stats_fields(tool, &mut f);
            f.push(Field::section("Label"));
            f.push(Field::new("Label", "/label", TextStyle));
            f.push(
                Field::new("Label background", "/label_background/color", Color)
                    .with_check("/label_background/on"),
            );
        }
    }
    f
}

/// The Text tab's formatting fields (the text itself is the drawing's
/// `text`). Empty: the family has no Text tab.
pub fn text_fields(tool: DrawingTool, props: &Props) -> Vec<Field> {
    use FieldKind::*;
    let mut f = Vec::new();
    let layout = |f: &mut Vec<Field>| {
        f.push(Field::new(
            "Horizontal alignment",
            "/text/h",
            Choice(H_ALIGN),
        ));
        f.push(Field::new("Vertical alignment", "/text/v", Choice(V_ALIGN)));
    };
    match props {
        Props::Line(_) if tool == DrawingTool::TrendAngle => {
            // The angle label's style (spec 3.5).
            f.push(Field::new("Angle label", "/text_style", TextStyle));
        }
        Props::Line(_)
        | Props::HLine(_)
        | Props::Channel(_)
        | Props::Shape(_)
        | Props::Range(_) => {
            f.push(Field::check("Show text", "/text/on"));
            f.push(Field::new("Text style", "/text_style", TextStyle));
            layout(&mut f);
        }
        Props::VLine(_) => {
            f.push(Field::check("Show text", "/text/on"));
            f.push(Field::new("Text style", "/text_style", TextStyle));
            layout(&mut f);
            f.push(Field::new(
                "Text orientation",
                "/text/vertical",
                Choice(ORIENTATION),
            ));
        }
        Props::TextBox(_) => f.push(Field::new("Text style", "/text_style", TextStyle)),
        Props::Cross(_) | Props::Fib(_) | Props::Brush(_) | Props::Path(_) | Props::Position(_) => {
        }
    }
    f
}

/// Whether the Text tab has a text box (the drawing's own text).
pub fn has_text_content(tool: DrawingTool, props: &Props) -> bool {
    tool != DrawingTool::TrendAngle && !text_fields(tool, props).is_empty()
}

/// Which layout the settings dialog uses for a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogLayout {
    /// The field list (every tool until it opts in).
    Generic,
    /// TradingView-measured (docs/specs/ui-guidelines.md §2.10): one line
    /// picker per line, text bar / text / alignment row, price + bar
    /// coordinates, visibility with ticks, ranges and range sliders.
    Comfortable,
}

/// The dialog layout of `tool`. The horizontal ray is the reference; a
/// tool opts in by returning `Comfortable` here (its generic fields keep
/// working inside the new layout).
pub fn dialog_layout(tool: DrawingTool) -> DialogLayout {
    match tool {
        DrawingTool::HorizontalRay | DrawingTool::HorizontalLine => DialogLayout::Comfortable,
        _ => DialogLayout::Generic,
    }
}

/// The props as JSON (read fields from it with [`get`]).
pub fn props_value(props: &Props) -> Value {
    serde_json::to_value(props).unwrap_or(Value::Null)
}

pub fn get<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    v.pointer(path)
}

pub fn get_bool(v: &Value, path: &str) -> bool {
    get(v, path).and_then(Value::as_bool).unwrap_or(false)
}

pub fn get_f64(v: &Value, path: &str) -> f64 {
    get(v, path).and_then(Value::as_f64).unwrap_or(0.0)
}

pub fn get_color(v: &Value, path: &str) -> Option<Rgba> {
    serde_json::from_value(get(v, path)?.clone()).ok()
}

pub fn get_stroke(v: &Value, path: &str) -> Option<Stroke> {
    serde_json::from_value(get(v, path)?.clone()).ok()
}

/// The saved string of a choice (enums by name, bools as "true" /
/// "false").
pub fn get_choice(v: &Value, path: &str) -> String {
    match get(v, path) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

/// A choice's value for [`set`]: "true" / "false" are bools, the rest
/// strings (enum variants).
pub fn choice_value(s: &str) -> Value {
    match s {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => Value::String(s.to_owned()),
    }
}

/// Sets the value at `path`; returns whether the props changed. Values the
/// family can't read leave the props unchanged.
pub fn set(props: &mut Props, path: &str, value: Value) -> bool {
    let mut v = props_value(props);
    let Some(slot) = v.pointer_mut(path) else {
        return false;
    };
    if *slot == value {
        return false;
    }
    *slot = value;
    match serde_json::from_value::<Props>(v) {
        Ok(p) if p.kind() == props.kind() => {
            *props = p;
            true
        }
        _ => false,
    }
}

pub fn set_color(props: &mut Props, path: &str, c: Rgba) -> bool {
    match serde_json::to_value(c) {
        Ok(v) => set(props, path, v),
        Err(_) => false,
    }
}

/// The Fib / channel levels.
pub fn levels(props: &Props) -> &[Level] {
    match props {
        Props::Fib(p) => &p.levels,
        Props::Channel(p) => &p.levels,
        _ => &[],
    }
}

/// Typed edits of level `i` (Fib and channel).
pub mod level {
    use super::*;

    fn get_mut(props: &mut Props, i: usize) -> Option<(&mut Level, Stroke)> {
        match props {
            Props::Fib(p) => {
                let line = p.trend_line;
                p.levels.get_mut(i).map(|l| (l, line))
            }
            Props::Channel(p) => {
                let line = p.line;
                p.levels.get_mut(i).map(|l| (l, line))
            }
            _ => None,
        }
    }

    pub fn set_on(props: &mut Props, i: usize, on: bool) -> bool {
        get_mut(props, i).is_some_and(|(l, _)| std::mem::replace(&mut l.on, on) != on)
    }

    pub fn set_coeff(props: &mut Props, i: usize, coeff: f64) -> bool {
        if !coeff.is_finite() {
            return false;
        }
        get_mut(props, i).is_some_and(|(l, _)| std::mem::replace(&mut l.coeff, coeff) != coeff)
    }

    /// The level's color. A channel level drawn with the channel's line
    /// gets its own stroke (the line's width and style) in that color.
    pub fn color(props: &Props, i: usize) -> Rgba {
        match props {
            Props::Channel(p) => p
                .levels
                .get(i)
                .map(|l| l.stroke.map_or(p.line.color, |s| s.color))
                .unwrap_or(p.line.color),
            _ => levels(props).get(i).map_or(Rgba::rgb(0, 0, 0), |l| l.color),
        }
    }

    pub fn set_color(props: &mut Props, i: usize, c: Rgba) -> bool {
        let channel = matches!(props, Props::Channel(_));
        let Some((l, line)) = get_mut(props, i) else {
            return false;
        };
        let before = *l;
        l.color = c;
        if channel {
            let mut s = l.stroke.unwrap_or(line);
            s.color = c;
            l.stroke = Some(s);
        }
        *l != before
    }

    /// "Use one color": every level in `c`.
    pub fn set_all_colors(props: &mut Props, c: Rgba) -> bool {
        let n = levels(props).len();
        let mut any = false;
        for i in 0..n {
            any |= set_color(props, i, c);
        }
        any
    }
}

/// Which parts of point `k` the Coordinates tab edits: (time, price).
pub fn coordinate_parts(tool: DrawingTool, k: usize) -> (bool, bool) {
    use DrawingTool::*;
    let _ = k;
    match tool {
        HorizontalLine => (false, true),
        VerticalLine => (true, false),
        _ => (true, true),
    }
}

/// Whether the tool has a Coordinates tab (freehand strokes do not).
pub fn has_coordinates(tool: DrawingTool) -> bool {
    tool.placement() != super::model::Placement::Freehand
}

/// "Price 1", "Price 2", … (TradingView's row names).
pub fn point_label(k: usize) -> String {
    format!("#{}", k + 1)
}

/// An anchor's bar time for the Coordinates tab: `YYYY-MM-DD HH:MM` (UTC,
/// like the time axis).
pub fn format_time(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    let rest = ms.rem_euclid(86_400_000) / 60_000;
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", rest / 60, rest % 60)
}

/// Parses `YYYY-MM-DD`, `YYYY-MM-DD HH:MM` or `YYYY-MM-DD HH:MM:SS` (UTC).
pub fn parse_time(s: &str) -> Option<i64> {
    let s = s.trim();
    let (date, time) = match s.split_once([' ', 'T']) {
        Some((d, t)) => (d, Some(t.trim())),
        None => (s, None),
    };
    let mut dp = date.split('-');
    let y: i64 = dp.next()?.trim().parse().ok()?;
    let m: u32 = dp.next()?.trim().parse().ok()?;
    let d: u32 = dp.next()?.trim().parse().ok()?;
    if dp.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let (mut hh, mut mm, mut ss) = (0i64, 0i64, 0i64);
    if let Some(t) = time.filter(|t| !t.is_empty()) {
        let mut tp = t.split(':');
        hh = tp.next()?.trim().parse().ok()?;
        mm = tp.next().map_or(Some(0), |x| x.trim().parse().ok())?;
        ss = tp.next().map_or(Some(0), |x| x.trim().parse().ok())?;
        if tp.next().is_some()
            || !(0..24).contains(&hh)
            || !(0..60).contains(&mm)
            || !(0..60).contains(&ss)
        {
            return None;
        }
    }
    let days = days_from_civil(y, m, d);
    // Reject dates that roll over (2026-02-31).
    if civil_from_days(days) != (y, m, d) {
        return None;
    }
    Some(days * 86_400_000 + (hh * 3600 + mm * 60 + ss) * 1000)
}

/// Sets point `k` (Coordinates tab); `None` leaves that part.
pub fn set_point(
    points: &mut [Anchor],
    k: usize,
    time_ms: Option<i64>,
    price: Option<f64>,
) -> bool {
    let Some(a) = points.get_mut(k) else {
        return false;
    };
    let before = *a;
    if let Some(t) = time_ms {
        a.time_ms = t;
    }
    if let Some(p) = price.filter(|p| p.is_finite()) {
        a.price = p;
    }
    *a != before
}

/// TradingView's "bar" coordinate of an anchor time: its slot on the
/// chart's x axis, rounded (0 = the first loaded bar; negative before it,
/// past the last bar beyond it). `None` without data.
pub fn bar_of_time(slots: &SlotTimes<'_>, time_ms: i64) -> Option<i64> {
    slots.slot_of(time_ms).map(|s| s.round() as i64)
}

/// The anchor time of bar number `bar` (inverse of [`bar_of_time`] on bar
/// times; extrapolated by the interval outside the data). Anchors stay
/// stored as times.
pub fn time_of_bar(slots: &SlotTimes<'_>, bar: i64) -> Option<i64> {
    slots.time_at(bar)
}

/// "#1 (price, bar)": the Coordinates row label of point `k`.
pub fn point_label_price_bar(k: usize) -> String {
    format!("#{} (price, bar)", k + 1)
}

/// Visibility tab row label.
pub fn unit_label(u: TimeUnit) -> &'static str {
    match u {
        TimeUnit::Second => "Seconds",
        TimeUnit::Minute => "Minutes",
        TimeUnit::Hour => "Hours",
        TimeUnit::Day => "Days",
        TimeUnit::Week => "Weeks",
        TimeUnit::Month => "Months",
    }
}
