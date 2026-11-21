//! Format v1 (one flat struct, PR "chart settings dialog") and its migration
//! to v2. v1 keys are read on load and never written back.
//!
//! [`ChartStyle`] deserializes through [`ChartStyleWire`], so every loader
//! (settings, templates, layouts) migrates without an extra call.

use super::*;
use ecolor::Color32;
use serde::Deserialize;

/// v1 user color overrides on top of the theme (`None` = theme color).
/// Kept as `Color32` (ecolor's premultiplied serde) so old files load exactly.
#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ChartColors {
    pub up_body: Option<Color32>,
    pub down_body: Option<Color32>,
    pub up_border: Option<Color32>,
    pub down_border: Option<Color32>,
    pub up_wick: Option<Color32>,
    pub down_wick: Option<Color32>,
    pub background: Option<Color32>,
    pub grid_vertical: Option<Color32>,
    pub grid_horizontal: Option<Color32>,
    pub crosshair: Option<Color32>,
    pub scale_text: Option<Color32>,
    pub scale_lines: Option<Color32>,
}

/// The v1 flat fields. Absent = v1's default, which v2's defaults equal.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct LegacyV1 {
    font_size: Option<f32>,
    dotted_grid: Option<bool>,
    color_by_prev_close: Option<bool>,
    body: Option<bool>,
    borders: Option<bool>,
    wick: Option<bool>,
    status_title: Option<bool>,
    status_values: Option<bool>,
    status_change: Option<bool>,
    status_volume: Option<bool>,
    legend_titles: Option<bool>,
    legend_inputs: Option<bool>,
    legend_values: Option<bool>,
    scale_buttons: Option<Visibility3>,
    no_overlapping_labels: Option<bool>,
    countdown: Option<bool>,
    last_price_label: Option<bool>,
    last_price_line: Option<bool>,
    indicator_value_labels: Option<bool>,
    day_of_week: Option<bool>,
    grid_vertical: Option<bool>,
    grid_horizontal: Option<bool>,
    margin_top: Option<f32>,
    margin_bottom: Option<f32>,
    margin_right: Option<f32>,
    colors: Option<ChartColors>,
}

/// What a settings file holds: the v2 fields plus any v1 keys.
#[derive(Deserialize)]
#[serde(default)]
pub(crate) struct ChartStyleWire {
    /// Absent in v1 files.
    version: u32,
    symbol: SymbolStyle,
    status_line: StatusLineStyle,
    scales: ScalesStyle,
    canvas: CanvasStyle,
    trading: TradingStyle,
    alerts: AlertsStyle,
    events: EventsStyle,
    #[serde(flatten)]
    legacy: LegacyV1,
}

impl Default for ChartStyleWire {
    fn default() -> Self {
        let d = ChartStyle::default();
        Self {
            version: 0,
            symbol: d.symbol,
            status_line: d.status_line,
            scales: d.scales,
            canvas: d.canvas,
            trading: d.trading,
            alerts: d.alerts,
            events: d.events,
            legacy: LegacyV1::default(),
        }
    }
}

impl From<ChartStyleWire> for ChartStyle {
    fn from(w: ChartStyleWire) -> Self {
        let mut s = ChartStyle {
            version: w.version,
            symbol: w.symbol,
            status_line: w.status_line,
            scales: w.scales,
            canvas: w.canvas,
            trading: w.trading,
            alerts: w.alerts,
            events: w.events,
            ..ChartStyle::default()
        };
        if s.version < STYLE_VERSION {
            w.legacy.migrate_into(&mut s);
        }
        s.version = STYLE_VERSION;
        s
    }
}

