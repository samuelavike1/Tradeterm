//! Persistent user settings, stored as JSON in the OS config directory
//! (e.g. `%APPDATA%\tradeterm\config\settings.json`,
//! `~/Library/Application Support/tradeterm/settings.json`,
//! `~/.config/tradeterm/settings.json`).

use egui::Color32;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tracing::{debug, warn};
use tt_chart::{ChartType, ScaleMode};
use tt_core::Timeframe;
use tt_theme::{ThemeMode, ThemeOverrides};

/// Changes are written once settings have been stable for this long.
pub const SAVE_DEBOUNCE: Duration = Duration::from_millis(750);

/// One chart tab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TabSettings {
    pub symbol: String,
    pub timeframe: Timeframe,
    pub chart_type: ChartType,
    pub scale_mode: ScaleMode,
    pub show_volume: bool,
}

impl Default for TabSettings {
    fn default() -> Self {
        Self {
            symbol: "BTCUSDT".to_owned(),
            timeframe: Timeframe::M1,
            chart_type: ChartType::Candles,
            scale_mode: ScaleMode::Linear,
            show_volume: true,
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
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::System,
            up_color: None,
            down_color: None,
            tabs: vec![TabSettings::default()],
            active_tab: 0,
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
    pub fn default_path() -> Option<PathBuf> {
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
                    show_volume: false,
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
        s.tabs[0].show_volume = false;
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
