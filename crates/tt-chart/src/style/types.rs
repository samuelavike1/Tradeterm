//! Value types shared by the chart settings (colors, line kinds, modes).
//!
//! Enum variants serialize in `snake_case`; they are stored in settings and
//! templates, so names are stable.

use ecolor::Color32;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// sRGB + alpha, **unmultiplied** (as a color picker shows it). Serialized
/// as `"#rrggbb"` (opaque) or `"#rrggbbaa"`; ecolor's legacy `[r, g, b, a]`
/// (premultiplied) array is also accepted.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self([r, g, b, 255])
    }

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }

    /// The opaque color of `c` (its alpha is dropped).
    pub const fn opaque(c: Color32) -> Self {
        Self([c.r(), c.g(), c.b(), 255])
    }

    pub const fn r(self) -> u8 {
        self.0[0]
    }
    pub const fn g(self) -> u8 {
        self.0[1]
    }
    pub const fn b(self) -> u8 {
        self.0[2]
    }
    pub const fn a(self) -> u8 {
        self.0[3]
    }

    /// Opacity in `0.0..=1.0`.
    pub fn opacity(self) -> f32 {
        self.0[3] as f32 / 255.0
    }

    /// The same color at `opacity` (`0.0..=1.0`).
    pub fn with_opacity(self, opacity: f32) -> Self {
        let a = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        Self([self.0[0], self.0[1], self.0[2], a])
    }

    /// The same opacity with the color of `rgb`.
    pub fn with_rgb_of(self, rgb: Rgba) -> Self {
        Self([rgb.0[0], rgb.0[1], rgb.0[2], self.0[3]])
    }

    /// Premultiplied color for drawing (exact for opaque colors).
    #[inline]
    pub const fn to_color32(self) -> Color32 {
        let [r, g, b, a] = self.0;
        Color32::from_rgba_unmultiplied_const(r, g, b, a)
    }

    #[inline]
    pub fn from_color32(c: Color32) -> Self {
        Self(c.to_srgba_unmultiplied())
    }

    pub fn to_hex(self) -> String {
        let [r, g, b, a] = self.0;
        if a == 255 {
            format!("#{r:02x}{g:02x}{b:02x}")
        } else {
            format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
        }
    }

    pub fn from_hex(s: &str) -> Option<Self> {
        let h = s.strip_prefix('#').unwrap_or(s);
        if !h.is_ascii() || !(h.len() == 6 || h.len() == 8) {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
        let a = if h.len() == 8 { byte(6)? } else { 255 };
        Some(Self([byte(0)?, byte(2)?, byte(4)?, a]))
    }
}

impl From<Rgba> for Color32 {
    fn from(c: Rgba) -> Self {
        c.to_color32()
    }
}

impl From<Color32> for Rgba {
    fn from(c: Color32) -> Self {
        Self::from_color32(c)
    }
}

impl Serialize for Rgba {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Hex(String),
            /// ecolor's `Color32` serde: premultiplied `[r, g, b, a]`.
            Premultiplied([u8; 4]),
        }
        match Repr::deserialize(d)? {
            Repr::Hex(s) => Rgba::from_hex(&s)
                .ok_or_else(|| serde::de::Error::custom(format!("invalid color {s:?}"))),
            Repr::Premultiplied([r, g, b, a]) => Ok(Rgba::from_color32(
                Color32::from_rgba_premultiplied(r, g, b, a),
            )),
        }
    }
}

/// A color setting: `None` = the theme token named by the field's doc
/// comment, so it follows dark / light. Only an explicit pick stores a value.
pub type ColorSlot = Option<Rgba>;

/// Resolves a slot against its theme token.
#[inline]
pub fn resolve(slot: ColorSlot, token: Color32) -> Color32 {
    match slot {
        Some(c) => c.to_color32(),
        None => token,
    }
}

macro_rules! settings_enum {
    (
        $(#[$meta:meta])*
        $name:ident { $( $(#[$vmeta:meta])* $variant:ident => $label:literal ),+ $(,)? }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// Text shown in the settings dialog.
            pub fn label(self) -> &'static str {
                match self {
                    $($name::$variant => $label,)+
                }
            }
        }
    };
}

/// Line pattern of a chart line (TradingView's chart settings use these three).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineKind {
    #[default]
    Solid,
    Dotted,
    Dashed,
}

impl LineKind {
    pub const ALL: &'static [LineKind] = &[LineKind::Solid, LineKind::Dotted, LineKind::Dashed];

    pub fn label(self) -> &'static str {
        match self {
            LineKind::Solid => "Solid",
            LineKind::Dotted => "Dotted",
            LineKind::Dashed => "Dashed",
        }
    }
}

settings_enum! {
    /// The bar value a line-like chart plots.
    PriceSource {
        Open => "Open",
        High => "High",
        Low => "Low",
        #[default]
        Close => "Close",
        Hl2 => "(H + L)/2",
        Hlc3 => "(H + L + C)/3",
        Ohlc4 => "(O + H + L + C)/4",
        Hlcc4 => "(H + L + C + C)/4",
    }
}

