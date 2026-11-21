//! Named chart layouts (TradingView's "Manage layouts" menu), stored in
//! `layouts.json` next to `settings.json`.
//!
//! A layout is the set of chart tabs (symbol, interval, chart type,
//! indicators, panes, the tab's own style) and the active tab. The working
//! copy of the current layout is `AppSettings::tabs`; this file holds the
//! saved versions. With autosave on, the current layout is written here on
//! every settings save; with it off, only on "Save layout".
//!
//! Drawings stay per symbol across layouts (`AppSettings::drawings`), like
//! TradingView's "sync drawings globally" option.
//!
//! Loading is tolerant like the templates file: missing → empty, unparseable
//! → moved aside, unreadable → in memory for the session. Writes are atomic.

use crate::fsutil;
use crate::settings::TabSettings;
use crate::templates::{NameError, validate_name};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::warn;

/// File name, next to `settings.json`.
pub const LAYOUTS_FILE: &str = "layouts.json";
/// Name of a layout that was never saved under a name of its own.
pub const UNNAMED: &str = "Unnamed";
/// Layouts listed under RECENTLY USED.
pub const RECENT_LAYOUTS: usize = 5;

/// One saved layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub name: String,
    pub tabs: Vec<TabSettings>,
    pub active_tab: usize,
    /// Unix milliseconds of the last save.
    pub modified_ms: i64,
    /// Unix milliseconds it was last opened (RECENTLY USED order).
    pub opened_ms: i64,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            name: UNNAMED.to_owned(),
            tabs: vec![TabSettings::default()],
            active_tab: 0,
            modified_ms: 0,
            opened_ms: 0,
        }
    }
}

