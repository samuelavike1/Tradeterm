//! Watchlists (TradingView's right panel, docs/specs/right-panel.md): named
//! lists of symbols with collapsible sections, the column options, the sort,
//! and which right-sidebar panel is open. Pure data: loaded tolerantly from
//! the settings file, edited by the UI.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use tracing::warn;
use tt_core::Symbol;

/// One row of a list: a symbol or a section caption.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WatchItem {
    Symbol {
        symbol: String,
        /// Base / quote asset (logo, name), when known.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        base: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        quote: String,
    },
    Section {
        section: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        collapsed: bool,
    },
}

impl WatchItem {
    pub fn symbol(symbol: &str, base: &str, quote: &str) -> Self {
        WatchItem::Symbol {
            symbol: symbol.to_owned(),
            base: base.to_owned(),
            quote: quote.to_owned(),
        }
    }

    pub fn section(title: &str) -> Self {
        WatchItem::Section {
            section: title.to_owned(),
            collapsed: false,
        }
    }

    pub fn symbol_str(&self) -> Option<&str> {
        match self {
            WatchItem::Symbol { symbol, .. } => Some(symbol),
            WatchItem::Section { .. } => None,
        }
    }

    pub fn is_section(&self) -> bool {
        matches!(self, WatchItem::Section { .. })
    }
}

/// A named list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Watchlist {
    pub name: String,
    /// Rows in order; read entry by entry (an unreadable one is skipped).
    #[serde(default, deserialize_with = "tolerant_items")]
    pub items: Vec<WatchItem>,
}

impl Watchlist {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            items: Vec::new(),
        }
    }

    pub fn contains(&self, symbol: &str) -> bool {
        self.items.iter().any(|i| i.symbol_str() == Some(symbol))
    }

    /// Symbols in list order.
    pub fn symbols(&self) -> impl Iterator<Item = &str> {
        self.items.iter().filter_map(WatchItem::symbol_str)
    }

    /// Inserts `item` after row `after` (at the end without one); `false`
    /// if it is already in the list or invalid. A collapsed section it
    /// lands in opens, so the new row shows.
    pub fn add_symbol(&mut self, item: WatchItem, after: Option<usize>) -> bool {
        let Some(sym) = item.symbol_str() else {
            return false;
        };
        if !Symbol::new(sym).is_valid() || self.contains(sym) {
            return false;
        }
        let at = match after {
            Some(i) if i < self.items.len() => i + 1,
            _ => self.items.len(),
        };
        self.items.insert(at, item);
        if let Some(WatchItem::Section { collapsed, .. }) =
            self.items[..at].iter_mut().rev().find(|i| i.is_section())
        {
            *collapsed = false;
        }
        true
    }

    /// Inserts a section caption above row `at` (at the end when out of
    /// range).
    pub fn add_section(&mut self, at: Option<usize>, title: &str) {
        let at = at.unwrap_or(self.items.len()).min(self.items.len());
        self.items.insert(at, WatchItem::section(title));
    }

    pub fn remove(&mut self, at: usize) -> Option<WatchItem> {
        (at < self.items.len()).then(|| self.items.remove(at))
    }

    /// Moves row `from` so it lands before the row that was at `to`
    /// (`to == len` moves it to the end).
    pub fn move_item(&mut self, from: usize, to: usize) -> bool {
        let n = self.items.len();
        if from >= n || to > n || to == from || to == from + 1 {
            return false;
        }
        let item = self.items.remove(from);
        let to = if to > from { to - 1 } else { to };
        self.items.insert(to, item);
        true
    }

    pub fn toggle_section(&mut self, at: usize) {
        if let Some(WatchItem::Section { collapsed, .. }) = self.items.get_mut(at) {
            *collapsed = !*collapsed;
        }
    }

    /// Row indices shown, in display order: rows under a collapsed section
    /// are hidden; with a sort, symbols are sorted within each section
    /// (sections keep their place, like TradingView).
    pub fn visible_rows(
        &self,
        sort: Option<Sort>,
        value: impl Fn(&str, SortKey) -> Option<f64>,
    ) -> Vec<usize> {
        let mut out = Vec::with_capacity(self.items.len());
        let mut run: Vec<usize> = Vec::new();
        let mut hidden = false;
        let flush = |run: &mut Vec<usize>, out: &mut Vec<usize>| {
            if let Some(s) = sort {
                run.sort_by(|&a, &b| {
                    let (sa, sb) = (
                        self.items[a].symbol_str().unwrap_or(""),
                        self.items[b].symbol_str().unwrap_or(""),
                    );
                    compare(sa, sb, s, &value)
                });
            }
            out.append(run);
        };
        for (i, item) in self.items.iter().enumerate() {
            match item {
                WatchItem::Section { collapsed, .. } => {
                    flush(&mut run, &mut out);
                    out.push(i);
                    hidden = *collapsed;
                }
                WatchItem::Symbol { .. } if !hidden => run.push(i),
                WatchItem::Symbol { .. } => {}
            }
        }
        flush(&mut run, &mut out);
        out
    }
}

