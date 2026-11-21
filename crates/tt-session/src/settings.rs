//! Persistent user settings, stored as JSON in the OS config directory
//! (e.g. `%APPDATA%\tradeterm\config\settings.json`,
//! `~/Library/Application Support/tradeterm/settings.json`,
//! `~/.config/tradeterm/settings.json`).

use crate::fsutil::{DebouncedFile, Poll};
use ecolor::Color32;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tracing::warn;
use tt_chart::{ChartType, Indicator, IndicatorStyle, Rgba, ScaleMode};
use tt_core::{Study, StudyKind, Timeframe};
use tt_theme::{ThemeMode, ThemeOverrides};

/// Environment variable that overrides the settings file location.
pub const SETTINGS_ENV: &str = "TRADETERM_SETTINGS";

/// Current `AppSettings::schema_version`. Files without the field are version 1.
pub const SCHEMA_VERSION: u32 = 1;

/// Changes are written once settings have been stable for this long.
pub const SAVE_DEBOUNCE: Duration = Duration::from_millis(750);

/// A recently opened symbol, shown when the search field is empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentSymbol {
    pub symbol: String,
    #[serde(default)]
    pub base: String,
    #[serde(default)]
    pub quote: String,
}

/// Most recent symbols kept.
pub const MAX_RECENT: usize = 12;

/// One indicator on a chart.
///
/// Files written before per-plot styles stored `colors`, one `[r, g, b, a]`
/// (straight alpha) per output position. They are migrated into `style` on
/// load (see [`legacy_style`]) and never written again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, from = "IndicatorConfigWire")]
pub struct IndicatorConfig {
    pub study: Study,
    pub visible: bool,
    /// Per-plot / level / fill style; sparse (only what differs from the
    /// study's defaults is written).
    pub style: IndicatorStyle,
    /// Relative pane height (pane indicators).
    pub pane_weight: f32,
    /// Pane collapsed to a strip.
    pub pane_collapsed: bool,
}

/// What an [`IndicatorConfig`] is read from: the current fields plus the
/// legacy `colors`.
#[derive(Deserialize)]
#[serde(default)]
struct IndicatorConfigWire {
    study: Study,
    visible: bool,
    colors: Vec<[u8; 4]>,
    style: IndicatorStyle,
    pane_weight: f32,
    pane_collapsed: bool,
}

impl Default for IndicatorConfigWire {
    fn default() -> Self {
        let d = IndicatorConfig::default();
        Self {
            study: d.study,
            visible: d.visible,
            colors: Vec::new(),
            style: d.style,
            pane_weight: d.pane_weight,
            pane_collapsed: d.pane_collapsed,
        }
    }
}

impl From<IndicatorConfigWire> for IndicatorConfig {
    fn from(w: IndicatorConfigWire) -> Self {
        let mut style = w.style;
        if !w.colors.is_empty() && style.plots.is_empty() {
            let legacy = legacy_style(&w.study, &w.colors);
            style.plots = legacy.plots;
            for (id, f) in legacy.fills {
                style.fills.entry(id).or_insert(f);
            }
        }
        style.prune(w.study.meta());
        Self {
            study: w.study,
            visible: w.visible,
            style,
            pane_weight: w.pane_weight,
            pane_collapsed: w.pane_collapsed,
        }
    }
}

/// Plot ids of the legacy `colors` list by output position. **Frozen**: it
/// describes files already written, so it must never follow later changes
/// to the study metadata. `None`: never user-editable (Volume columns
/// followed the theme). Kinds added later never wrote `colors`.
fn legacy_color_ids(kind: StudyKind) -> &'static [Option<&'static str>] {
    match kind {
        StudyKind::Volume => &[None, Some("vol_ma")],
        StudyKind::Sma | StudyKind::Ema => &[Some("plot")],
        StudyKind::BollingerBands => &[Some("basis"), Some("upper"), Some("lower")],
        StudyKind::Vwap => &[Some("vwap")],
        StudyKind::Rsi => &[Some("rsi")],
        StudyKind::Macd => &[Some("hist"), Some("macd"), Some("signal")],
        StudyKind::Stochastic => &[Some("k"), Some("d")],
        StudyKind::Atr => &[Some("atr")],
        StudyKind::Obv => &[Some("obv")],
        #[allow(unreachable_patterns)]
        _ => &[],
    }
}

/// Fills the legacy renderer drew in output 0's color at 8 % opacity
/// (frozen like [`legacy_color_ids`]).
fn legacy_tinted_fill(kind: StudyKind) -> Option<&'static str> {
    match kind {
        StudyKind::BollingerBands | StudyKind::Rsi | StudyKind::Stochastic => Some("bg"),
        _ => None,
    }
}

/// The style that reproduces a legacy `colors` list: each color on its
/// plot id; the fill tinted by output 0 follows it; the MACD histogram's
/// faded "fall above" column is its color at 45 % (the legacy rule). The
/// caller prunes entries equal to the defaults.
pub fn legacy_style(study: &Study, colors: &[[u8; 4]]) -> IndicatorStyle {
    let kind = study.kind();
    let mut style = IndicatorStyle::default();
    for (id, c) in legacy_color_ids(kind).iter().zip(colors) {
        if let Some(id) = id {
            style.set_plot_color(id, 0, Rgba(*c));
        }
    }
    if let Some(&[r, g, b, _]) = colors.first() {
        let alpha = |a: f32| (a * 255.0).round() as u8;
        if let Some(fill) = legacy_tinted_fill(kind) {
            style.fill_mut(fill).color = Some(Rgba::new(r, g, b, alpha(0.08)));
        }
        if kind == StudyKind::Macd {
            style.set_plot_color("hist", 1, Rgba::new(r, g, b, alpha(0.45)));
        }
    }
    style
}