impl LegacyV1 {
    /// Writes the v1 values that are present over `s`.
    fn migrate_into(self, s: &mut ChartStyle) {
        fn set<T>(dst: &mut T, v: Option<T>) {
            if let Some(v) = v {
                *dst = v;
            }
        }
        use SharedCandleFlag as F;
        let flags = [
            (F::ColorByPrevClose, self.color_by_prev_close),
            (F::Body, self.body),
            (F::Borders, self.borders),
            (F::Wick, self.wick),
        ];
        for (flag, v) in flags {
            if let Some(v) = v {
                s.apply_shared_flag(flag, v);
            }
        }

        let st = &mut s.status_line;
        set(&mut st.title, self.status_title);
        set(&mut st.chart_values, self.status_values);
        set(&mut st.bar_change, self.status_change);
        set(&mut st.volume, self.status_volume);
        set(&mut st.indicator_titles, self.legend_titles);
        set(&mut st.indicator_inputs, self.legend_inputs);
        set(&mut st.indicator_values, self.legend_values);

        let sc = &mut s.scales;
        set(&mut sc.text_size, self.font_size);
        set(&mut sc.scale_buttons, self.scale_buttons);
        set(&mut sc.no_overlapping_labels, self.no_overlapping_labels);
        set(&mut sc.countdown, self.countdown);
        if self.last_price_label.is_some() || self.last_price_line.is_some() {
            sc.symbol_label = LabelMode::from_value_line(
                self.last_price_label.unwrap_or(true),
                self.last_price_line.unwrap_or(true),
            );
        }
        if let Some(on) = self.indicator_value_labels {
            sc.indicators = if on {
                LabelMode::Value
            } else {
                LabelMode::Hidden
            };
        }
        set(&mut sc.day_of_week, self.day_of_week);

        let cv = &mut s.canvas;
        if let Some(dotted) = self.dotted_grid {
            let kind = if dotted {
                LineKind::Dotted
            } else {
                LineKind::Solid
            };
            cv.grid_v_kind = kind;
            cv.grid_h_kind = kind;
        }
        if self.grid_vertical.is_some() || self.grid_horizontal.is_some() {
            cv.grid_lines = GridLines::from_flags(
                self.grid_vertical.unwrap_or(true),
                self.grid_horizontal.unwrap_or(true),
            );
        }
        set(&mut cv.margins.top, self.margin_top);
        set(&mut cv.margins.bottom, self.margin_bottom);
        set(&mut cv.margins.right, self.margin_right);

        if let Some(c) = self.colors {
            let slot = |v: Option<Color32>| v.map(Rgba::from_color32);
            let cv = &mut s.canvas;
            cv.background = slot(c.background);
            cv.grid_v_color = slot(c.grid_vertical);
            cv.grid_h_color = slot(c.grid_horizontal);
            cv.crosshair = slot(c.crosshair);
            cv.scale_text = slot(c.scale_text);
            cv.scale_lines = slot(c.scale_lines);
            use SharedCandleColor as C;
            for (key, v) in [
                (C::UpBody, c.up_body),
                (C::DownBody, c.down_body),
                (C::UpBorder, c.up_border),
                (C::DownBorder, c.down_border),
                (C::UpWick, c.up_wick),
                (C::DownWick, c.down_wick),
            ] {
                if v.is_some() {
                    s.apply_shared_color(key, slot(v));
                }
            }
        }
    }
}

/// A v1 Symbol-tab option. v1 had one "CANDLES" block for every chart type;
/// these apply it to every type that v1 drew with it. Used by the migration
/// and by the dialog until it shows per-type blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedCandleFlag {
    /// Candles, Heikin Ashi, Bars.
    ColorByPrevClose,
    /// Candles, Heikin Ashi, Hollow candles (v1 drew falling hollow candles
    /// like candles).
    Body,
    /// Candles, Heikin Ashi, Hollow candles.
    Borders,
    /// Candles, Heikin Ashi, Hollow candles.
    Wick,
}

/// A v1 candle color (see [`SharedCandleFlag`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedCandleColor {
    /// The up color of every type v1 drew with `theme.up`.
    UpBody,
    /// The down color of every type v1 drew with `theme.down`.
    DownBody,
    /// Candles, Heikin Ashi, Hollow and Volume candles.
    UpBorder,
    DownBorder,
    /// Candles, Heikin Ashi, Hollow and Volume candles.
    UpWick,
    DownWick,
}