/// Orders two symbols by `s`; rows without a value go last either way.
fn compare(a: &str, b: &str, s: Sort, value: &impl Fn(&str, SortKey) -> Option<f64>) -> Ordering {
    let ord = if s.key == SortKey::Symbol {
        a.cmp(b)
    } else {
        match (value(a, s.key), value(b, s.key)) {
            (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
            (Some(_), None) => return Ordering::Less,
            (None, Some(_)) => return Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    };
    if s.descending { ord.reverse() } else { ord }
}

/// Watchlist columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortKey {
    Symbol,
    Last,
    Change,
    ChangePct,
    Volume,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sort {
    pub key: SortKey,
    #[serde(default)]
    pub descending: bool,
}

impl Sort {
    /// A header click: ascending, then descending, then unsorted
    /// (TradingView's cycle). Symbol starts ascending, numbers descending.
    pub fn cycle(current: Option<Sort>, key: SortKey) -> Option<Sort> {
        let first_desc = key != SortKey::Symbol;
        match current {
            Some(s) if s.key == key && s.descending == first_desc => Some(Sort {
                key,
                descending: !first_desc,
            }),
            Some(s) if s.key == key => None,
            _ => Some(Sort {
                key,
                descending: first_desc,
            }),
        }
    }
}

/// Which columns show (TradingView's "Customize columns" and "Symbol
/// display").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Columns {
    pub last: bool,
    pub change: bool,
    pub change_pct: bool,
    pub volume: bool,
    pub logo: bool,
    /// Show the asset name instead of the ticker.
    pub name: bool,
}

impl Default for Columns {
    fn default() -> Self {
        Self {
            last: true,
            change: true,
            change_pct: true,
            volume: false,
            logo: true,
            name: false,
        }
    }
}

/// Every list, the active one and the view options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Watchlists {
    #[serde(deserialize_with = "tolerant_lists")]
    pub lists: Vec<Watchlist>,
    pub active: usize,
    #[serde(deserialize_with = "tolerant_or_default")]
    pub columns: Columns,
    #[serde(deserialize_with = "tolerant_or_default")]
    pub sort: Option<Sort>,
}

impl Default for Watchlists {
    fn default() -> Self {
        let mut list = Watchlist::new("Watchlist");
        for (s, b) in [
            ("BTCUSDT", "BTC"),
            ("ETHUSDT", "ETH"),
            ("SOLUSDT", "SOL"),
            ("BNBUSDT", "BNB"),
            ("XRPUSDT", "XRP"),
        ] {
            list.items.push(WatchItem::symbol(s, b, "USDT"));
        }
        Self {
            lists: vec![list],
            active: 0,
            columns: Columns::default(),
            sort: None,
        }
    }
}

impl Watchlists {
    /// At least one list, a valid active index, symbols upper-cased, valid
    /// and once per list, names non-empty.
    pub fn normalized(mut self) -> Self {
        if self.lists.is_empty() {
            self.lists = Watchlists::default().lists;
        }
        for (k, list) in self.lists.iter_mut().enumerate() {
            if list.name.trim().is_empty() {
                list.name = format!("Watchlist {}", k + 1);
            }
            let mut seen = std::collections::HashSet::new();
            list.items.retain_mut(|item| match item {
                WatchItem::Symbol { symbol, .. } => {
                    *symbol = symbol.trim().to_ascii_uppercase();
                    Symbol::new(symbol.as_str()).is_valid() && seen.insert(symbol.clone())
                }
                WatchItem::Section { .. } => true,
            });
        }
        self.active = self.active.min(self.lists.len() - 1);
        self
    }