impl Layout {
    /// "BTCUSDT, 15" style summary of the tabs for menus and lists.
    pub fn summary(&self) -> String {
        let first = self
            .tabs
            .get(self.active_tab)
            .or(self.tabs.first())
            .map(|t| format!("{}, {}", t.symbol, t.timeframe.label()))
            .unwrap_or_default();
        match self.tabs.len() {
            0 | 1 => first,
            n => format!("{first} +{}", n - 1),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
struct LayoutsFile {
    version: u32,
    layouts: Vec<Layout>,
}

/// Loads and writes `layouts.json`.
pub struct LayoutStore {
    path: Option<PathBuf>,
    layouts: Vec<Layout>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

impl LayoutStore {
    pub fn path_next_to(settings: &Path) -> PathBuf {
        settings.with_file_name(LAYOUTS_FILE)
    }

    /// An empty store that never writes.
    pub fn in_memory() -> Self {
        Self {
            path: None,
            layouts: Vec::new(),
        }
    }

    pub fn load(path: Option<PathBuf>) -> Self {
        let Some(path) = path else {
            return Self::in_memory();
        };
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Self {
                    path: Some(path),
                    layouts: Vec::new(),
                };
            }
            Err(e) => {
                warn!(path = %path.display(), "could not read layouts ({e}); they will not be saved");
                return Self::in_memory();
            }
        };
        match serde_json::from_slice::<LayoutsFile>(&bytes) {
            Ok(f) => {
                let mut s = Self {
                    path: Some(path),
                    layouts: f.layouts,
                };
                s.layouts.retain(|l| validate_name(&l.name).is_ok());
                for l in &mut s.layouts {
                    if l.tabs.is_empty() {
                        l.tabs.push(TabSettings::default());
                    }
                    l.active_tab = l.active_tab.min(l.tabs.len() - 1);
                }
                s.sort();
                s
            }
            Err(e) => {
                warn!(path = %path.display(), "invalid layouts file: {e}");
                match fsutil::backup_corrupt(&path) {
                    Ok(_) => Self {
                        path: Some(path),
                        layouts: Vec::new(),
                    },
                    Err(_) => Self::in_memory(),
                }
            }
        }
    }

    fn sort(&mut self) {
        self.layouts
            .sort_by_cached_key(|l| (l.name.to_lowercase(), l.name.clone()));
    }

    fn find(&self, name: &str) -> Option<usize> {
        self.layouts.iter().position(|l| same(&l.name, name))
    }

    /// All layouts, sorted by name.
    pub fn list(&self) -> &[Layout] {
        &self.layouts
    }

    pub fn get(&self, name: &str) -> Option<&Layout> {
        self.find(name).map(|i| &self.layouts[i])
    }

    pub fn contains(&self, name: &str) -> bool {
        self.find(name).is_some()
    }

    /// Saves `tabs` as `name` (overwriting a layout of that name). Returns
    /// the stored name.
    pub fn save(
        &mut self,
        name: &str,
        tabs: &[TabSettings],
        active_tab: usize,
    ) -> Result<String, NameError> {
        let name = validate_name(name)?;
        let now = now_ms();
        let tabs = if tabs.is_empty() {
            vec![TabSettings::default()]
        } else {
            tabs.to_vec()
        };
        let active_tab = active_tab.min(tabs.len() - 1);
        match self.find(&name) {
            Some(i) => {
                let l = &mut self.layouts[i];
                if l.tabs == tabs && l.active_tab == active_tab {
                    return Ok(l.name.clone());
                }
                l.tabs = tabs;
                l.active_tab = active_tab;
                l.modified_ms = now;
            }
            None => self.layouts.push(Layout {
                name: name.clone(),
                tabs,
                active_tab,
                modified_ms: now,
                opened_ms: now,
            }),
        }
        self.sort();
        self.write();
        Ok(name)
    }

    /// Marks `name` as just opened (RECENTLY USED).
    pub fn touch(&mut self, name: &str) {
        if let Some(i) = self.find(name) {
            self.layouts[i].opened_ms = now_ms();
            self.write();
        }
    }

    pub fn delete(&mut self, name: &str) -> bool {
        let Some(i) = self.find(name) else {
            return false;
        };
        self.layouts.remove(i);
        self.write();
        true
    }

    /// Renames `from` to `to` (a case-only change is allowed).
    pub fn rename(&mut self, from: &str, to: &str) -> Result<String, NameError> {
        let to = validate_name(to)?;
        let i = self.find(from).ok_or(NameError::NotFound)?;
        if self.find(&to).is_some_and(|j| j != i) {
            return Err(NameError::Exists);
        }
        self.layouts[i].name = to.clone();
        self.layouts[i].modified_ms = now_ms();
        self.sort();
        self.write();
        Ok(to)
    }

    /// Most recently opened layouts other than `current`, newest first.
    pub fn recent(&self, current: &str, n: usize) -> Vec<&Layout> {
        let mut v: Vec<&Layout> = self
            .layouts
            .iter()
            .filter(|l| !same(&l.name, current))
            .collect();
        v.sort_by_key(|l| std::cmp::Reverse(l.opened_ms.max(l.modified_ms)));
        v.truncate(n);
        v
    }

    /// A free name based on `base`: "base", "base (2)", "base (3)", …
    pub fn unique_name(&self, base: &str) -> String {
        let base = base.trim();
        if !self.contains(base) {
            return base.to_owned();
        }
        (2..)
            .map(|i| format!("{base} ({i})"))
            .find(|n| !self.contains(n))
            .unwrap_or_default()
    }

    fn write(&self) {
        let Some(path) = &self.path else {
            return;
        };
        let file = LayoutsFile {
            version: 1,
            layouts: self.layouts.clone(),
        };
        match serde_json::to_vec_pretty(&file) {
            Ok(bytes) => {
                if let Err(e) = fsutil::atomic_write(path, &bytes) {
                    warn!(path = %path.display(), "could not write layouts: {e}");
                }
            }
            Err(e) => warn!("could not serialize layouts: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tt_core::Timeframe;

    fn tab(sym: &str) -> TabSettings {
        TabSettings {
            symbol: sym.to_owned(),
            timeframe: Timeframe::M15,
            ..TabSettings::default()
        }
    }

    #[test]
    fn save_rename_delete_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(LAYOUTS_FILE);
        let mut s = LayoutStore::load(Some(path.clone()));
        assert_eq!(s.save(" Main ", &[tab("ETHUSDT")], 3).unwrap(), "Main");
        s.save("alts", &[tab("SOLUSDT"), tab("BNBUSDT")], 1)
            .unwrap();
        assert_eq!(s.get("main").unwrap().active_tab, 0, "clamped");
        assert_eq!(s.get("ALTS").unwrap().summary(), "BNBUSDT, 15m +1");

        let s2 = LayoutStore::load(Some(path.clone()));
        assert_eq!(s2.list().len(), 2, "written and read back");

        assert_eq!(s.rename("main", "alts"), Err(NameError::Exists));
        assert_eq!(s.rename("main", "MAIN").unwrap(), "MAIN");
        assert_eq!(s.unique_name("alts"), "alts (2)");
        assert!(s.delete("Alts"));
        assert!(!s.contains("alts"));
        assert_eq!(LayoutStore::load(Some(path)).list().len(), 1);
    }

    #[test]
    fn recent_excludes_current_newest_first() {
        let mut s = LayoutStore::in_memory();
        for n in ["a", "b", "c"] {
            s.save(n, &[tab("BTCUSDT")], 0).unwrap();
        }
        for (i, l) in s.layouts.iter_mut().enumerate() {
            l.opened_ms = i as i64;
            l.modified_ms = 0;
        }
        let names: Vec<_> = s.recent("C", 5).iter().map(|l| l.name.clone()).collect();
        assert_eq!(names, ["b", "a"]);
    }

    #[test]
    fn corrupt_file_is_moved_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(LAYOUTS_FILE);
        std::fs::write(&path, "{nope").unwrap();
        let s = LayoutStore::load(Some(path.clone()));
        assert!(s.list().is_empty());
        assert!(!path.exists());
    }
}