impl ChartStyle {
    /// The current value of a shared option (read from Candles).
    pub fn shared_flag(&self, flag: SharedCandleFlag) -> bool {
        let c = &self.symbol.candles;
        match flag {
            SharedCandleFlag::ColorByPrevClose => c.color_by_prev_close,
            SharedCandleFlag::Body => c.body,
            SharedCandleFlag::Borders => c.borders,
            SharedCandleFlag::Wick => c.wick,
        }
    }

    pub fn apply_shared_flag(&mut self, flag: SharedCandleFlag, v: bool) {
        let y = &mut self.symbol;
        match flag {
            SharedCandleFlag::ColorByPrevClose => {
                y.candles.color_by_prev_close = v;
                y.heikin_ashi.color_by_prev_close = v;
                y.bars.color_by_prev_close = v;
            }
            SharedCandleFlag::Body => {
                y.candles.body = v;
                y.heikin_ashi.body = v;
                y.hollow_candles.body = v;
            }
            SharedCandleFlag::Borders => {
                y.candles.borders = v;
                y.heikin_ashi.borders = v;
                y.hollow_candles.borders = v;
            }
            SharedCandleFlag::Wick => {
                y.candles.wick = v;
                y.heikin_ashi.wick = v;
                y.hollow_candles.wick = v;
            }
        }
    }

    /// The Candles slot a shared color is read from.
    pub fn shared_color(&self, key: SharedCandleColor) -> ColorSlot {
        let c = &self.symbol.candles;
        match key {
            SharedCandleColor::UpBody => c.body_up,
            SharedCandleColor::DownBody => c.body_down,
            SharedCandleColor::UpBorder => c.border_up,
            SharedCandleColor::DownBorder => c.border_down,
            SharedCandleColor::UpWick => c.wick_up,
            SharedCandleColor::DownWick => c.wick_down,
        }
    }

    pub fn apply_shared_color(&mut self, key: SharedCandleColor, v: ColorSlot) {
        let y = &mut self.symbol;
        let candles = [
            &mut y.candles,
            &mut y.hollow_candles,
            &mut y.volume_candles,
            &mut y.heikin_ashi,
        ];
        match key {
            SharedCandleColor::UpBody => {
                candles.into_iter().for_each(|c| c.body_up = v);
                y.bars.up = v;
                y.columns.up = v;
                y.hlc_area.high.color = v;
                y.baseline.top_line = v;
                y.renko.colors.up = v;
                y.line_break.colors.up = v;
            }
            SharedCandleColor::DownBody => {
                candles.into_iter().for_each(|c| c.body_down = v);
                y.bars.down = v;
                y.columns.down = v;
                y.hlc_area.low.color = v;
                y.baseline.bottom_line = v;
                y.renko.colors.down = v;
                y.line_break.colors.down = v;
            }
            SharedCandleColor::UpBorder => candles.into_iter().for_each(|c| c.border_up = v),
            SharedCandleColor::DownBorder => candles.into_iter().for_each(|c| c.border_down = v),
            SharedCandleColor::UpWick => candles.into_iter().for_each(|c| c.wick_up = v),
            SharedCandleColor::DownWick => candles.into_iter().for_each(|c| c.wick_down = v),
        }
    }