impl Default for IndicatorConfig {
    fn default() -> Self {
        Self::from_indicator(&Indicator::new(StudyKind::Volume.default_study()))
    }
}

impl IndicatorConfig {
    pub fn from_indicator(ind: &Indicator) -> Self {
        Self {
            study: ind.study.clone(),
            visible: ind.visible,
            style: ind.style.pruned(ind.study.meta()),
            pane_weight: ind.pane_weight,
            pane_collapsed: ind.pane_collapsed,
        }
    }

    pub fn to_indicator(&self) -> Indicator {
        let mut ind = Indicator::new(self.study.clone());
        ind.visible = self.visible;
        ind.pane_collapsed = self.pane_collapsed;
        ind.style = self.style.clone();
        if self.pane_weight.is_finite() && self.pane_weight > 0.0 {
            ind.pane_weight = self.pane_weight.clamp(0.05, 20.0);
        }
        ind
    }
}

/// An indicator's inputs and style without runtime or pane fields: what
/// "Save as default" and named presets store.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndicatorPreset {
    pub study: Study,
    /// Sparse, like [`IndicatorConfig::style`].
    #[serde(default)]
    pub style: IndicatorStyle,
}

impl IndicatorPreset {
    /// Inputs and (pruned) style of `ind`.
    pub fn of(ind: &Indicator) -> Self {
        Self {
            study: ind.study.clone(),
            style: ind.style.pruned(ind.study.meta()),
        }
    }

    /// Factory settings of `kind`.
    pub fn factory(kind: StudyKind) -> Self {
        Self {
            study: kind.default_study(),
            style: IndicatorStyle::default(),
        }
    }

    pub fn kind(&self) -> StudyKind {
        self.study.kind()
    }

    /// Applies inputs and style to `ind` (identity, visibility and pane
    /// fields stay).
    pub fn apply_to(&self, ind: &mut Indicator) {
        ind.study = self.study.clone();
        ind.style = self.style.clone();
    }
}

/// A user-named preset of one indicator kind (a tradeterm extension of
/// TradingView's Defaults menu).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedPreset {
    pub name: String,
    pub preset: IndicatorPreset,
}

/// Longest preset name kept.
pub const MAX_PRESET_NAME: usize = 64;

/// User defaults by kind ("Save as default").
pub type IndicatorDefaults = std::collections::BTreeMap<StudyKind, IndicatorPreset>;
/// Named presets by kind.
pub type IndicatorPresets = std::collections::BTreeMap<StudyKind, Vec<NamedPreset>>;

/// A new indicator of `kind`: the user default if one was saved, else the
/// factory settings.
pub fn new_indicator(kind: StudyKind, defaults: &IndicatorDefaults) -> Indicator {
    let mut ind = Indicator::new(kind.default_study());
    if let Some(p) = defaults.get(&kind).filter(|p| p.kind() == kind) {
        p.apply_to(&mut ind);
    }
    ind
}

/// `name` trimmed and cut to [`MAX_PRESET_NAME`] characters (`None` if
/// empty).
pub fn preset_name(name: &str) -> Option<String> {
    let name: String = name.trim().chars().take(MAX_PRESET_NAME).collect();
    let name = name.trim_end().to_owned();
    (!name.is_empty()).then_some(name)
}

/// Saves `preset` under `name` in its kind's list. Names are unique,
/// compared case-insensitively: saving under an existing name replaces that
/// preset in place. Returns `false` for an empty name.
pub fn save_preset(presets: &mut IndicatorPresets, name: &str, preset: IndicatorPreset) -> bool {
    let Some(name) = preset_name(name) else {
        return false;
    };
    let list = presets.entry(preset.kind()).or_default();
    let key = name.to_lowercase();
    match list.iter_mut().find(|p| p.name.to_lowercase() == key) {
        Some(p) => {
            p.name = name;
            p.preset = preset;
        }
        None => list.push(NamedPreset { name, preset }),
    }
    true
}

/// Deletes preset `name` of `kind` (case-insensitive). Returns whether one
/// was removed.
pub fn delete_preset(presets: &mut IndicatorPresets, kind: StudyKind, name: &str) -> bool {
    let Some(list) = presets.get_mut(&kind) else {
        return false;
    };
    let key = name.trim().to_lowercase();
    let before = list.len();
    list.retain(|p| p.name.to_lowercase() != key);
    let removed = list.len() != before;
    if list.is_empty() {
        presets.remove(&kind);
    }
    removed
}

/// User defaults per indicator kind ("Save as default"). Read leniently:
/// an entry for a kind this build does not know, or whose study is of
/// another kind than its key, is skipped instead of failing the file.
fn tolerant_defaults<'de, D>(
    d: D,
) -> Result<std::collections::BTreeMap<StudyKind, IndicatorPreset>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = std::collections::BTreeMap::<String, serde_json::Value>::deserialize(d)?;
    Ok(raw
        .into_iter()
        .filter_map(|(k, v)| {
            let kind: StudyKind = serde_json::from_value(serde_json::Value::String(k.clone()))
                .map_err(|e| warn!("skipping indicator default {k}: {e}"))
                .ok()?;
            let p: IndicatorPreset = serde_json::from_value(v)
                .map_err(|e| warn!("skipping indicator default {k}: {e}"))
                .ok()?;
            (p.kind() == kind).then_some((kind, p))
        })
        .collect())
}

