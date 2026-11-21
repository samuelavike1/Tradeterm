//! Indicator templates (docs/specs/indicators.md §6, slice I-8): a named set
//! of indicators with their settings, optionally with a symbol and an
//! interval. Applying one replaces the chart's indicators. User templates
//! live in `AppSettings::indicator_templates`; [`builtin`] ones ship with
//! the app.

use crate::settings::IndicatorConfig;
use crate::templates::{NameError, validate_name};
use serde::{Deserialize, Serialize};
use tt_core::{Source, Study, Timeframe};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IndicatorTemplate {
    pub name: String,
    pub indicators: Vec<IndicatorConfig>,
    /// "Remember symbol": applying switches to it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// "Remember interval": applying switches to it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeframe: Option<Timeframe>,
    pub favorite: bool,
}

impl IndicatorTemplate {
    /// Menu description: "Volume, RSI, MACD · BTCUSDT · 15m".
    pub fn description(&self) -> String {
        let mut parts = vec![
            self.indicators
                .iter()
                .map(|i| i.study.meta().short.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        ];
        parts.extend(self.symbol.clone());
        parts.extend(self.timeframe.map(|t| t.label()));
        parts.join(" · ")
    }
}

fn find(list: &[IndicatorTemplate], name: &str) -> Option<usize> {
    let n = name.to_lowercase();
    list.iter().position(|t| t.name.to_lowercase() == n)
}

/// Adds `t` (name validated, overwriting a template with the same name,
/// keeping its star); the list stays sorted by name. Returns the name.
pub fn save(
    list: &mut Vec<IndicatorTemplate>,
    mut t: IndicatorTemplate,
) -> Result<String, NameError> {
    t.name = validate_name(&t.name)?;
    let name = t.name.clone();
    match find(list, &name) {
        Some(i) => {
            t.favorite = list[i].favorite;
            list[i] = t;
        }
        None => list.push(t),
    }
    list.sort_by_cached_key(|t| t.name.to_lowercase());
    Ok(name)
}

pub fn delete(list: &mut Vec<IndicatorTemplate>, name: &str) -> bool {
    find(list, name).map(|i| list.remove(i)).is_some()
}

pub fn contains(list: &[IndicatorTemplate], name: &str) -> bool {
    find(list, name).is_some()
}

pub fn toggle_favorite(list: &mut [IndicatorTemplate], name: &str) {
    if let Some(i) = find(list, name) {
        list[i].favorite = !list[i].favorite;
    }
}

fn cfg(study: Study) -> IndicatorConfig {
    IndicatorConfig {
        study,
        ..IndicatorConfig::default()
    }
}

/// Templates shipped with the app (TradingView's "Technicals" section,
/// with today's catalog).
pub fn builtin() -> Vec<IndicatorTemplate> {
    let ema = |length| {
        cfg(Study::Ema {
            length,
            source: Source::Close,
        })
    };
    let volume = || cfg(tt_core::StudyKind::Volume.default_study());
    vec![
        IndicatorTemplate {
            name: "Moving averages".into(),
            indicators: vec![volume(), ema(9), ema(21), ema(50), ema(200)],
            ..Default::default()
        },
        IndicatorTemplate {
            name: "Oscillators".into(),
            indicators: [
                tt_core::StudyKind::Volume,
                tt_core::StudyKind::Rsi,
                tt_core::StudyKind::Stochastic,
                tt_core::StudyKind::Macd,
            ]
            .into_iter()
            .map(|k| cfg(k.default_study()))
            .collect(),
            ..Default::default()
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(n: &str) -> IndicatorTemplate {
        IndicatorTemplate {
            name: n.into(),
            indicators: vec![cfg(tt_core::StudyKind::Rsi.default_study())],
            ..Default::default()
        }
    }

    #[test]
    fn save_overwrites_by_name_and_keeps_the_star() {
        let mut list = Vec::new();
        assert_eq!(save(&mut list, named(" zeta ")).unwrap(), "zeta");
        save(&mut list, named("Alpha")).unwrap();
        toggle_favorite(&mut list, "ALPHA");
        let mut again = named("alpha");
        again.timeframe = Some(Timeframe::H1);
        save(&mut list, again).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "alpha");
        assert!(list[0].favorite && list[0].timeframe == Some(Timeframe::H1));
        assert_eq!(save(&mut list, named("  ")), Err(NameError::Empty));
        assert!(delete(&mut list, "Zeta") && !contains(&list, "zeta"));
    }

    #[test]
    fn templates_round_trip_through_json() {
        let mut t = named("x");
        t.symbol = Some("ETHUSDT".into());
        let back: IndicatorTemplate =
            serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        assert_eq!(back, t);
        assert!(builtin().iter().all(|t| !t.indicators.is_empty()));
        assert_eq!(t.description(), "RSI · ETHUSDT");
    }
}
