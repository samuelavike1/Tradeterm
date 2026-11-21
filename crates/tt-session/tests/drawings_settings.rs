//! Settings files with drawings: one drawing this build can't read (e.g. a
//! tool from a newer build) must not discard the settings file.

use tt_chart::DrawingTool;
use tt_session::settings::AppSettings;

const SETTINGS: &str = r##"{
  "theme_mode": "Light",
  "tabs": [{ "symbol": "ETHUSDT" }],
  "drawings": {
    "BTCUSDT": [
      {
        "tool": "TrendLine",
        "points": [
          { "time_ms": 1700000000000, "price": 35000.0 },
          { "time_ms": 1700003600000, "price": 36000.0 }
        ],
        "color": [242, 54, 69, 255],
        "width": 3.0,
        "style": "Dashed",
        "locked": false
      },
      {
        "tool": "Pitchfork",
        "points": [{ "time_ms": 1700000000000, "price": 35000.0 }],
        "color": [41, 98, 255, 255],
        "width": 2.0,
        "props": { "kind": "Pitchfork", "levels": [1, 2, 3] }
      }
    ],
    "ETHUSDT": "not a list"
  }
}"##;

#[test]
fn a_bad_drawing_does_not_discard_the_settings() {
    let s: AppSettings = serde_json::from_str(SETTINGS).expect("settings load");
    assert_eq!(s.theme_mode, tt_theme::ThemeMode::Light);
    assert_eq!(s.tabs[0].symbol, "ETHUSDT");
    let btc = &s.drawings["BTCUSDT"];
    assert_eq!(btc.items.len(), 1);
    assert_eq!(btc.items[0].tool, DrawingTool::TrendLine);
    assert_eq!(btc.unknown.len(), 1);

    // Saving writes the unreadable drawing back unchanged.
    let out = serde_json::to_value(&s).unwrap();
    let saved = out["drawings"]["BTCUSDT"].as_array().unwrap();
    assert_eq!(saved.len(), 2);
    assert_eq!(saved[1]["tool"], "Pitchfork");
    assert_eq!(saved[1]["props"]["levels"], serde_json::json!([1, 2, 3]));
    // And it reads back the same way.
    let again: AppSettings = serde_json::from_value(out).unwrap();
    assert_eq!(again.drawings["BTCUSDT"].items.len(), 1);
    assert_eq!(again.drawings["BTCUSDT"].unknown.len(), 1);
}

#[test]
fn drawings_of_the_wrong_shape_are_dropped_not_fatal() {
    let s: AppSettings =
        serde_json::from_str(r#"{"theme_mode": "Dark", "drawings": 42}"#).expect("settings load");
    assert_eq!(s.theme_mode, tt_theme::ThemeMode::Dark);
    assert!(s.drawings.is_empty());
    let s: AppSettings = serde_json::from_str(r#"{"theme_mode": "Dark"}"#).unwrap();
    assert!(s.drawings.is_empty());
}