/// Named presets per indicator kind, read element by element like
/// [`tolerant_defaults`].
fn tolerant_presets<'de, D>(
    d: D,
) -> Result<std::collections::BTreeMap<StudyKind, Vec<NamedPreset>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = std::collections::BTreeMap::<String, serde_json::Value>::deserialize(d)?;
    Ok(raw
        .into_iter()
        .filter_map(|(k, v)| {
            let kind: StudyKind = serde_json::from_value(serde_json::Value::String(k.clone()))
                .map_err(|e| warn!("skipping indicator presets {k}: {e}"))
                .ok()?;
            let list: Vec<NamedPreset> = match v {
                serde_json::Value::Array(items) => items
                    .into_iter()
                    .filter_map(|v| {
                        serde_json::from_value::<NamedPreset>(v)
                            .map_err(|e| warn!("skipping indicator preset: {e}"))
                            .ok()
                    })
                    .filter(|p| p.preset.kind() == kind && !p.name.trim().is_empty())
                    .collect(),
                _ => return None,
            };
            (!list.is_empty()).then_some((kind, list))
        })
        .collect())
}

/// Deserializes a list element by element, skipping (with a warning) the
/// entries that fail, so one unknown entry, such as a study kind written by
/// a newer version, does not reject the whole settings file.
fn tolerant_vec<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let raw = Vec::<serde_json::Value>::deserialize(d)?;
    Ok(raw
        .into_iter()
        .filter_map(|v| match serde_json::from_value(v) {
            Ok(t) => Some(t),
            Err(e) => {
                warn!("skipping unreadable settings entry: {e}");
                None
            }
        })
        .collect())
}

/// One chart tab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TabSettings {
    pub symbol: String,
    pub timeframe: Timeframe,
    pub chart_type: ChartType,
    pub scale_mode: ScaleMode,
    /// Indicators in draw order. New charts start with Volume, like
    /// TradingView's default layout. Entries that fail to load (e.g. a study
    /// kind from a newer version) are skipped, not the whole file.
    #[serde(deserialize_with = "tolerant_vec")]
    pub indicators: Vec<IndicatorConfig>,
    /// Price-pane legend collapsed.
    pub legend_collapsed: bool,
    /// "Invert scale" of the price axis.
    pub invert_scale: bool,
    /// Price pane collapsed to a strip.
    pub main_collapsed: bool,
    /// This chart's own settings (TradingView keeps settings per chart).
    /// `None` = follows the user default, [`AppSettings::chart_style`]; old
    /// files have no `style`, so every tab follows the global style.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<tt_chart::ChartStyle>,
    /// Pre-indicator volume toggle; only read, to migrate old files.
    #[serde(skip_serializing)]
    pub show_volume: Option<bool>,
}

impl Default for TabSettings {
    fn default() -> Self {
        Self {
            symbol: "BTCUSDT".to_owned(),
            timeframe: Timeframe::M1,
            chart_type: ChartType::Candles,
            scale_mode: ScaleMode::Linear,
            indicators: vec![IndicatorConfig::default()],
            legend_collapsed: false,
            invert_scale: false,
            main_collapsed: false,
            style: None,
            show_volume: None,
        }
    }
}

impl TabSettings {
    /// The style this tab draws with: its own, or the user default.
    pub fn effective_style<'a>(
        &'a self,
        default: &'a tt_chart::ChartStyle,
    ) -> &'a tt_chart::ChartStyle {
        self.style.as_ref().unwrap_or(default)
    }
}

/// A tab's own style for `style`: `None` (follow the user default) when it
/// equals `default`, so tabs that look like the default keep following it.
pub fn tab_style_override(
    style: &tt_chart::ChartStyle,
    default: &tt_chart::ChartStyle,
) -> Option<tt_chart::ChartStyle> {
    (style != default).then(|| style.clone())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    /// File format version, for migrations in [`Self::normalized`]. Missing = 1.
    pub schema_version: u32,
    pub theme_mode: ThemeMode,
    /// `None` = palette default. Stored as `[r, g, b]`.
    pub up_color: Option<[u8; 3]>,
    pub down_color: Option<[u8; 3]>,
    /// Open chart tabs, left to right. Never empty after [`Self::normalized`].
    pub tabs: Vec<TabSettings>,
    pub active_tab: usize,
    /// Most recent first.
    pub recent_symbols: Vec<RecentSymbol>,
    /// Intervals shown as toolbar buttons.
    pub favorite_timeframes: Vec<Timeframe>,
    /// User-added intervals (beyond the built-in lists).
    pub custom_timeframes: Vec<Timeframe>,
    /// Collapsed sections of the interval menu ("seconds", "minutes", …).
    pub collapsed_timeframe_sections: Vec<String>,
    /// Chart types pinned to the toolbar as quick buttons.
    pub favorite_chart_types: Vec<ChartType>,
    /// Starred entries of the indicators dialog (unknown kinds are skipped).
    #[serde(deserialize_with = "tolerant_vec")]
    pub favorite_indicators: Vec<StudyKind>,
    /// "Save as default" per indicator kind: new instances start from it.
    #[serde(
        deserialize_with = "tolerant_defaults",
        skip_serializing_if = "std::collections::BTreeMap::is_empty"
    )]
    pub indicator_defaults: std::collections::BTreeMap<StudyKind, IndicatorPreset>,
    /// Named presets per indicator kind, in creation order.
    #[serde(
        deserialize_with = "tolerant_presets",
        skip_serializing_if = "std::collections::BTreeMap::is_empty"
    )]
    pub indicator_presets: std::collections::BTreeMap<StudyKind, Vec<NamedPreset>>,
    /// The user default chart settings (TradingView's Settings dialog): new
    /// tabs start from it and tabs without their own `style` follow it.
    pub chart_style: tt_chart::ChartStyle,
    /// Drawings per symbol (shown on every timeframe of that symbol).
    /// Loaded leniently: a drawing this build can't read is skipped and
    /// kept as raw JSON (written back on save); it never fails the file.
    #[serde(deserialize_with = "tt_chart::drawings::deserialize_drawing_map")]
    pub drawings: tt_chart::DrawingMap,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            theme_mode: ThemeMode::System,
            up_color: None,
            down_color: None,
            tabs: vec![TabSettings::default()],
            active_tab: 0,
            recent_symbols: Vec::new(),
            favorite_timeframes: Timeframe::ALL.to_vec(),
            custom_timeframes: Vec::new(),
            collapsed_timeframe_sections: Vec::new(),
            favorite_chart_types: Vec::new(),
            favorite_indicators: Vec::new(),
            indicator_defaults: Default::default(),
            indicator_presets: Default::default(),
            chart_style: tt_chart::ChartStyle::default(),
            drawings: Default::default(),
        }
    }
}

