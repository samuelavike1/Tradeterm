//! Persistent user settings, stored as JSON in the OS config directory
//! (e.g. `%APPDATA%\tradeterm\config\settings.json`,
//! `~/Library/Application Support/tradeterm/settings.json`,
//! `~/.config/tradeterm/settings.json`).

use ecolor::Color32;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tracing::{debug, warn};
use tt_chart::{ChartType, Indicator, ScaleMode};
use tt_core::{Study, StudyKind, Timeframe};
use tt_theme::{ThemeMode, ThemeOverrides};

/// Environment variable that overrides the settings file location.
pub const SETTINGS_ENV: &str = "TRADETERM_SETTINGS";

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IndicatorConfig {
    pub study: Study,
    pub visible: bool,
    /// One `[r, g, b, a]` per output; empty = defaults. (Field-level default:
    /// the struct default is a Volume indicator with Volume's colors.)
    #[serde(default)]
    pub colors: Vec<[u8; 4]>,
    /// Relative pane height (pane indicators).
    pub pane_weight: f32,
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
            colors: ind
                .colors
                .iter()
                .map(|c| c.to_srgba_unmultiplied())
                .collect(),
            pane_weight: ind.pane_weight,
        }
    }

    pub fn to_indicator(&self) -> Indicator {
        let mut ind = Indicator::new(self.study.clone());
        ind.visible = self.visible;
        for (slot, [r, g, b, a]) in ind.colors.iter_mut().zip(&self.colors) {
            *slot = Color32::from_rgba_unmultiplied(*r, *g, *b, *a);
        }
        if self.pane_weight.is_finite() && self.pane_weight > 0.0 {
            ind.pane_weight = self.pane_weight.clamp(0.05, 20.0);
        }
        ind
    }
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
    /// TradingView's default layout.
    pub indicators: Vec<IndicatorConfig>,
    /// Price-pane legend collapsed.
    pub legend_collapsed: bool,
    /// "Invert scale" of the price axis.
    pub invert_scale: bool,
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
            show_volume: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
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
    /// Starred entries of the indicators dialog.
    pub favorite_indicators: Vec<StudyKind>,
    /// Chart settings (TradingView's Settings dialog), shared by all charts.
    pub chart_style: tt_chart::ChartStyle,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
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
            chart_style: tt_chart::ChartStyle::default(),
        }
    }
}

impl AppSettings {
    /// Guarantees at least one tab and a valid active index.
    pub fn normalized(mut self) -> Self {
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

/// Loads settings at startup and writes them back, debounced.
pub struct SettingsStore {
    path: Option<PathBuf>,
    saved: AppSettings,
    pending: Option<(AppSettings, Instant)>,
}

impl SettingsStore {
    /// Default location from the `directories` crate.
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

    /// Loads from `path`; missing or unreadable files fall back to defaults.
    pub fn load(path: Option<PathBuf>) -> (AppSettings, Self) {
        let settings = path
            .as_deref()
            .and_then(read_settings)
            .unwrap_or_default()
            .normalized();
        let store = Self {
            path,
            saved: settings.clone(),
            pending: None,
        };
        (settings, store)
    }

    /// Call every frame with the current settings. Saves once they have been
    /// unchanged for [`SAVE_DEBOUNCE`]. Returns how long until the next check
    /// is needed, so the caller can schedule a repaint.
    pub fn tick(&mut self, current: &AppSettings, now: Instant) -> Option<Duration> {
        if *current == self.saved {
            self.pending = None;
            return None;
        }
        match &self.pending {
            Some((p, since)) if p == current => {
                let elapsed = now.saturating_duration_since(*since);
                if elapsed >= SAVE_DEBOUNCE {
                    self.save_now(current);
                    None
                } else {
                    Some(SAVE_DEBOUNCE - elapsed)
                }
            }
            _ => {
                self.pending = Some((current.clone(), now));
                Some(SAVE_DEBOUNCE)
            }
        }
    }

    /// Writes immediately if anything changed (used on exit).
    pub fn flush(&mut self, current: &AppSettings) {
        if *current != self.saved {
            self.save_now(current);
        }
    }

    fn save_now(&mut self, current: &AppSettings) {
        self.pending = None;
        // Record as saved even on failure so a broken disk doesn't cause a retry storm.
        self.saved = current.clone();
        let Some(path) = &self.path else { return };
        match write_settings(path, current) {
            Ok(()) => debug!(path = %path.display(), "settings saved"),
            Err(e) => warn!(path = %path.display(), "could not save settings: {e:#}"),
        }
    }
}

fn read_settings(path: &Path) -> Option<AppSettings> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            warn!(path = %path.display(), "could not read settings: {e}");
            return None;
        }
    };
    match serde_json::from_str(&text) {
        Ok(s) => Some(s),
        Err(e) => {
            warn!(path = %path.display(), "ignoring invalid settings file: {e}");
            None
        }
    }
}

/// Atomic write: temp file in the same directory, then rename.
fn write_settings(path: &Path, settings: &AppSettings) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(settings)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
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
        rsi.colors[0] = Color32::from_rgb(1, 2, 3);
        rsi.visible = false;
        let cfg = IndicatorConfig::from_indicator(&rsi);
        let back = serde_json::from_str::<IndicatorConfig>(&serde_json::to_string(&cfg).unwrap())
            .unwrap()
            .to_indicator();
        assert_eq!(back.study, rsi.study);
        assert_eq!(back.colors[0], Color32::from_rgb(1, 2, 3));
        assert!(!back.visible);

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

    #[test]
    fn store_debounces_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");

        let (mut s, mut store) = SettingsStore::load(Some(path.clone()));
        assert_eq!(s, AppSettings::default());

        let t0 = Instant::now();
        s.tabs[0].symbol = "ETHUSDT".into();
        assert_eq!(store.tick(&s, t0), Some(SAVE_DEBOUNCE));
        assert!(!path.exists(), "must not save before the debounce elapses");

        // Another change restarts the timer.
        s.tabs[0].legend_collapsed = true;
        store.tick(&s, t0 + Duration::from_millis(500));
        store.tick(&s, t0 + Duration::from_millis(1000));
        assert!(!path.exists());
        assert_eq!(store.tick(&s, t0 + Duration::from_millis(1300)), None);
        assert!(path.exists());

        let (loaded, _) = SettingsStore::load(Some(path.clone()));
        assert_eq!(loaded, s);

        // Unchanged settings never write.
        std::fs::remove_file(&path).unwrap();
        assert_eq!(store.tick(&s, t0 + Duration::from_secs(10)), None);
        assert!(!path.exists());
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{ not json").unwrap();
        let (s, mut store) = SettingsStore::load(Some(path.clone()));
        assert_eq!(s, AppSettings::default());
        let mut changed = s.clone();
        changed.tabs[0].symbol = "BNBUSDT".into();
        store.flush(&changed);
        assert_eq!(SettingsStore::load(Some(path)).0, changed);
    }
}