    pub fn current(&self) -> &Watchlist {
        &self.lists[self.active.min(self.lists.len() - 1)]
    }

    pub fn current_mut(&mut self) -> &mut Watchlist {
        let i = self.active.min(self.lists.len() - 1);
        &mut self.lists[i]
    }

    /// A name not used by another list: `base`, else `base (2)`, …
    pub fn unique_name(&self, base: &str) -> String {
        let base = base.trim();
        let base = if base.is_empty() { "Watchlist" } else { base };
        let taken = |n: &str| self.lists.iter().any(|l| l.name == n);
        if !taken(base) {
            return base.to_owned();
        }
        (2..)
            .map(|k| format!("{base} ({k})"))
            .find(|n| !taken(n))
            .expect("unbounded")
    }

    /// Creates an empty list and makes it active.
    pub fn create(&mut self, name: &str) {
        let name = self.unique_name(name);
        self.lists.push(Watchlist::new(&name));
        self.active = self.lists.len() - 1;
    }

    /// Copies the active list ("Make a copy") and makes the copy active.
    pub fn duplicate(&mut self) {
        let mut copy = self.current().clone();
        copy.name = self.unique_name(&format!("{} copy", copy.name));
        self.lists.push(copy);
        self.active = self.lists.len() - 1;
    }

    pub fn rename(&mut self, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() || name == self.current().name {
            return false;
        }
        let name = self.unique_name(name);
        self.current_mut().name = name;
        true
    }

    /// Deletes the active list (never the last one).
    pub fn delete_current(&mut self) -> bool {
        if self.lists.len() <= 1 {
            return false;
        }
        self.lists.remove(self.active);
        self.active = self.active.min(self.lists.len() - 1);
        true
    }

    /// Every symbol of every list, once (the quote stream's set).
    pub fn all_symbols(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for s in self.lists.iter().flat_map(|l| l.symbols()) {
            if !out.iter().any(|o| o == s) {
                out.push(s.to_owned());
            }
        }
        out
    }
}

/// Which right-sidebar panel is open, its width and the object-tree tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanelKind {
    #[default]
    Watchlist,
    Alerts,
    ObjectTree,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RightPanelSettings {
    #[serde(deserialize_with = "tolerant_or_default")]
    pub open: Option<PanelKind>,
    pub width: f32,
    /// The object tree panel shows the data window.
    pub data_window: bool,
}

impl Default for RightPanelSettings {
    fn default() -> Self {
        Self {
            open: None,
            width: tt_theme::metrics::W_PANEL,
            data_window: false,
        }
    }
}

impl RightPanelSettings {
    /// Width clamped to the panel limits (a bad value gets the default).
    pub fn clamped_width(w: f32) -> f32 {
        use tt_theme::metrics::{W_PANEL, W_PANEL_MAX, W_PANEL_MIN};
        if w.is_finite() {
            w.clamp(W_PANEL_MIN, W_PANEL_MAX)
        } else {
            W_PANEL
        }
    }
}

/// Reads a value, falling back to its default (with a warning) when it is
/// unreadable.
pub(crate) fn tolerant_or_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    let raw = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(raw)
        .map_err(|e| warn!("ignoring an unreadable settings value: {e}"))
        .unwrap_or_default())
}

fn tolerant_items<'de, D>(d: D) -> Result<Vec<WatchItem>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = serde_json::Value::deserialize(d)?;
    let serde_json::Value::Array(items) = raw else {
        warn!("ignoring unreadable watchlist items");
        return Ok(Vec::new());
    };
    Ok(items
        .into_iter()
        .filter_map(|v| {
            serde_json::from_value(v)
                .map_err(|e| warn!("skipping watchlist entry: {e}"))
                .ok()
        })
        .collect())
}