impl AppSettings {
    /// Guarantees at least one tab and a valid active index.
    pub fn normalized(mut self) -> Self {
        if self.schema_version > SCHEMA_VERSION {
            warn!(
                version = self.schema_version,
                "settings file is from a newer version; unknown fields are dropped on save"
            );
        }
        // Migrations by version go here (none yet beyond the field-level ones below).
        self.schema_version = SCHEMA_VERSION;
        if self.tabs.is_empty() {
            self.tabs.push(TabSettings::default());
        }
        self.active_tab = self.active_tab.min(self.tabs.len() - 1);
        for tab in &mut self.tabs {
            // Files from before indicators had a volume on/off switch and no
            // indicator list (so they got the default Volume indicator).
            if tab.show_volume.take() == Some(false) {
                tab.indicators
                    .retain(|i| i.study.kind() != StudyKind::Volume);
            }
        }
        self
    }

    pub fn overrides(&self) -> ThemeOverrides {
        let c = |v: Option<[u8; 3]>| v.map(|[r, g, b]| Color32::from_rgb(r, g, b));
        ThemeOverrides {
            up: c(self.up_color),
            down: c(self.down_color),
        }
    }

    pub fn set_overrides(&mut self, o: ThemeOverrides) {
        let c = |v: Option<Color32>| {
            v.map(|c| {
                let [r, g, b, _] = c.to_srgba_unmultiplied();
                [r, g, b]
            })
        };
        self.up_color = c(o.up);
        self.down_color = c(o.down);
    }
}

/// Loads settings at startup and writes them back, debounced, on a
/// background thread (see [`DebouncedFile`]).
pub struct SettingsStore {
    file: DebouncedFile<AppSettings>,
    corrupt_backup: Option<PathBuf>,
}

impl SettingsStore {
    /// `$TRADETERM_SETTINGS` if set (e.g. for a portable install or test
    /// runs that must not touch the user's settings), otherwise the OS config
    /// directory from the `directories` crate.
    pub fn default_path() -> Option<PathBuf> {
        if let Some(p) = std::env::var_os(SETTINGS_ENV).filter(|p| !p.is_empty()) {
            return Some(PathBuf::from(p));
        }
        directories::ProjectDirs::from("", "", "tradeterm")
            .map(|d| d.config_dir().join("settings.json"))
    }

    /// Loads from `path`; missing files fall back to defaults. An unparseable
    /// file is renamed to `settings.json.corrupt-<unix-ms>` first, so it is
    /// never overwritten. If it can't be read or moved aside, saving is
    /// disabled for this session.
    pub fn load(path: Option<PathBuf>) -> (AppSettings, Self) {
        let (settings, path, corrupt_backup) = match path {
            None => (None, None, None),
            Some(p) => match read_settings(&p) {
                Read::Missing => (None, Some(p), None),
                Read::Ok(s) => (Some(*s), Some(p), None),
                Read::Unreadable => (None, None, None),
                Read::Corrupt => match crate::fsutil::backup_corrupt(&p) {
                    Ok(backup) => {
                        warn!(
                            path = %p.display(),
                            backup = %backup.display(),
                            "invalid settings file moved aside; starting with defaults"
                        );
                        (None, Some(p), Some(backup))
                    }
                    Err(e) => {
                        warn!(
                            path = %p.display(),
                            "invalid settings file could not be moved aside ({e}); settings will not be saved"
                        );
                        (None, None, None)
                    }
                },
            },
        };
        let settings = settings.unwrap_or_default().normalized();
        let file = DebouncedFile::new(path, SAVE_DEBOUNCE, Some(&settings));
        let store = Self {
            file,
            corrupt_backup,
        };
        (settings, store)
    }

    /// The settings file, if settings are saved this session.
    pub fn path(&self) -> Option<&Path> {
        self.file.path()
    }

    /// Where an unparseable settings file was moved at load, if any (for a
    /// one-time notice).
    pub fn corrupt_backup(&self) -> Option<&Path> {
        self.corrupt_backup.as_deref()
    }

    /// Records that settings changed (cheap). Returns `Some(wait)` when the
    /// caller must call [`Self::poll`] after `wait`.
    pub fn mark_changed(&mut self, now: Instant) -> Option<Duration> {
        self.file.mark_changed(now)
    }

    /// [`Poll::Due`]: settings have been stable for [`SAVE_DEBOUNCE`]; build a
    /// snapshot and pass it to [`Self::save`]. [`Poll::Wait`]: poll again later.
    pub fn poll(&mut self, now: Instant) -> Poll {
        self.file.poll(now)
    }

    /// Queues a background write (skipped if the content is unchanged).
    pub fn save(&mut self, settings: AppSettings) {
        self.file.save(settings);
    }

    /// Writes now if anything changed and waits for all writes to finish
    /// (used on exit).
    pub fn flush_with(&mut self, settings: impl FnOnce() -> AppSettings) {
        self.file.flush_with(settings);
    }

