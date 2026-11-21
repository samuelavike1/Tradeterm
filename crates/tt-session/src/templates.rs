//! Chart-settings templates (TradingView's Settings › Template ▾), stored in
//! `chart-templates.json` next to `settings.json`.
//!
//! A template holds a whole [`ChartStyle`] (every page, every chart type's
//! style and inputs), not per-chart state (symbol, interval, chart type,
//! indicators, drawings). Templates are global.
//!
//! The user default ("Save as default") is not in this file: it is
//! `AppSettings::chart_style`.
//!
//! Loading is tolerant: a missing file is empty, an unparseable file is moved
//! aside (`chart-templates.json.corrupt-<ms>`), and an entry this build can't
//! read is skipped but kept as raw JSON and written back, so a newer build's
//! templates survive. Styles go through `ChartStyle`'s migration on load.
//! Writes are immediate and atomic (template edits are rare).

use crate::fsutil;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::warn;
use tt_chart::ChartStyle;

/// File name, next to `settings.json`.
pub const TEMPLATES_FILE: &str = "chart-templates.json";
/// Current [`ChartTemplates::version`].
pub const TEMPLATES_VERSION: u32 = 1;
/// Longest template name, in characters.
pub const MAX_NAME_CHARS: usize = 64;

/// One saved template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartTemplate {
    pub name: String,
    #[serde(default)]
    pub style: ChartStyle,
    /// Unix milliseconds of the last save.
    #[serde(default)]
    pub modified_ms: i64,
}

/// The file's content.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct ChartTemplates {
    pub version: u32,
    /// Sorted by name, case-insensitively.
    pub templates: Vec<ChartTemplate>,
    /// Entries this build could not read, written back unchanged.
    #[serde(skip)]
    pub unknown: Vec<serde_json::Value>,
}

impl<'de> Deserialize<'de> for ChartTemplates {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(default)]
        struct Wire {
            version: u32,
            templates: Vec<serde_json::Value>,
        }
        impl Default for Wire {
            fn default() -> Self {
                Self {
                    version: TEMPLATES_VERSION,
                    templates: Vec::new(),
                }
            }
        }
        let w = Wire::deserialize(d)?;
        let mut out = ChartTemplates {
            version: w.version,
            ..Default::default()
        };
        for raw in w.templates {
            match serde_json::from_value::<ChartTemplate>(raw.clone()) {
                Ok(t) => match validate_name(&t.name) {
                    Ok(name) if out.find(&name).is_none() => {
                        out.templates.push(ChartTemplate { name, ..t })
                    }
                    _ => {
                        warn!(name = %t.name, "skipping chart template with an invalid or duplicate name");
                        out.unknown.push(raw);
                    }
                },
                Err(e) => {
                    warn!("skipping unreadable chart template: {e}");
                    out.unknown.push(raw);
                }
            }
        }
        out.sort();
        Ok(out)
    }
}

impl ChartTemplates {
    fn sort(&mut self) {
        self.templates
            .sort_by_cached_key(|t| (t.name.to_lowercase(), t.name.clone()));
    }

    /// Index of the template named `name` (case-insensitive).
    fn find(&self, name: &str) -> Option<usize> {
        let key = name.trim().to_lowercase();
        self.templates
            .iter()
            .position(|t| t.name.to_lowercase() == key)
    }

    fn to_json(&self) -> serde_json::Value {
        let mut list: Vec<serde_json::Value> = self
            .templates
            .iter()
            .filter_map(|t| serde_json::to_value(t).ok())
            .collect();
        list.extend(self.unknown.iter().cloned());
        serde_json::json!({ "version": TEMPLATES_VERSION, "templates": list })
    }
}

/// Why a name was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameError {
    Empty,
    TooLong,
    /// Another template has this name (case-insensitive).
    Exists,
    /// No template has this name.
    NotFound,
}

impl NameError {
    pub fn message(self) -> &'static str {
        match self {
            NameError::Empty => "Enter a name.",
            NameError::TooLong => "The name is too long (64 characters at most).",
            NameError::Exists => "A template with this name already exists.",
            NameError::NotFound => "No template with this name.",
        }
    }
}

