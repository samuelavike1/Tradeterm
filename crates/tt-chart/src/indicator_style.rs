//! Per-indicator style: plots, levels and fills keyed by the stable ids of
//! the study metadata ([`tt_core::StudyMeta`]), plus precision and the
//! status-line / price-scale switches.
//!
//! Styles are **sparse overrides**: every field is optional and a missing
//! one means "the metadata default", so settings files stay small and
//! improved defaults reach old charts. [`IndicatorStyle::plot`],
//! [`IndicatorStyle::hline`] and [`IndicatorStyle::fill`] resolve an
//! override against its definition into a plain `Copy` value for drawing
//! (no allocation, a short map lookup).
//!
//! A style is never part of an indicator's compute cache: editing it only
//! repaints.

use crate::style::Rgba;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tt_core::{FillDef, HLineDef, LineStyle, PlotDef, PlotType, StudyMeta};

/// Most colors a plot has (MACD histogram: 4; Volume: 2; others 1).
pub const MAX_PALETTE: usize = 4;

/// Highest selectable precision (TradingView's Default, 0..=8).
pub const MAX_PRECISION: u8 = 8;

/// Plot line widths offered (1..=4 px).
pub const MAX_WIDTH: u8 = 4;

impl Rgba {
    /// From tt-core's `[r, g, b, a]` (both are straight alpha).
    #[inline]
    pub const fn from_core(c: tt_core::Rgba) -> Self {
        Self(c)
    }

    /// To tt-core's `[r, g, b, a]`.
    #[inline]
    pub const fn to_core(self) -> tt_core::Rgba {
        self.0
    }
}

/// The chart's line style for a metadata one.
#[inline]
pub fn line_style(s: LineStyle) -> crate::drawings::LineStyle {
    match s {
        LineStyle::Solid => crate::drawings::LineStyle::Solid,
        LineStyle::Dashed => crate::drawings::LineStyle::Dashed,
        LineStyle::Dotted => crate::drawings::LineStyle::Dotted,
    }
}

/// Overrides of one plot. `None` = the [`PlotDef`] default.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PlotStyle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    /// Colors by palette index (single-color plots use index 0); a `None`
    /// entry keeps that palette color's default. Opacity is in the alpha.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub colors: Vec<Option<Rgba>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plot_type: Option<PlotType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_style: Option<LineStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_line: Option<bool>,
}

/// Overrides of one level (`hline`). `None` = the [`HLineDef`] default.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HLineStyle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Rgba>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<LineStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
}

/// Overrides of one fill. `None` = the [`FillDef`] default.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FillStyle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Rgba>,
    /// Second color of a gradient fill.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color2: Option<Rgba>,
}

/// Style of one indicator. Maps are keyed by `PlotDef::id`,
/// `HLineDef::id` and `FillDef::id`; unknown keys (a plot removed in a
/// later version) are kept but ignored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IndicatorStyle {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub plots: BTreeMap<String, PlotStyle>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub hlines: BTreeMap<String, HLineStyle>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub fills: BTreeMap<String, FillStyle>,
    /// Decimals; `None` = "Default" (the study's value format).
    pub precision: Option<u8>,
    /// Last-value tags on the axis (ANDed with the chart setting).
    pub labels_on_price_scale: bool,
    /// Values in the legend (ANDed with the chart setting).
    pub values_in_status_line: bool,
    /// Inputs after the title in the legend (ANDed with the chart setting).
    pub inputs_in_status_line: bool,
}

impl Default for IndicatorStyle {
    fn default() -> Self {
        Self {
            plots: BTreeMap::new(),
            hlines: BTreeMap::new(),
            fills: BTreeMap::new(),
            precision: None,
            labels_on_price_scale: true,
            values_in_status_line: true,
            inputs_in_status_line: true,
        }
    }
}

/// A plot's effective style.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedPlot {
    pub visible: bool,
    /// The palette (`colors[..len]`); single-color plots have one entry.
    colors: [Rgba; MAX_PALETTE],
    len: u8,
    /// Which palette entries are explicit overrides (bit `i`).
    overridden: u8,
    pub width: u8,
    pub plot_type: PlotType,
    pub line_style: LineStyle,
    pub price_line: bool,
}

impl ResolvedPlot {
    /// The main color (palette entry 0).
    #[inline]
    pub fn color(&self) -> Rgba {
        self.colors[0]
    }

    /// Palette entry `i` (entry 0 when out of range).
    #[inline]
    pub fn palette(&self, i: usize) -> Rgba {
        self.colors[if i < self.len as usize { i } else { 0 }]
    }

    /// The whole palette.
    pub fn colors(&self) -> &[Rgba] {
        &self.colors[..self.len as usize]
    }

    /// Whether palette entry `i` was set by the user (Volume uses the
    /// theme's colors for entries that were not).
    #[inline]
    pub fn is_overridden(&self, i: usize) -> bool {
        i < MAX_PALETTE && self.overridden & (1 << i) != 0
    }