/// When an on-chart control or label shows (TradingView's "Visible on mouse
/// over / Always visible / Hidden"). v1 stored `"OnHover"` / `"Always"` /
/// `"Hidden"`; those names still load.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility3 {
    /// While the pointer is over the area (TradingView's default).
    #[default]
    #[serde(alias = "OnHover")]
    OnHover,
    #[serde(alias = "Always")]
    Always,
    #[serde(alias = "Hidden")]
    Hidden,
}

impl Visibility3 {
    pub const ALL: [Visibility3; 3] = [
        Visibility3::OnHover,
        Visibility3::Always,
        Visibility3::Hidden,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Visibility3::OnHover => "Visible on mouse over",
            Visibility3::Always => "Always visible",
            Visibility3::Hidden => "Hidden",
        }
    }
}

/// When the A / L / % scale buttons at the bottom of the price axis show.
pub type ScaleButtons = Visibility3;

settings_enum! {
    /// Navigation / pane button visibility (Canvas › Buttons).
    ButtonsVisibility {
        #[default]
        OnHover => "Visible on mouse over",
        Always => "Always visible",
        Never => "Always invisible",
    }
}

settings_enum! {
    /// Which grid directions are drawn.
    GridLines {
        #[default]
        Both => "Vert and horz",
        Vertical => "Vert only",
        Horizontal => "Horz only",
        None => "None",
    }
}

impl GridLines {
    pub fn from_flags(vertical: bool, horizontal: bool) -> Self {
        match (vertical, horizontal) {
            (true, true) => GridLines::Both,
            (true, false) => GridLines::Vertical,
            (false, true) => GridLines::Horizontal,
            (false, false) => GridLines::None,
        }
    }

    #[inline]
    pub fn vertical(self) -> bool {
        matches!(self, GridLines::Both | GridLines::Vertical)
    }

    #[inline]
    pub fn horizontal(self) -> bool {
        matches!(self, GridLines::Both | GridLines::Horizontal)
    }
}

settings_enum! {
    BackgroundKind {
        #[default]
        Solid => "Solid",
        Gradient => "Gradient",
    }
}

settings_enum! {
    /// How a Renko / Kagi / P&F box (or reversal) size is assigned.
    BoxMethod {
        #[default]
        Atr => "ATR",
        Traditional => "Traditional",
        PercentLtp => "Percentage (LTP)",
    }
}

settings_enum! {
    /// Which bar values form Renko bricks.
    BrickSource {
        #[default]
        Close => "Close",
        Ohlc => "OHLC",
    }
}

settings_enum! {
    /// Point & figure source.
    PnfSource {
        #[default]
        Close => "Close",
        HighLow => "High/Low",
    }
}

settings_enum! {
    /// Price-scale label of a series / indicator: which of name, value and
    /// line show.
    LabelMode {
        #[default]
        ValueLine => "Value, line",
        Value => "Value",
        Line => "Line",
        NameValueLine => "Name, value, line",
        NameValue => "Name, value",
        Name => "Name",
        Hidden => "Hidden",
    }
}

impl LabelMode {
    /// The mode showing exactly a value label and/or a line.
    pub fn from_value_line(value: bool, line: bool) -> Self {
        match (value, line) {
            (true, true) => LabelMode::ValueLine,
            (true, false) => LabelMode::Value,
            (false, true) => LabelMode::Line,
            (false, false) => LabelMode::Hidden,
        }
    }

    #[inline]
    pub fn shows_value(self) -> bool {
        matches!(
            self,
            LabelMode::ValueLine
                | LabelMode::Value
                | LabelMode::NameValueLine
                | LabelMode::NameValue
        )
    }

    #[inline]
    pub fn shows_line(self) -> bool {
        matches!(
            self,
            LabelMode::ValueLine | LabelMode::Line | LabelMode::NameValueLine
        )
    }

    #[inline]
    pub fn shows_name(self) -> bool {
        matches!(
            self,
            LabelMode::NameValueLine | LabelMode::NameValue | LabelMode::Name
        )
    }

    /// This mode with the value label switched on / off (name and line kept).
    pub fn with_value(self, on: bool) -> Self {
        self.rebuild(self.shows_name(), on, self.shows_line())
    }

    /// This mode with the line switched on / off (name and value kept).
    pub fn with_line(self, on: bool) -> Self {
        self.rebuild(self.shows_name(), self.shows_value(), on)
    }

    fn rebuild(self, name: bool, value: bool, line: bool) -> Self {
        match (name, value, line) {
            (true, true, true) => LabelMode::NameValueLine,
            (true, true, false) => LabelMode::NameValue,
            // There is no "name, line" mode: keep the name.
            (true, false, _) => LabelMode::Name,
            (false, v, l) => LabelMode::from_value_line(v, l),
        }
    }
}