/// Trims `name` and checks the length rules (1–64 characters).
pub fn validate_name(name: &str) -> Result<String, NameError> {
    let name = name.trim();
    if name.is_empty() {
        Err(NameError::Empty)
    } else if name.chars().count() > MAX_NAME_CHARS {
        Err(NameError::TooLong)
    } else {
        Ok(name.to_owned())
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// Loads and writes `chart-templates.json`.
pub struct TemplateStore {
    /// None: in memory only (no config directory, or the file was unreadable).
    path: Option<PathBuf>,
    data: ChartTemplates,
}

impl TemplateStore {
    /// The templates file next to the settings file at `settings`.
    pub fn path_next_to(settings: &Path) -> PathBuf {
        settings.with_file_name(TEMPLATES_FILE)
    }

    /// An empty store that never writes (tests, no config directory).
    pub fn in_memory() -> Self {
        Self {
            path: None,
            data: ChartTemplates {
                version: TEMPLATES_VERSION,
                ..Default::default()
            },
        }
    }

    /// Loads `path`. Missing → empty; unparseable → moved aside, empty;
    /// unreadable (I/O error) → empty and never written this session.
    pub fn load(path: Option<PathBuf>) -> Self {
        let Some(path) = path else {
            return Self::in_memory();
        };
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Self {
                    path: Some(path),
                    ..Self::in_memory()
                };
            }
            Err(e) => {
                warn!(path = %path.display(), "could not read chart templates ({e}); they will not be saved");
                return Self::in_memory();
            }
        };
        match serde_json::from_slice::<ChartTemplates>(&bytes) {
            Ok(data) => Self {
                path: Some(path),
                data,
            },
            Err(e) => {
                warn!(path = %path.display(), "invalid chart templates file: {e}");
                match fsutil::backup_corrupt(&path) {
                    Ok(backup) => {
                        warn!(backup = %backup.display(), "invalid chart templates file moved aside");
                        Self {
                            path: Some(path),
                            ..Self::in_memory()
                        }
                    }
                    Err(e) => {
                        warn!(
                            "could not move the invalid chart templates file aside ({e}); templates will not be saved"
                        );
                        Self::in_memory()
                    }
                }
            }
        }
    }

    /// Saved templates, sorted by name.
    pub fn list(&self) -> &[ChartTemplate] {
        &self.data.templates
    }

    pub fn names(&self) -> Vec<String> {
        self.data.templates.iter().map(|t| t.name.clone()).collect()
    }

    /// The template named `name` (case-insensitive).
    pub fn get(&self, name: &str) -> Option<&ChartTemplate> {
        self.data.find(name).map(|i| &self.data.templates[i])
    }

    pub fn contains(&self, name: &str) -> bool {
        self.data.find(name).is_some()
    }

    /// Saves `style` as `name`, overwriting a template with the same name
    /// (the UI confirms that first). Keeps the existing name's spelling
    /// unless it changed. Returns the stored name.
    pub fn save(&mut self, name: &str, style: &ChartStyle) -> Result<String, NameError> {
        let name = validate_name(name)?;
        let t = ChartTemplate {
            name: name.clone(),
            style: style.clone(),
            modified_ms: now_ms(),
        };
        match self.data.find(&name) {
            Some(i) => self.data.templates[i] = t,
            None => self.data.templates.push(t),
        }
        self.data.sort();
        self.write();
        Ok(name)
    }

    /// Deletes `name`; false if there is none.
    pub fn delete(&mut self, name: &str) -> bool {
        let Some(i) = self.data.find(name) else {
            return false;
        };
        self.data.templates.remove(i);
        self.write();
        true
    }

    /// Renames `from` to `to` (a case-only change of the same template is
    /// allowed).
    pub fn rename(&mut self, from: &str, to: &str) -> Result<String, NameError> {
        let to = validate_name(to)?;
        let i = self.data.find(from).ok_or(NameError::NotFound)?;
        if self.data.find(&to).is_some_and(|j| j != i) {
            return Err(NameError::Exists);
        }
        let t = &mut self.data.templates[i];
        t.name = to.clone();
        t.modified_ms = now_ms();
        self.data.sort();
        self.write();
        Ok(to)
    }

    fn write(&self) {
        let Some(path) = &self.path else {
            return;
        };
        let bytes = match serde_json::to_vec_pretty(&self.data.to_json()) {
            Ok(b) => b,
            Err(e) => {
                warn!("could not serialize chart templates: {e}");
                return;
            }
        };
        if let Err(e) = fsutil::atomic_write(path, &bytes) {
            warn!(path = %path.display(), "could not write chart templates: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tt_chart::Rgba;

    fn styled(r: u8) -> ChartStyle {
        let mut s = ChartStyle::default();
        s.symbol.candles.body_up = Some(Rgba::rgb(r, 2, 3));
        s.symbol.area.width = 3.0;
        s.canvas.watermark.visible = true;
        s
    }

    #[test]
    fn save_get_rename_delete_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(TEMPLATES_FILE);
        let mut store = TemplateStore::load(Some(path.clone()));
        assert!(store.list().is_empty());

        assert_eq!(store.save("  zeta ", &styled(1)).unwrap(), "zeta");
        store.save("Alpha", &styled(2)).unwrap();
        store.save("beta", &styled(3)).unwrap();
        assert_eq!(
            store.names(),
            ["Alpha", "beta", "zeta"],
            "sorted, case-insensitive"
        );
        assert!(path.exists(), "writes are immediate");

        // Same name (any case) overwrites.
        store.save("ALPHA", &styled(9)).unwrap();
        assert_eq!(store.list().len(), 3);
        assert_eq!(store.get("alpha").unwrap().style, styled(9));

        // Reload: everything survives, including every nested field.
        let back = TemplateStore::load(Some(path.clone()));
        assert_eq!(back.list(), store.list());
        assert_eq!(back.get("zeta").unwrap().style, styled(1));

        assert_eq!(store.rename("beta", "zeta"), Err(NameError::Exists));
        assert_eq!(store.rename("nope", "x"), Err(NameError::NotFound));
        assert_eq!(store.rename("beta", "Gamma").unwrap(), "Gamma");
        assert_eq!(
            store.rename("gamma", "GAMMA").unwrap(),
            "GAMMA",
            "case-only"
        );
        assert!(store.delete("zeta"));
        assert!(!store.delete("zeta"));
        let back = TemplateStore::load(Some(path));
        assert_eq!(back.names(), ["ALPHA", "GAMMA"]);
    }

    #[test]
    fn name_rules() {
        assert_eq!(validate_name("   "), Err(NameError::Empty));
        assert_eq!(validate_name(&"x".repeat(65)), Err(NameError::TooLong));
        assert_eq!(validate_name(&"é".repeat(64)).unwrap().chars().count(), 64);
        let mut store = TemplateStore::in_memory();
        assert_eq!(
            store.save("", &ChartStyle::default()),
            Err(NameError::Empty)
        );
        assert!(store.list().is_empty());
    }

    #[test]
    fn tolerant_loading() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(TEMPLATES_FILE);
        // A good entry, a v1 (flat) style that migrates, an entry with a broken
        // style, a nameless entry, a duplicate and unknown keys.
        std::fs::write(
            &path,
            r##"{"version":1,"future":true,"templates":[
                {"name":"Good","style":{"version":2,"canvas":{"watermark":{"visible":true}}},"modified_ms":5,"extra":1},
                {"name":"Old","style":{"font_size":14.0,"dotted_grid":false,"wick":false}},
                {"name":"Broken","style":{"version":2,"symbol":{"area":{"width":"wide"}}}},
                {"style":{}},
                {"name":"good","style":{}}
            ]}"##,
        )
        .unwrap();
        let mut store = TemplateStore::load(Some(path.clone()));
        assert_eq!(store.names(), ["Good", "Old"]);
        assert!(store.get("good").unwrap().style.canvas.watermark.visible);
        let old = &store.get("old").unwrap().style;
        assert_eq!(old.scales.text_size, 14.0);
        assert!(!old.symbol.candles.wick, "v1 template migrated");

        // Saving keeps the unreadable entries for a newer build.
        store.save("New", &ChartStyle::default()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("Broken") && text.contains("wide"), "{text}");
        let back = TemplateStore::load(Some(path.clone()));
        assert_eq!(back.names(), ["Good", "New", "Old"]);

        // An unparseable file is moved aside, never overwritten.
        std::fs::write(&path, "{ nope").unwrap();
        let mut store = TemplateStore::load(Some(path.clone()));
        assert!(store.list().is_empty());
        let backups: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".corrupt-"))
            .collect();
        assert_eq!(backups.len(), 1);
        store.save("Fresh", &ChartStyle::default()).unwrap();
        assert_eq!(TemplateStore::load(Some(path)).names(), ["Fresh"]);

        // Missing file / no path: empty, saving works in memory.
        let mut mem = TemplateStore::load(None);
        mem.save("A", &ChartStyle::default()).unwrap();
        assert_eq!(mem.names(), ["A"]);
    }
}