fn tolerant_lists<'de, D>(d: D) -> Result<Vec<Watchlist>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = serde_json::Value::deserialize(d)?;
    let serde_json::Value::Array(lists) = raw else {
        warn!("ignoring unreadable watchlists");
        return Ok(Vec::new());
    };
    Ok(lists
        .into_iter()
        .filter_map(|v| {
            serde_json::from_value(v)
                .map_err(|e| warn!("skipping watchlist: {e}"))
                .ok()
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(items: &[&str]) -> Watchlist {
        let mut l = Watchlist::new("L");
        for s in items {
            if let Some(t) = s.strip_prefix('#') {
                l.items.push(WatchItem::section(t));
            } else {
                l.items.push(WatchItem::symbol(s, "", ""));
            }
        }
        l
    }

    fn names(l: &Watchlist, rows: &[usize]) -> Vec<String> {
        rows.iter()
            .map(|&i| match &l.items[i] {
                WatchItem::Symbol { symbol, .. } => symbol.clone(),
                WatchItem::Section { section, .. } => format!("#{section}"),
            })
            .collect()
    }

    #[test]
    fn round_trips_and_writes_compact_json() {
        let mut w = Watchlists::default();
        w.current_mut().add_section(Some(2), "Alts");
        w.current_mut().toggle_section(2);
        w.sort = Some(Sort {
            key: SortKey::ChangePct,
            descending: true,
        });
        let json = serde_json::to_string(&w).unwrap();
        assert!(
            json.contains(r#"{"section":"Alts","collapsed":true}"#),
            "{json}"
        );
        assert!(json.contains(r#"{"symbol":"BTCUSDT","base":"BTC","quote":"USDT"}"#));
        let back: Watchlists = serde_json::from_str(&json).unwrap();
        assert_eq!(back, w);
    }

    #[test]
    fn tolerant_loading_skips_bad_entries_and_repairs() {
        let json = r#"{
            "lists": [
                {"name": "A", "items": [{"symbol":"btcusdt"}, 42, {"section":"S"},
                                        {"symbol":"BTCUSDT"}, {"symbol":"bad sym!"}, {"weird":1}]},
                "not a list",
                {"name": "", "items": "nope"}
            ],
            "active": 9,
            "columns": "garbage",
            "sort": {"key": "unknown"},
            "future": true
        }"#;
        let w: Watchlists = serde_json::from_str::<Watchlists>(json)
            .unwrap()
            .normalized();
        assert_eq!(w.lists.len(), 2);
        assert_eq!(w.lists[0].symbols().collect::<Vec<_>>(), ["BTCUSDT"]);
        assert_eq!(w.lists[0].items.len(), 2);
        assert_eq!(w.lists[1].name, "Watchlist 2");
        assert!(w.lists[1].items.is_empty());
        assert_eq!(w.active, 1);
        assert_eq!(w.columns, Columns::default());
        assert_eq!(w.sort, None);

        // Nothing at all, or no lists: the default list.
        let empty: Watchlists = serde_json::from_str::<Watchlists>(r#"{"lists": []}"#)
            .unwrap()
            .normalized();
        assert_eq!(empty, Watchlists::default());
        let wrong: Watchlists = serde_json::from_str::<Watchlists>(r#"{"lists": 5}"#)
            .unwrap()
            .normalized();
        assert_eq!(wrong.lists.len(), 1);
    }

    #[test]
    fn right_panel_settings_are_tolerant() {
        let s: RightPanelSettings =
            serde_json::from_str(r#"{"open":"object_tree","width":420}"#).unwrap();
        assert_eq!(s.open, Some(PanelKind::ObjectTree));
        assert_eq!(s.width, 420.0);
        let s: RightPanelSettings = serde_json::from_str(r#"{"open":"calendar"}"#).unwrap();
        assert_eq!(s.open, None);
        assert_eq!(RightPanelSettings::clamped_width(5.0), 240.0);
        assert_eq!(RightPanelSettings::clamped_width(f32::NAN), 300.0);
    }

    #[test]
    fn list_edits() {
        let mut l = list(&["AAAUSDT", "BBBUSDT"]);
        assert!(!l.add_symbol(WatchItem::symbol("AAAUSDT", "", ""), None));
        assert!(l.add_symbol(WatchItem::symbol("CCCUSDT", "", ""), Some(0)));
        assert_eq!(
            l.symbols().collect::<Vec<_>>(),
            ["AAAUSDT", "CCCUSDT", "BBBUSDT"]
        );
        assert!(l.move_item(0, 3));
        assert_eq!(
            l.symbols().collect::<Vec<_>>(),
            ["CCCUSDT", "BBBUSDT", "AAAUSDT"]
        );
        assert!(l.move_item(2, 0));
        assert_eq!(
            l.symbols().collect::<Vec<_>>(),
            ["AAAUSDT", "CCCUSDT", "BBBUSDT"]
        );
        assert!(!l.move_item(1, 2), "dropping a row on its own slot");
        l.add_section(Some(1), "Mid");
        assert!(l.items[1].is_section());
        assert!(l.remove(1).unwrap().is_section());
        assert!(l.remove(9).is_none());
        // Adding into a collapsed section opens it.
        l.add_section(None, "Closed");
        l.toggle_section(3);
        assert!(l.add_symbol(WatchItem::symbol("DDDUSDT", "", ""), None));
        assert_eq!(l.items[3], WatchItem::section("Closed"));
    }

    #[test]
    fn lists_create_rename_copy_delete() {
        let mut w = Watchlists::default();
        w.create("Watchlist");
        assert_eq!(w.current().name, "Watchlist (2)");
        assert!(w.rename("Majors"));
        w.duplicate();
        assert_eq!(w.current().name, "Majors copy");
        assert_eq!(w.lists.len(), 3);
        assert!(w.delete_current());
        assert!(w.delete_current());
        assert!(!w.delete_current(), "the last list stays");
        assert_eq!(w.active, 0);
    }

    #[test]
    fn sorting_within_sections_and_collapsing() {
        let l = list(&["BUSDT", "AUSDT", "#S", "DUSDT", "CUSDT", "EUSDT"]);
        let last = |s: &str, _| match s {
            "AUSDT" => Some(1.0),
            "BUSDT" => Some(2.0),
            "CUSDT" => Some(5.0),
            "DUSDT" => Some(3.0),
            _ => None,
        };
        let none = l.visible_rows(None, last);
        assert_eq!(
            names(&l, &none),
            ["BUSDT", "AUSDT", "#S", "DUSDT", "CUSDT", "EUSDT"]
        );
        let desc = Some(Sort {
            key: SortKey::Last,
            descending: true,
        });
        let rows = l.visible_rows(desc, last);
        // Sorted within each section; no value (EUSDT) last.
        assert_eq!(
            names(&l, &rows),
            ["BUSDT", "AUSDT", "#S", "CUSDT", "DUSDT", "EUSDT"]
        );
        let asc = Some(Sort {
            key: SortKey::Last,
            descending: false,
        });
        let rows = l.visible_rows(asc, last);
        assert_eq!(
            names(&l, &rows),
            ["AUSDT", "BUSDT", "#S", "DUSDT", "CUSDT", "EUSDT"]
        );
        let by_name = Some(Sort {
            key: SortKey::Symbol,
            descending: false,
        });
        let rows = l.visible_rows(by_name, last);
        assert_eq!(
            names(&l, &rows),
            ["AUSDT", "BUSDT", "#S", "CUSDT", "DUSDT", "EUSDT"]
        );

        let mut l = l;
        l.toggle_section(2);
        let rows = l.visible_rows(None, last);
        assert_eq!(names(&l, &rows), ["BUSDT", "AUSDT", "#S"]);
    }

    #[test]
    fn sort_cycle_matches_tradingview() {
        let s = Sort::cycle(None, SortKey::Last);
        assert_eq!(
            s,
            Some(Sort {
                key: SortKey::Last,
                descending: true
            })
        );
        let s = Sort::cycle(s, SortKey::Last);
        assert_eq!(
            s,
            Some(Sort {
                key: SortKey::Last,
                descending: false
            })
        );
        assert_eq!(Sort::cycle(s, SortKey::Last), None);
        let s = Sort::cycle(None, SortKey::Symbol);
        assert_eq!(
            s,
            Some(Sort {
                key: SortKey::Symbol,
                descending: false
            })
        );
    }
}