    /// Whether the type fills down to zero (so auto-fit includes 0).
    pub fn from_zero(&self) -> bool {
        matches!(
            self.plot_type,
            PlotType::Histogram | PlotType::Columns | PlotType::Area | PlotType::AreaBr
        )
    }
}

/// A level's effective style.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedHLine {
    pub visible: bool,
    pub value: f64,
    pub color: Rgba,
    pub width: u8,
    pub style: LineStyle,
}

/// A fill's effective style.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedFill {
    pub visible: bool,
    pub color: Rgba,
    pub color2: Option<Rgba>,
}

/// Number of palette entries of a plot (at least 1).
fn palette_len(def: &PlotDef) -> usize {
    def.palette.len().clamp(1, MAX_PALETTE)
}

/// Default color of palette entry `i`.
fn default_color(def: &PlotDef, i: usize) -> Rgba {
    match def.palette.get(i) {
        Some((_, c)) => Rgba::from_core(*c),
        None => Rgba::from_core(def.default.color),
    }
}

impl IndicatorStyle {
    /// Effective style of `def`.
    pub fn plot(&self, def: &PlotDef) -> ResolvedPlot {
        let o = self.plots.get(def.id);
        let len = palette_len(def);
        let mut colors = [Rgba::from_core(def.default.color); MAX_PALETTE];
        let mut overridden = 0u8;
        for (i, c) in colors.iter_mut().enumerate().take(len) {
            *c = match o.and_then(|o| o.colors.get(i).copied().flatten()) {
                Some(c) => {
                    overridden |= 1 << i;
                    c
                }
                None => default_color(def, i),
            };
        }
        let d = &def.default;
        ResolvedPlot {
            visible: o.and_then(|o| o.visible).unwrap_or(d.visible),
            colors,
            len: len as u8,
            overridden,
            width: o
                .and_then(|o| o.width)
                .unwrap_or(d.width)
                .clamp(1, MAX_WIDTH),
            plot_type: o.and_then(|o| o.plot_type).unwrap_or(d.plot_type),
            line_style: o.and_then(|o| o.line_style).unwrap_or(d.line_style),
            price_line: o.and_then(|o| o.price_line).unwrap_or(false),
        }
    }

    /// Effective style of `def`.
    pub fn hline(&self, def: &HLineDef) -> ResolvedHLine {
        let o = self.hlines.get(def.id);
        ResolvedHLine {
            visible: o.and_then(|o| o.visible).unwrap_or(true),
            value: o
                .and_then(|o| o.value)
                .filter(|v| v.is_finite())
                .unwrap_or(def.value),
            color: o
                .and_then(|o| o.color)
                .unwrap_or(Rgba::from_core(def.color)),
            width: o
                .and_then(|o| o.width)
                .unwrap_or(def.width)
                .clamp(1, MAX_WIDTH),
            style: o.and_then(|o| o.style).unwrap_or(def.style),
        }
    }

    /// Effective style of `def`.
    pub fn fill(&self, def: &FillDef) -> ResolvedFill {
        let o = self.fills.get(def.id);
        ResolvedFill {
            visible: o.and_then(|o| o.visible).unwrap_or(def.visible),
            color: o
                .and_then(|o| o.color)
                .unwrap_or(Rgba::from_core(def.color)),
            color2: o.and_then(|o| o.color2).or(def.color2.map(Rgba::from_core)),
        }
    }

    /// The overrides of plot `id`, created if missing (for editing).
    pub fn plot_mut(&mut self, id: &str) -> &mut PlotStyle {
        self.plots.entry(id.to_owned()).or_default()
    }

    pub fn hline_mut(&mut self, id: &str) -> &mut HLineStyle {
        self.hlines.entry(id.to_owned()).or_default()
    }

    pub fn fill_mut(&mut self, id: &str) -> &mut FillStyle {
        self.fills.entry(id.to_owned()).or_default()
    }

    /// Sets palette entry `i` of plot `id`.
    pub fn set_plot_color(&mut self, id: &str, i: usize, c: Rgba) {
        if i >= MAX_PALETTE {
            return;
        }
        let p = self.plot_mut(id);
        if p.colors.len() <= i {
            p.colors.resize(i + 1, None);
        }
        p.colors[i] = Some(c);
    }

    /// Decimals to format with (`None` = the study's default format).
    pub fn precision(&self) -> Option<u8> {
        self.precision.map(|p| p.min(MAX_PRECISION))
    }