    /// Blocks until queued writes are on disk.
    pub fn wait_idle(&self) {
        self.file.wait_idle();
    }
}

enum Read {
    Missing,
    Ok(Box<AppSettings>),
    Unreadable,
    Corrupt,
}

fn read_settings(path: &Path) -> Read {
    let bytes = match std::fs::read(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Read::Missing,
        Err(e) => {
            warn!(path = %path.display(), "could not read settings ({e}); settings will not be saved");
            return Read::Unreadable;
        }
    };
    match serde_json::from_slice(&bytes) {
        Ok(s) => Read::Ok(Box::new(s)),
        Err(e) => {
            warn!(path = %path.display(), "invalid settings file: {e}");
            Read::Corrupt
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trip_and_partial_files() {
        let mut s = AppSettings {
            theme_mode: ThemeMode::Light,
            tabs: vec![
                TabSettings::default(),
                TabSettings {
                    symbol: "ETHUSDT".into(),
                    timeframe: Timeframe::H4,
                    chart_type: ChartType::HeikinAshi,
                    scale_mode: ScaleMode::Logarithmic,
                    indicators: vec![
                        IndicatorConfig::from_indicator(&Indicator::new(
                            StudyKind::Rsi.default_study(),
                        )),
                        IndicatorConfig::default(),
                    ],
                    legend_collapsed: true,
                    invert_scale: false,
                    main_collapsed: false,
                    style: None,
                    show_volume: None,
                },
            ],
            active_tab: 1,
            ..Default::default()
        };
        s.set_overrides(ThemeOverrides {
            up: Some(Color32::from_rgb(1, 2, 3)),
            down: None,
        });
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains(r#""timeframe":"4h""#), "{json}");
        let back: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
        assert_eq!(back.overrides().up, Some(Color32::from_rgb(1, 2, 3)));

        // Unknown/missing fields fall back to defaults (forward/backward compatible).
        let partial: AppSettings =
            serde_json::from_str(r#"{"tabs":[{"symbol":"SOLUSDT"}],"future_field":1}"#).unwrap();
        assert_eq!(partial.tabs[0].symbol, "SOLUSDT");
        assert_eq!(partial.tabs[0].chart_type, ChartType::Candles);

        // No tabs / out-of-range active tab are repaired.
        let broken: AppSettings = serde_json::from_str(r#"{"tabs":[],"active_tab":7}"#).unwrap();
        let fixed = broken.normalized();
        assert_eq!(fixed.tabs.len(), 1);
        assert_eq!(fixed.active_tab, 0);
    }

    #[test]
    fn indicators_round_trip_and_volume_migration() {
        let mut rsi = Indicator::new(StudyKind::Rsi.default_study());
        rsi.set_color(0, Color32::from_rgb(1, 2, 3));
        rsi.style.hline_mut("upper").value = Some(80.0);
        rsi.style.precision = Some(1);
        rsi.visible = false;
        let cfg = IndicatorConfig::from_indicator(&rsi);
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(
            !json.contains("[[1,2,3"),
            "no legacy colors written: {json}"
        );
        let back = serde_json::from_str::<IndicatorConfig>(&json)
            .unwrap()
            .to_indicator();
        assert_eq!(back.study, rsi.study);
        assert_eq!(back.color(0), Color32::from_rgb(1, 2, 3));
        assert_eq!(back.style, rsi.style);
        assert!(!back.visible);
        // Default styles are not written at all.
        let plain =
            IndicatorConfig::from_indicator(&Indicator::new(StudyKind::Macd.default_study()));
        let json = serde_json::to_string(&plain).unwrap();
        assert!(
            !json.contains("plots") && !json.contains("colors"),
            "{json}"
        );

        // Old files: no indicator list. Volume on → Volume indicator; off → none.
        let old: AppSettings = serde_json::from_str(
            r#"{"tabs":[{"symbol":"A","show_volume":true},{"symbol":"B","show_volume":false},{"symbol":"C"}]}"#,
        )
        .unwrap();
        let s = old.normalized();
        let kinds = |t: &TabSettings| {
            t.indicators
                .iter()
                .map(|i| i.study.kind())
                .collect::<Vec<_>>()
        };
        assert_eq!(kinds(&s.tabs[0]), [StudyKind::Volume]);
        assert!(kinds(&s.tabs[1]).is_empty());
        assert_eq!(kinds(&s.tabs[2]), [StudyKind::Volume]);
        // The legacy field is never written back.
        assert!(!serde_json::to_string(&s).unwrap().contains("show_volume"));
        // An explicitly empty list stays empty.
        let empty: AppSettings = serde_json::from_str(r#"{"tabs":[{"indicators":[]}]}"#).unwrap();
        assert!(empty.normalized().tabs[0].indicators.is_empty());
    }

    /// A tab's indicator list as written before per-plot styles (default colors).
    const CURRENT_INDICATORS: &str = r#"[
        {"study":{"type":"Volume","ma_length":20,"color_by_prev_close":false,"show_ma":true},
         "visible":true,"colors":[[8,153,129,255],[33,150,243,255]],
         "pane_weight":1.0,"pane_collapsed":false},
        {"study":{"type":"Rsi","length":21,"source":"Hlc3"},"visible":false,
         "colors":[[126,87,194,255]],"pane_weight":2.5,"pane_collapsed":true},
        {"study":{"type":"Vwap"},"visible":true,"colors":[],"pane_weight":1.0,
         "pane_collapsed":false}
    ]"#;

    #[test]
    fn legacy_indicator_format_loads_unchanged() {
        let json = format!(r#"{{"tabs":[{{"symbol":"A","indicators":{CURRENT_INDICATORS}}}]}}"#);
        let s: AppSettings = serde_json::from_str(&json).unwrap();
        let ind = &s.tabs[0].indicators;
        assert_eq!(
            ind,
            &[
                IndicatorConfig {
                    study: Study::Volume {
                        ma_length: 20,
                        color_by_prev_close: false,
                        show_ma: true,
                    },
                    visible: true,
                    style: IndicatorStyle::default(),
                    pane_weight: 1.0,
                    pane_collapsed: false,
                },
                IndicatorConfig {
                    study: Study::Rsi {
                        length: 21,
                        source: tt_core::Source::Hlc3,
                    },
                    visible: false,
                    style: IndicatorStyle::default(),
                    pane_weight: 2.5,
                    pane_collapsed: true,
                },
                IndicatorConfig {
                    study: Study::Vwap,
                    visible: true,
                    style: IndicatorStyle::default(),
                    pane_weight: 1.0,
                    pane_collapsed: false,
                },
            ]
        );
        let rsi = ind[1].to_indicator();
        assert_eq!(rsi.color(0), Color32::from_rgb(126, 87, 194));
        assert_eq!(rsi.pane_weight, 2.5);
        // Writing it back gives the same file content.
        let back: Vec<IndicatorConfig> =
            serde_json::from_str(&serde_json::to_string(ind).unwrap()).unwrap();
        assert_eq!(&back, ind);
    }

    /// Every kind with customized legacy `colors` (as the old version wrote
    /// them: the full list per indicator) draws the same colors after the
    /// migration, and is written back without `colors`.
    #[test]
    fn legacy_custom_colors_migrate_by_plot_id() {
        let s: AppSettings =
            serde_json::from_str(include_str!("fixtures/indicators_v1_custom.json")).unwrap();
        let inds: Vec<Indicator> = s.tabs[0]
            .indicators
            .iter()
            .map(IndicatorConfig::to_indicator)
            .collect();
        assert_eq!(inds.len(), 10);
        let color = |k: StudyKind, id: &str, i: usize| {
            let ind = inds.iter().find(|x| x.study.kind() == k).unwrap();
            let def = ind.study.meta().plot(id).unwrap();
            ind.style.plot(def).palette(i).0
        };
        let fill = |k: StudyKind, id: &str| {
            let ind = inds.iter().find(|x| x.study.kind() == k).unwrap();
            ind.style.fill(ind.study.meta().fill(id).unwrap()).color.0
        };
        let overridden = |k: StudyKind| {
            let ind = inds.iter().find(|x| x.study.kind() == k).unwrap();
            ind.style.plots.keys().cloned().collect::<Vec<_>>()
        };
        use StudyKind::*;
        // Volume: columns keep the theme (output 0 was never editable).
        assert_eq!(overridden(Volume), ["vol_ma"]);
        assert_eq!(color(Volume, "vol_ma", 0), [10, 20, 30, 255]);
        assert_eq!(color(Sma, "plot", 0), [200, 100, 0, 128], "opacity kept");
        assert!(overridden(Ema).is_empty(), "default colors store nothing");
        assert!(!inds[2].visible);
        assert_eq!(color(BollingerBands, "basis", 0), [255, 0, 0, 255]);
        assert_eq!(color(BollingerBands, "upper", 0), [0, 255, 0, 255]);
        assert_eq!(color(BollingerBands, "lower", 0), [0, 0, 255, 255]);
        assert_eq!(fill(BollingerBands, "bg"), [255, 0, 0, 20], "basis at 8 %");
        assert_eq!(color(Vwap, "vwap", 0), [9, 9, 9, 255]);
        assert_eq!(color(Rsi, "rsi", 0), [255, 235, 59, 255]);
        assert_eq!(fill(Rsi, "bg"), [255, 235, 59, 20]);
        assert_eq!(color(Macd, "hist", 0), [0, 200, 0, 255]);
        assert_eq!(color(Macd, "hist", 1), [0, 200, 0, 115], "faded at 45 %");
        let hist = StudyKind::Macd.meta().plot("hist").unwrap();
        assert_eq!(
            color(Macd, "hist", 2),
            hist.palette[2].1,
            "negative side unchanged"
        );
        assert_eq!(color(Macd, "hist", 3), hist.palette[3].1);
        assert_eq!(color(Macd, "macd", 0), [1, 2, 3, 255]);
        assert_eq!(color(Macd, "signal", 0), [4, 5, 6, 255]);
        assert_eq!(color(Stochastic, "k", 0), [7, 8, 9, 255]);
        assert_eq!(overridden(Stochastic), ["k"]);
        assert_eq!(fill(Stochastic, "bg"), [7, 8, 9, 20]);
        assert_eq!(color(Atr, "atr", 0), [100, 100, 100, 255]);
        assert!(overridden(Obv).is_empty());
        assert!(inds[6].pane_collapsed);
        assert_eq!(inds[5].pane_weight, 2.0);

        // Written back: no legacy colors, and it reloads to the same thing.
        let json = serde_json::to_string(&s).unwrap();
        assert!(!json.contains(r#""colors":[["#), "{json}");
        let again: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(again.tabs[0].indicators, s.tabs[0].indicators);
        // A file written now and read by the old version: it ignores
        // `style` (unknown field) and uses its default colors.
        assert!(json.contains(r#""style":{"#));
    }

    #[test]
    fn unknown_indicator_is_skipped_not_the_file() {
        // A study kind from a newer version, a known kind with a broken
        // field and an unknown source: only those entries are dropped.
        let json = r#"{
            "theme_mode":"Light",
            "favorite_indicators":["Rsi","Supertrend","Macd"],
            "tabs":[{"symbol":"ETHUSDT","indicators":[
                {"study":{"type":"Rsi","length":14,"source":"Close"},"visible":true},
                {"study":{"type":"Supertrend","atr_length":10,"factor":3.0},"visible":true},
                {"study":{"type":"Sma","length":"nine","source":"Close"}},
                {"study":{"type":"Ema","length":9,"source":"Hlcc5"}},
                {"study":{"type":"Macd","fast":12,"slow":26,"signal":9,"source":"Close"}}
            ]}]
        }"#;
        let s: AppSettings = serde_json::from_str(json).unwrap();
        assert_eq!(s.theme_mode, ThemeMode::Light);
        assert_eq!(s.tabs[0].symbol, "ETHUSDT");
        let kinds: Vec<_> = s.tabs[0]
            .indicators
            .iter()
            .map(|i| i.study.kind())
            .collect();
        assert_eq!(kinds, [StudyKind::Rsi, StudyKind::Macd]);
        assert_eq!(s.favorite_indicators, [StudyKind::Rsi, StudyKind::Macd]);

        // Through the store as well (the path users hit at startup).
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, json).unwrap();
        let (loaded, _) = SettingsStore::load(Some(path));
        assert_eq!(loaded.tabs[0].indicators.len(), 2);
        assert_eq!(loaded.theme_mode, ThemeMode::Light);

        // A non-list value is still an error (handled as an invalid file).
        assert!(serde_json::from_str::<TabSettings>(r#"{"indicators":5}"#).is_err());
    }

    #[test]
    fn store_debounces_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");

        let (mut s, mut store) = SettingsStore::load(Some(path.clone()));
        assert_eq!(s, AppSettings::default());

        let t0 = Instant::now();
        s.tabs[0].symbol = "ETHUSDT".into();
        assert_eq!(store.mark_changed(t0), Some(SAVE_DEBOUNCE));
        assert_eq!(
            store.poll(t0 + Duration::from_millis(500)),
            Poll::Wait(Duration::from_millis(250))
        );

        // Another change restarts the timer (no second check is scheduled).
        s.tabs[0].legend_collapsed = true;
        assert_eq!(store.mark_changed(t0 + Duration::from_millis(500)), None);
        assert!(matches!(
            store.poll(t0 + Duration::from_millis(1000)),
            Poll::Wait(_)
        ));
        store.wait_idle();
        assert!(!path.exists(), "must not save before the debounce elapses");
        assert_eq!(store.poll(t0 + Duration::from_millis(1300)), Poll::Due);
        store.save(s.clone());
        store.wait_idle();
        assert!(path.exists());

        let (loaded, _) = SettingsStore::load(Some(path.clone()));
        assert_eq!(loaded, s);

        // Nothing changed: idle, and identical content is never rewritten.
        assert_eq!(store.poll(t0 + Duration::from_secs(10)), Poll::Idle);
        std::fs::remove_file(&path).unwrap();
        store.mark_changed(t0 + Duration::from_secs(10));
        store.save(s.clone());
        store.wait_idle();
        assert!(!path.exists());

        // Quit flushes pending changes synchronously.
        s.tabs[0].symbol = "SOLUSDT".into();
        store.mark_changed(t0 + Duration::from_secs(11));
        store.flush_with(|| s.clone());
        assert_eq!(SettingsStore::load(Some(path.clone())).0, s);
        // Not dirty: flush doesn't build a snapshot.
        store.flush_with(|| unreachable!());
    }

    #[test]
    fn corrupt_file_is_backed_up_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{ not json").unwrap();
        let (s, mut store) = SettingsStore::load(Some(path.clone()));
        assert_eq!(s, AppSettings::default());

        let backup = store.corrupt_backup().expect("backed up").to_owned();
        assert!(!path.exists());
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), "{ not json");
        let name = backup.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("settings.json.corrupt-"), "{name}");

        let mut changed = s.clone();
        changed.tabs[0].symbol = "BNBUSDT".into();
        store.mark_changed(Instant::now());
        store.flush_with(|| changed.clone());
        assert_eq!(SettingsStore::load(Some(path)).0, changed);
        // The backup survives the new write.
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), "{ not json");
    }

    #[test]
    fn indicator_defaults_and_presets_round_trip_and_drive_new_indicators() {
        let mut s = AppSettings::default();
        let json = serde_json::to_string(&s).unwrap();
        assert!(
            !json.contains("indicator_defaults") && !json.contains("indicator_presets"),
            "empty maps are not written"
        );

        // Save as default: RSI 21 on high, purple line 2 px.
        let mut rsi = Indicator::new(StudyKind::Rsi.default_study());
        assert!(rsi.study.set("length", tt_core::InputValue::Int(21)));
        rsi.style.plot_mut("rsi").width = Some(2);
        rsi.style.plot_mut("rsi").visible = Some(true); // = default, pruned
        let preset = IndicatorPreset::of(&rsi);
        assert!(preset.style.plots["rsi"].visible.is_none(), "pruned");
        s.indicator_defaults.insert(StudyKind::Rsi, preset.clone());

        // Named presets: unique case-insensitively, replaced in place.
        assert!(save_preset(
            &mut s.indicator_presets,
            "  Fast ",
            preset.clone()
        ));
        let mut slow = preset.clone();
        slow.study.set("length", tt_core::InputValue::Int(50));
        assert!(save_preset(&mut s.indicator_presets, "Slow", slow.clone()));
        assert!(save_preset(&mut s.indicator_presets, "FAST", slow.clone()));
        assert!(!save_preset(&mut s.indicator_presets, "   ", slow.clone()));
        let list = &s.indicator_presets[&StudyKind::Rsi];
        assert_eq!(
            list.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["FAST", "Slow"]
        );
        assert_eq!(list[0].preset, slow);

        let back: AppSettings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);

        // New instances use the default; other kinds use factory settings.
        let ind = new_indicator(StudyKind::Rsi, &back.indicator_defaults);
        assert_eq!(ind.study.get("length"), Some(tt_core::InputValue::Int(21)));
        assert_eq!(ind.style, preset.style);
        let sma = new_indicator(StudyKind::Sma, &back.indicator_defaults);
        assert_eq!(sma.study, StudyKind::Sma.default_study());

        // Delete; the last one removes the kind's entry.
        assert!(delete_preset(
            &mut s.indicator_presets,
            StudyKind::Rsi,
            "fast"
        ));
        assert!(!delete_preset(
            &mut s.indicator_presets,
            StudyKind::Rsi,
            "fast"
        ));
        assert!(delete_preset(
            &mut s.indicator_presets,
            StudyKind::Rsi,
            "Slow"
        ));
        assert!(s.indicator_presets.is_empty());
    }

    #[test]
    fn indicator_defaults_and_presets_load_tolerantly() {
        // An unknown kind (newer version), a broken study, a mismatched key
        // and an empty name are skipped; the good entries and the rest of
        // the file survive.
        let json = r#"{
            "favorite_indicators": ["Rsi"],
            "indicator_defaults": {
                "Supertrend": {"study": {"type": "Supertrend", "length": 10}},
                "Sma": {"study": {"type": "Sma", "length": "x"}},
                "Ema": {"study": {"type": "Sma", "length": 5, "source": "Close"}},
                "Rsi": {"study": {"type": "Rsi", "length": 21, "source": "Close"},
                        "style": {"precision": 3}}
            },
            "indicator_presets": {
                "Future": [{"name": "a", "preset": {"study": {"type": "Future"}}}],
                "Macd": "not a list",
                "Rsi": [
                    {"name": "ok", "preset": {"study": {"type": "Rsi", "length": 7, "source": "Close"}}},
                    {"name": "bad", "preset": {"study": {"type": "Nope"}}},
                    {"name": "  ", "preset": {"study": {"type": "Rsi", "length": 7, "source": "Close"}}},
                    {"name": "kind", "preset": {"study": {"type": "Atr", "length": 7}}}
                ]
            }
        }"#;
        let s: AppSettings = serde_json::from_str(json).unwrap();
        assert_eq!(s.favorite_indicators, [StudyKind::Rsi]);
        assert_eq!(
            s.indicator_defaults.keys().copied().collect::<Vec<_>>(),
            [StudyKind::Rsi]
        );
        assert_eq!(
            s.indicator_defaults[&StudyKind::Rsi].style.precision,
            Some(3)
        );
        let rsi = &s.indicator_presets[&StudyKind::Rsi];
        assert_eq!(rsi.len(), 1);
        assert_eq!(rsi[0].name, "ok");
        assert_eq!(s.indicator_presets.len(), 1);
        // Files without the fields load as before.
        let old: AppSettings = serde_json::from_str(r#"{"active_tab":0}"#).unwrap();
        assert!(old.indicator_defaults.is_empty() && old.indicator_presets.is_empty());
    }

    #[test]
    fn per_tab_style_resolution_and_compat() {
        // Old files: one global style, no per-tab style → every tab follows it.
        let old: AppSettings = serde_json::from_str(
            r#"{"chart_style":{"font_size":14.0,"wick":false},"tabs":[{"symbol":"A"},{"symbol":"B"}]}"#,
        )
        .unwrap();
        let old = old.normalized();
        assert_eq!(
            old.chart_style.scales.text_size, 14.0,
            "v1 global style is the user default"
        );
        for t in &old.tabs {
            assert!(t.style.is_none());
            assert_eq!(t.effective_style(&old.chart_style), &old.chart_style);
        }
        // Tabs following the default write no `style` key.
        let json = serde_json::to_value(&old).unwrap();
        for tab in json["tabs"].as_array().unwrap() {
            assert!(tab.get("style").is_none(), "{tab}");
        }

        // A tab with its own style keeps it through a round trip; the other
        // tab still follows the default.
        let mut s = old.clone();
        let mut own = s.chart_style.clone();
        own.symbol.line.width = 4.0;
        s.tabs[1].style = tab_style_override(&own, &s.chart_style);
        assert!(s.tabs[1].style.is_some());
        let back: AppSettings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
        assert_eq!(
            back.tabs[1]
                .effective_style(&back.chart_style)
                .symbol
                .line
                .width,
            4.0
        );
        assert_eq!(
            back.tabs[0].effective_style(&back.chart_style),
            &back.chart_style
        );

        // A style equal to the default means "follow the default".
        assert_eq!(tab_style_override(&s.chart_style, &s.chart_style), None);
        // A per-tab v1-shaped style migrates like the global one.
        let t: TabSettings = serde_json::from_str(r#"{"style":{"dotted_grid":false}}"#).unwrap();
        assert_eq!(
            t.style.unwrap().canvas.grid_v_kind,
            tt_chart::style::LineKind::Solid
        );
    }

    #[test]
    fn schema_version_defaults_and_is_written() {
        let old: AppSettings = serde_json::from_str(r#"{"active_tab":0}"#).unwrap();
        assert_eq!(old.schema_version, 1);
        assert_eq!(AppSettings::default().schema_version, SCHEMA_VERSION);
        let json = serde_json::to_string(&AppSettings::default()).unwrap();
        assert!(json.contains(r#""schema_version":1"#), "{json}");
        // A newer file still loads; it is normalized to the current version.
        let newer: AppSettings = serde_json::from_str(r#"{"schema_version":99}"#).unwrap();
        assert_eq!(newer.normalized().schema_version, SCHEMA_VERSION);
    }
}