    /// Pre-chart-settings up / down color overrides (`AppSettings.up_color`):
    /// applied where the style has no body color yet.
    pub fn adopt_legacy_up_down(&mut self, up: Option<Color32>, down: Option<Color32>) {
        if self.symbol.candles.body_up.is_none() && up.is_some() {
            self.apply_shared_color(SharedCandleColor::UpBody, up.map(Rgba::from_color32));
        }
        if self.symbol.candles.body_down.is_none() && down.is_some() {
            self.apply_shared_color(SharedCandleColor::DownBody, down.map(Rgba::from_color32));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChartType;
    use tt_theme::Theme;

    /// v1 `ChartStyle` and its color logic, frozen verbatim (style.rs before
    /// v2), to check that migrated files keep their meaning.
    mod v1 {
        use ecolor::Color32;
        use serde::Deserialize;
        use tt_theme::Theme;

        #[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
        pub enum ScaleButtons {
            #[default]
            OnHover,
            Always,
            Hidden,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
        #[serde(default)]
        pub struct ChartColors {
            pub up_body: Option<Color32>,
            pub down_body: Option<Color32>,
            pub up_border: Option<Color32>,
            pub down_border: Option<Color32>,
            pub up_wick: Option<Color32>,
            pub down_wick: Option<Color32>,
            pub background: Option<Color32>,
            pub grid_vertical: Option<Color32>,
            pub grid_horizontal: Option<Color32>,
            pub crosshair: Option<Color32>,
            pub scale_text: Option<Color32>,
            pub scale_lines: Option<Color32>,
        }

        #[derive(Debug, Clone, PartialEq, Deserialize)]
        #[serde(default)]
        pub struct ChartStyle {
            pub font_size: f32,
            pub dotted_grid: bool,
            pub color_by_prev_close: bool,
            pub body: bool,
            pub borders: bool,
            pub wick: bool,
            pub status_title: bool,
            pub status_values: bool,
            pub status_change: bool,
            pub status_volume: bool,
            pub legend_titles: bool,
            pub legend_inputs: bool,
            pub legend_values: bool,
            pub scale_buttons: ScaleButtons,
            pub no_overlapping_labels: bool,
            pub countdown: bool,
            pub last_price_label: bool,
            pub last_price_line: bool,
            pub indicator_value_labels: bool,
            pub day_of_week: bool,
            pub grid_vertical: bool,
            pub grid_horizontal: bool,
            pub margin_top: f32,
            pub margin_bottom: f32,
            pub margin_right: f32,
            pub colors: ChartColors,
        }

        impl Default for ChartStyle {
            fn default() -> Self {
                Self {
                    font_size: 12.5,
                    dotted_grid: true,
                    color_by_prev_close: false,
                    body: true,
                    borders: true,
                    wick: true,
                    status_title: true,
                    status_values: true,
                    status_change: true,
                    status_volume: false,
                    legend_titles: true,
                    legend_inputs: true,
                    legend_values: true,
                    scale_buttons: ScaleButtons::OnHover,
                    no_overlapping_labels: true,
                    countdown: true,
                    last_price_label: true,
                    last_price_line: true,
                    indicator_value_labels: true,
                    day_of_week: true,
                    grid_vertical: true,
                    grid_horizontal: true,
                    margin_top: 8.0,
                    margin_bottom: 8.0,
                    margin_right: 5.0,
                    colors: ChartColors::default(),
                }
            }
        }

        impl ChartStyle {
            pub fn theme(&self, base: &Theme) -> Theme {
                let c = &self.colors;
                let mut t = base.clone();
                if let Some(v) = c.up_body {
                    t.up = v;
                    t.volume_up = tt_theme::with_alpha(v, t.volume_up.a() as f32 / 255.0);
                }
                if let Some(v) = c.down_body {
                    t.down = v;
                    t.volume_down = tt_theme::with_alpha(v, t.volume_down.a() as f32 / 255.0);
                }
                if let Some(v) = c.up_wick {
                    t.up_wick = v;
                }
                if let Some(v) = c.down_wick {
                    t.down_wick = v;
                }
                if let Some(v) = c.background {
                    t.bg = v;
                }
                if let Some(v) = c.crosshair {
                    t.crosshair = v;
                }
                if let Some(v) = c.scale_text {
                    t.text_muted = v;
                }
                if let Some(v) = c.scale_lines {
                    t.border = v;
                }
                t
            }

            pub fn grid_color(&self, theme: &Theme, vertical: bool) -> Color32 {
                let o = if vertical {
                    self.colors.grid_vertical
                } else {
                    self.colors.grid_horizontal
                };
                o.unwrap_or(theme.grid)
            }

            pub fn border_color(&self, theme: &Theme, up: bool) -> Color32 {
                let o = if up {
                    self.colors.up_border
                } else {
                    self.colors.down_border
                };
                o.unwrap_or(if up { theme.up } else { theme.down })
            }
        }
    }

    const V1_DEFAULT: &str = include_str!("fixtures/v1_default.json");
    /// `ChartStyle::default()` of v1 with many non-default fields, serialized
    /// by the v1 code (note the premultiplied grid color).
    const V1_CUSTOM: &str = include_str!("fixtures/v1_custom.json");

    /// Every value the v1 renderer and UI read has the same meaning in v2.
    fn assert_same_meaning(json: &str) {
        let old: v1::ChartStyle = serde_json::from_str(json).unwrap();
        let new: ChartStyle = serde_json::from_str(json).unwrap();
        assert_eq!(new.version, STYLE_VERSION);

        for base in [Theme::dark(), Theme::light()] {
            let (to, tn) = (old.theme(&base), new.theme(&base));
            assert_eq!(to, tn, "theme");
            for vertical in [true, false] {
                assert_eq!(old.grid_color(&to, vertical), new.grid_color(&tn, vertical));
            }
            // Candles, Heikin Ashi and Hollow candles share the drawing code.
            for t in [
                ChartType::Candles,
                ChartType::HeikinAshi,
                ChartType::HollowCandles,
            ] {
                let c = new.symbol.candle(t);
                for up in [true, false] {
                    assert_eq!(
                        old.border_color(&to, up),
                        ChartStyle::border_color(c, &tn, up)
                    );
                }
            }
            // Per-type slots, resolved against the (overridden) theme the
            // renderer draws with, give the v1 colors.
            let y = &new.symbol;
            for (slot, token, want) in [
                (y.bars.up, tn.up, to.up),
                (y.bars.down, tn.down, to.down),
                (y.columns.up, tn.up, to.up),
                (y.columns.down, tn.down, to.down),
                (y.hlc_area.high.color, tn.up, to.up),
                (y.hlc_area.low.color, tn.down, to.down),
                (y.baseline.top_line, tn.up, to.up),
                (y.baseline.bottom_line, tn.down, to.down),
                (y.renko.colors.up, tn.up, to.up),
                (y.line_break.colors.down, tn.down, to.down),
                (y.hollow_candles.body_up, tn.up, to.up),
                (y.volume_candles.body_down, tn.down, to.down),
                (y.volume_candles.wick_up, tn.up_wick, to.up_wick),
                (y.hollow_candles.wick_down, tn.down_wick, to.down_wick),
                (y.heikin_ashi.wick_up, tn.up_wick, to.up_wick),
            ] {
                assert_eq!(new.resolve(slot, token), want);
            }
        }

        let y = &new.symbol;
        for t in [ChartType::Candles, ChartType::HeikinAshi] {
            let c = y.candle(t);
            assert_eq!(
                (c.color_by_prev_close, c.body, c.borders, c.wick),
                (old.color_by_prev_close, old.body, old.borders, old.wick)
            );
        }
        assert_eq!(y.bars.color_by_prev_close, old.color_by_prev_close);
        let h = &y.hollow_candles;
        assert_eq!(
            (h.body, h.borders, h.wick),
            (old.body, old.borders, old.wick)
        );

        let st = &new.status_line;
        assert_eq!(
            [
                st.title,
                st.chart_values,
                st.bar_change,
                st.volume,
                st.indicator_titles,
                st.indicator_inputs,
                st.indicator_values
            ],
            [
                old.status_title,
                old.status_values,
                old.status_change,
                old.status_volume,
                old.legend_titles,
                old.legend_inputs,
                old.legend_values
            ]
        );

        let sc = &new.scales;
        assert_eq!(sc.text_size, old.font_size);
        assert_eq!(
            format!("{:?}", sc.scale_buttons),
            format!("{:?}", old.scale_buttons)
        );
        assert_eq!(sc.no_overlapping_labels, old.no_overlapping_labels);
        assert_eq!(sc.countdown, old.countdown);
        assert_eq!(sc.symbol_label.shows_value(), old.last_price_label);
        assert_eq!(sc.symbol_label.shows_line(), old.last_price_line);
        assert_eq!(sc.indicators.shows_value(), old.indicator_value_labels);
        assert_eq!(sc.day_of_week, old.day_of_week);

        let cv = &new.canvas;
        assert_eq!(cv.grid_lines.vertical(), old.grid_vertical);
        assert_eq!(cv.grid_lines.horizontal(), old.grid_horizontal);
        assert_eq!(cv.grid_v_kind == LineKind::Dotted, old.dotted_grid);
        assert_eq!(cv.grid_h_kind == LineKind::Dotted, old.dotted_grid);
        assert_eq!(
            (cv.margins.top, cv.margins.bottom, cv.margins.right),
            (old.margin_top, old.margin_bottom, old.margin_right)
        );

        // Re-serialized without v1 keys; loads back unchanged.
        let json2 = serde_json::to_string(&new).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json2).unwrap();
        let keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();
        assert_eq!(
            keys,
            [
                "version",
                "symbol",
                "status_line",
                "scales",
                "canvas",
                "trading",
                "alerts",
                "events"
            ]
        );
        let back: ChartStyle = serde_json::from_str(&json2).unwrap();
        assert_eq!(back, new);
    }

    #[test]
    fn v1_default_file_is_the_v2_default() {
        let s: ChartStyle = serde_json::from_str(V1_DEFAULT).unwrap();
        assert_eq!(s, ChartStyle::default());
        assert_same_meaning(V1_DEFAULT);
    }

    #[test]
    fn v1_custom_file_keeps_its_meaning() {
        assert_same_meaning(V1_CUSTOM);
        let s: ChartStyle = serde_json::from_str(V1_CUSTOM).unwrap();
        // Spot checks against the fixture.
        assert_eq!(s.scales.text_size, 14.0);
        assert_eq!(s.scales.scale_buttons, Visibility3::Always);
        assert_eq!(s.scales.symbol_label, LabelMode::Value);
        assert_eq!(s.canvas.grid_lines, GridLines::Horizontal);
        assert_eq!(s.canvas.grid_h_kind, LineKind::Solid);
        assert_eq!(s.canvas.margins.top, 12.0);
        assert_eq!(s.symbol.bars.up, Some(Rgba::rgb(38, 166, 154)));
        assert_eq!(s.symbol.candles.border_down, Some(Rgba::rgb(200, 10, 20)));
        assert_eq!(s.canvas.grid_h_color, Some(Rgba::new(40, 40, 40, 128)));
        assert!(s.symbol.bars.color_by_prev_close && !s.symbol.candles.borders);
        // Untouched by v1: new fields keep their defaults.
        assert_eq!(s.symbol.line, LineSeriesStyle::default());
        assert!(!s.symbol.hollow_candles.color_by_prev_close);
    }

    #[test]
    fn partial_v1_files_migrate() {
        let s: ChartStyle = serde_json::from_str(r#"{"countdown":false}"#).unwrap();
        assert!(!s.scales.countdown && s.scales.symbol_label.shows_line());
        let s: ChartStyle = serde_json::from_str(r#"{"last_price_line":false}"#).unwrap();
        assert_eq!(s.scales.symbol_label, LabelMode::Value);
        let s: ChartStyle = serde_json::from_str(r#"{"grid_vertical":false}"#).unwrap();
        assert_eq!(s.canvas.grid_lines, GridLines::Horizontal);
    }

    #[test]
    fn v2_files_ignore_stray_v1_keys() {
        let s: ChartStyle =
            serde_json::from_str(r#"{"version":2,"countdown":false,"font_size":20.0}"#).unwrap();
        assert!(s.scales.countdown);
        assert_eq!(s.scales.text_size, 12.5);
    }

    #[test]
    fn legacy_up_down_fill_missing_body_colors() {
        let red = Color32::from_rgb(255, 0, 0);
        let blue = Color32::from_rgb(0, 0, 255);
        let mut s = ChartStyle::default();
        s.apply_shared_color(SharedCandleColor::DownBody, Some(Rgba::from_color32(blue)));
        s.adopt_legacy_up_down(Some(red), Some(red));
        assert_eq!(s.symbol.candles.body_up, Some(Rgba::from_color32(red)));
        assert_eq!(s.symbol.bars.up, Some(Rgba::from_color32(red)));
        assert_eq!(s.symbol.candles.body_down, Some(Rgba::from_color32(blue)));
    }
}