    /// Drops every override equal to its default and every empty entry,
    /// so only real changes are stored. Entries for ids `meta` does not
    /// know are kept (a newer version may know them).
    pub fn prune(&mut self, meta: &StudyMeta) {
        self.plots.retain(|id, o| {
            if let Some(def) = meta.plot(id) {
                let d = &def.default;
                prune_eq(&mut o.visible, d.visible);
                prune_eq(&mut o.width, d.width);
                prune_eq(&mut o.plot_type, d.plot_type);
                prune_eq(&mut o.line_style, d.line_style);
                prune_eq(&mut o.price_line, false);
                o.colors.truncate(palette_len(def));
                for (i, c) in o.colors.iter_mut().enumerate() {
                    if *c == Some(default_color(def, i)) {
                        *c = None;
                    }
                }
                while o.colors.last() == Some(&None) {
                    o.colors.pop();
                }
            }
            *o != PlotStyle::default()
        });
        self.hlines.retain(|id, o| {
            if let Some(def) = meta.hline(id) {
                prune_eq(&mut o.visible, true);
                prune_eq(&mut o.value, def.value);
                prune_eq(&mut o.color, Rgba::from_core(def.color));
                prune_eq(&mut o.width, def.width);
                prune_eq(&mut o.style, def.style);
            }
            *o != HLineStyle::default()
        });
        self.fills.retain(|id, o| {
            if let Some(def) = meta.fill(id) {
                prune_eq(&mut o.visible, def.visible);
                prune_eq(&mut o.color, Rgba::from_core(def.color));
                if o.color2.is_some() && o.color2 == def.color2.map(Rgba::from_core) {
                    o.color2 = None;
                }
            }
            *o != FillStyle::default()
        });
    }

    /// [`Self::prune`]d copy.
    pub fn pruned(&self, meta: &StudyMeta) -> Self {
        let mut s = self.clone();
        s.prune(meta);
        s
    }
}

fn prune_eq<T: PartialEq>(v: &mut Option<T>, default: T) {
    if v.as_ref() == Some(&default) {
        *v = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tt_core::StudyKind;

    #[test]
    fn defaults_come_from_the_metadata() {
        for kind in StudyKind::ALL {
            let meta = kind.meta();
            let s = IndicatorStyle::default();
            for def in meta.plots {
                let r = s.plot(def);
                assert_eq!(
                    r.color().to_core(),
                    def.default.color,
                    "{kind:?} {}",
                    def.id
                );
                assert_eq!(r.visible, def.default.visible);
                assert_eq!(r.plot_type, def.default.plot_type);
                assert_eq!(r.colors().len(), def.palette.len().max(1));
                assert!(!r.is_overridden(0));
            }
            for def in meta.hlines {
                let r = s.hline(def);
                assert!(r.visible);
                assert_eq!((r.value, r.style), (def.value, def.style));
            }
            for def in meta.fills {
                assert_eq!(s.fill(def).color.to_core(), def.color);
            }
        }
    }

    #[test]
    fn overrides_resolve_per_palette_entry_and_prune_to_nothing() {
        let meta = StudyKind::Macd.meta();
        let hist = meta.plot("hist").unwrap();
        let mut s = IndicatorStyle::default();
        s.set_plot_color("hist", 2, Rgba::rgb(1, 2, 3));
        s.plot_mut("hist").width = Some(9);
        let r = s.plot(hist);
        assert_eq!(r.palette(2), Rgba::rgb(1, 2, 3));
        assert_eq!(
            r.palette(0).to_core(),
            hist.palette[0].1,
            "others keep defaults"
        );
        assert!(r.is_overridden(2) && !r.is_overridden(0));
        assert_eq!(r.width, MAX_WIDTH, "clamped");

        // Setting everything back to the defaults leaves nothing stored.
        s.set_plot_color("hist", 2, Rgba::from_core(hist.palette[2].1));
        s.plot_mut("hist").width = Some(1);
        s.prune(meta);
        assert_eq!(s, IndicatorStyle::default());
        // Unknown ids survive (written by a newer version).
        s.plot_mut("future").visible = Some(false);
        s.prune(meta);
        assert!(s.plots.contains_key("future"));
    }

    #[test]
    fn sparse_serde_round_trip() {
        assert_eq!(
            serde_json::to_string(&IndicatorStyle::default()).unwrap(),
            r#"{"precision":null,"labels_on_price_scale":true,"values_in_status_line":true,"inputs_in_status_line":true}"#
        );
        let mut s = IndicatorStyle::default();
        s.set_plot_color("rsi", 0, Rgba::new(1, 2, 3, 128));
        s.hline_mut("upper").value = Some(75.0);
        s.fill_mut("bg").visible = Some(false);
        s.precision = Some(3);
        s.values_in_status_line = false;
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains(r##""colors":["#01020380"]"##), "{json}");
        assert_eq!(serde_json::from_str::<IndicatorStyle>(&json).unwrap(), s);
        // Missing fields take the defaults.
        assert_eq!(
            serde_json::from_str::<IndicatorStyle>("{}").unwrap(),
            IndicatorStyle::default()
        );
    }
}