settings_enum! {
    /// Value shown in the last-price label.
    LastValueMode {
        #[default]
        AccordingToScale => "Value according to scale",
        PriceAndPercent => "Price and percentage value",
    }
}

settings_enum! {
    TitleFormat {
        Description => "Description",
        #[default]
        Ticker => "Ticker",
        TickerAndDescription => "Ticker and description",
    }
}

settings_enum! {
    /// Date format of time-axis and crosshair labels.
    DateFormat {
        #[default]
        DayMonYy => "29 Sep '97",
        MonDdYyyy => "Sep 29, 1997",
        DdMonYyyy => "29 Sep 1997",
        Iso => "1997-09-29",
        YyMmDd => "97-09-29",
        YyyySlash => "1997/09/29",
        DdMmYyyyDot => "29.09.1997",
        DdMmYyDot => "29.09.97",
        DdMmYyyySlash => "29/09/1997",
        DdMmYySlash => "29/09/97",
        MmDdYyyySlash => "09/29/1997",
        MmDdYySlash => "09/29/97",
        DdMmYyyyDash => "29-09-1997",
    }
}

settings_enum! {
    HoursFormat {
        #[default]
        H24 => "24-hours",
        H12 => "12-hours",
    }
}

settings_enum! {
    /// Trading session (stocks and funds; crypto trades 24/7).
    Session {
        #[default]
        Regular => "Regular trading hours",
        Extended => "Extended trading hours",
        H24 => "24 hours",
    }
}

settings_enum! {
    ScalePlacement {
        #[default]
        Auto => "Auto",
        Left => "Left",
        Right => "Right",
    }
}

settings_enum! {
    /// How profit & loss is displayed (Trading tab).
    PnlDisplay {
        #[default]
        Money => "Money",
        Ticks => "Ticks",
        Percent => "Percent",
    }
}

settings_enum! {
    TradeNotifications {
        #[default]
        RejectionOnly => "Rejection only",
        All => "All events",
    }
}

settings_enum! {
    /// Horizontal alignment of order / position lines.
    LineAlignment {
        #[default]
        Right => "Right",
        Left => "Left",
        Center => "Center",
    }
}

/// Price format override (Symbol › Data modification › Precision).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Precision {
    /// From the symbol's tick size.
    #[default]
    Default,
    /// Fixed decimals, 0..=10.
    Decimals(u8),
    /// Fractional (1/2 … 1/320), for futures and bonds.
    Fraction(u16),
}

/// Time zone of time labels (Symbol › Time zone).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeZoneSetting {
    /// The exchange's zone (UTC for crypto).
    #[default]
    Exchange,
    Utc,
    /// The operating system's zone.
    Local,
    /// An IANA zone name, e.g. `"Europe/Berlin"`.
    Iana(String),
}

/// A line's color, width and pattern.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StrokeStyle {
    pub color: ColorSlot,
    pub width: f32,
    pub kind: LineKind,
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            color: None,
            width: 2.0,
            kind: LineKind::Solid,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_hex_and_legacy_arrays() {
        let c = Rgba::new(40, 41, 42, 128);
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(json, r##""#28292a80""##);
        assert_eq!(serde_json::from_str::<Rgba>(&json).unwrap(), c);
        assert_eq!(
            serde_json::to_string(&Rgba::rgb(1, 2, 3)).unwrap(),
            r##""#010203""##
        );
        // ecolor stored premultiplied arrays.
        let legacy: Rgba = serde_json::from_str("[20,20,20,128]").unwrap();
        assert_eq!(legacy, Rgba::new(40, 40, 40, 128));
        assert_eq!(
            legacy.to_color32(),
            Color32::from_rgba_premultiplied(20, 20, 20, 128)
        );
        assert!(serde_json::from_str::<Rgba>(r##""#12345""##).is_err());
    }

    #[test]
    fn color32_round_trip_is_exact() {
        for a in [1u8, 7, 64, 128, 200, 254, 255] {
            for p in 0..=a {
                let c = Color32::from_rgba_premultiplied(p, p / 2, 0, a);
                assert_eq!(Rgba::from_color32(c).to_color32(), c, "p={p} a={a}");
            }
        }
    }

    #[test]
    fn label_modes() {
        for (v, l) in [(true, true), (true, false), (false, true), (false, false)] {
            let m = LabelMode::from_value_line(v, l);
            assert_eq!((m.shows_value(), m.shows_line()), (v, l));
            assert_eq!(m.with_value(!v).shows_value(), !v);
            assert_eq!(m.with_line(!l).shows_line(), !l);
        }
        assert_eq!(
            serde_json::to_string(&LabelMode::NameValueLine).unwrap(),
            r#""name_value_line""#
        );
        assert_eq!(
            serde_json::from_str::<Visibility3>(r#""OnHover""#).unwrap(),
            Visibility3::OnHover
        );
        assert_eq!(
            serde_json::to_string(&Visibility3::Always).unwrap(),
            r#""always""#
        );
    }
}
