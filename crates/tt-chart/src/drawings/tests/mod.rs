use super::hit::extend;
use super::props::{Extend, LineEnd, Props};
use super::stats::format_span;
use super::*;
use crate::viewport::{PlotRect, PriceScale};
use ecolor::Color32;
use emath::vec2;
use serde::{Deserialize, Serialize};

mod families;
mod interaction;
mod settings;

pub(super) const NO_MODS: Mods = Mods {
    shift: false,
    ctrl: false,
};

pub(super) fn mapper(times: &[i64]) -> Mapper<'_> {
    Mapper {
        vp: Viewport {
            bar_spacing: 10.0,
            right_offset_bars: 0.0,
            price_min: 0.0,
            price_max: 100.0,
            plot: PlotRect {
                left: 0.0,
                top: 0.0,
                right: 1000.0,
                bottom: 500.0,
            },
            bar_count: times.len(),
            scale: PriceScale::linear(),
        },
        slots: SlotTimes::new(times, 60_000),
    }
}

pub(super) fn series(n: usize) -> CandleSeries {
    let mut s = CandleSeries::new();
    for i in 0..n {
        s.upsert(tt_core::Candle {
            open_time_ms: i as i64 * 60_000,
            open: 50.0,
            high: 60.0,
            low: 40.0,
            close: 55.0,
            volume: 1.0,
        });
    }
    s
}

#[test]
fn anchors_round_trip_and_extend() {
    let times: Vec<i64> = (0..100).map(|i| i * 60_000).collect();
    let m = mapper(&times);
    let p = m.screen(Anchor {
        time_ms: 50 * 60_000,
        price: 25.0,
    });
    let a = m.anchor(p);
    assert_eq!(a.time_ms, 50 * 60_000);
    assert!((a.price - 25.0).abs() < 1e-3);
    // Past the last bar: extrapolated by the interval.
    let far = m.anchor(pos2(m.vp.plot.right + 0.0, 0.0));
    assert!(far.time_ms >= 99 * 60_000);

    let r = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0));
    let (a, b) = extend(pos2(10.0, 10.0), pos2(20.0, 20.0), r, true, true);
    assert!(a.distance(pos2(0.0, 0.0)) < 1e-3 && b.distance(pos2(100.0, 100.0)) < 1e-3);
    let (a, b) = extend(pos2(10.0, 10.0), pos2(20.0, 20.0), r, false, true);
    assert_eq!(a, pos2(10.0, 10.0));
    assert!(b.distance(pos2(100.0, 100.0)) < 1e-3);
}

#[test]
fn place_select_drag_and_delete() {
    let times: Vec<i64> = (0..100).map(|i| i * 60_000).collect();
    let m = mapper(&times);
    let s = series(100);
    let pane = Rect::from_min_max(pos2(0.0, 0.0), pos2(1000.0, 500.0));
    let mut ds = Drawings {
        tool: Some(DrawingTool::TrendLine),
        ..Drawings::empty()
    };
    // Click, move, click.
    assert_eq!(
        ds.press(pos2(105.0, 100.0), NO_MODS, false, &m, &s),
        DrawingPress::Handled
    );
    assert!(!ds.release(pos2(105.0, 100.0), NO_MODS));
    assert_eq!(
        ds.motion(pos2(305.0, 300.0), NO_MODS, &m, &s),
        Repaint::Overlay
    );
    assert_eq!(
        ds.press(pos2(305.0, 300.0), NO_MODS, false, &m, &s),
        DrawingPress::Placed
    );
    assert_eq!(ds.items.len(), 1);
    assert_eq!(ds.tool, None, "tool released without keep drawing");
    let id = ds.items[0].id;
    assert_eq!(ds.selection(), &[id]);
    ds.layout(&m, pane, pane);
    assert!(ds.take_dirty());

    // Drag-to-place a rectangle in one gesture.
    ds.tool = Some(DrawingTool::Rectangle);
    ds.press(pos2(505.0, 100.0), NO_MODS, false, &m, &s);
    ds.motion(pos2(605.0, 200.0), NO_MODS, &m, &s);
    assert!(ds.release(pos2(605.0, 200.0), NO_MODS));
    assert_eq!(ds.items.len(), 2);
    ds.layout(&m, pane, pane);

    // Grab the trend line's body and move it right by 10 bars, up 10.
    let mid = pos2(205.0, 200.0);
    assert_eq!(ds.hit(mid), Some((id, DrawingPart::Body)));
    assert_eq!(ds.press(mid, NO_MODS, false, &m, &s), DrawingPress::Handled);
    assert_eq!(
        ds.motion(mid + vec2(100.0, -50.0), NO_MODS, &m, &s),
        Repaint::All
    );
    assert!(ds.release(mid + vec2(100.0, -50.0), NO_MODS));
    let d = &ds.items[0];
    assert_eq!(d.points[0].time_ms, 20 * 60_000);
    assert!((d.points[0].price - 90.0).abs() < 1e-3, "{:?}", d.points);

    // Magnet snaps to the bar's high.
    ds.magnet = MagnetMode::Weak;
    let a = m.snapped(pos2(505.0, m.vp.y_of(61.0)), MagnetMode::Weak, &s);
    assert_eq!(a.price, 60.0);

    // Empty space deselects; Delete removes the selection.
    assert_eq!(
        ds.press(pos2(900.0, 480.0), NO_MODS, false, &m, &s),
        DrawingPress::Handled
    );
    assert_eq!(ds.selection(), &[] as &[u64]);
    assert_eq!(
        ds.press(pos2(900.0, 480.0), NO_MODS, false, &m, &s),
        DrawingPress::Ignored
    );
    ds.select_only(id);
    assert!(ds.delete_selected());
    assert_eq!(ds.items.len(), 1);

    // One-point tools place with a single click; Escape cancels a tool.
    ds.tool = Some(DrawingTool::HorizontalLine);
    assert_eq!(
        ds.press(pos2(10.0, 250.0), NO_MODS, false, &m, &s),
        DrawingPress::Placed
    );
    ds.tool = Some(DrawingTool::Ray);
    assert!(ds.cancel());
    assert_eq!(ds.tool, None);
}

#[test]
fn three_point_open_ended_and_freehand_placement() {
    let times: Vec<i64> = (0..100).map(|i| i * 60_000).collect();
    let m = mapper(&times);
    let s = series(100);
    let mut ds = Drawings::empty();

    // Three clicks (the second point may also be dragged out).
    ds.tool = Some(DrawingTool::ParallelChannel);
    ds.press(pos2(105.0, 100.0), NO_MODS, false, &m, &s);
    ds.motion(pos2(205.0, 150.0), NO_MODS, &m, &s);
    assert!(!ds.release(pos2(205.0, 150.0), NO_MODS), "dragged out p2");
    assert!(ds.placing());
    ds.motion(pos2(205.0, 250.0), NO_MODS, &m, &s);
    assert_eq!(
        ds.press(pos2(205.0, 250.0), NO_MODS, false, &m, &s),
        DrawingPress::Placed
    );
    assert_eq!(ds.items.last().unwrap().points.len(), 3);

    // Open-ended: click, click, double-click finishes.
    ds.tool = Some(DrawingTool::Path);
    for x in [105.0, 205.0] {
        ds.press(pos2(x, 100.0), NO_MODS, false, &m, &s);
        ds.release(pos2(x, 100.0), NO_MODS);
        ds.motion(pos2(x + 50.0, 120.0), NO_MODS, &m, &s);
    }
    ds.press(pos2(305.0, 200.0), NO_MODS, false, &m, &s);
    assert_eq!(
        ds.press(pos2(305.0, 200.0), NO_MODS, true, &m, &s),
        DrawingPress::Placed
    );
    let path = ds.items.last().unwrap();
    assert_eq!(path.tool, DrawingTool::Path);
    assert_eq!(path.points.len(), 3);

    // Enter finishes; Escape finishes too once two points are fixed, and
    // drops a path with only one.
    ds.tool = Some(DrawingTool::Polyline);
    ds.press(pos2(105.0, 100.0), NO_MODS, false, &m, &s);
    assert!(!ds.finish_placing(), "one point is not a polyline");
    assert!(!ds.placing());
    ds.tool = Some(DrawingTool::Polyline);
    ds.press(pos2(105.0, 100.0), NO_MODS, false, &m, &s);
    ds.press(pos2(205.0, 100.0), NO_MODS, false, &m, &s);
    let n = ds.items.len();
    assert!(ds.cancel());
    assert_eq!(ds.items.len(), n + 1);
    assert_eq!(ds.items.last().unwrap().points.len(), 2);

    // Freehand: press, drag, release.
    ds.tool = Some(DrawingTool::Brush);
    ds.press(pos2(100.0, 100.0), NO_MODS, false, &m, &s);
    for i in 1..=20 {
        ds.motion(pos2(100.0 + i as f32 * 3.0, 100.0), NO_MODS, &m, &s);
    }
    assert!(ds.release(pos2(160.0, 100.0), NO_MODS));
    assert_eq!(ds.items.last().unwrap().points.len(), 21);
    // A click without dragging draws nothing.
    let n = ds.items.len();
    ds.tool = Some(DrawingTool::Brush);
    ds.press(pos2(100.0, 100.0), NO_MODS, false, &m, &s);
    assert!(!ds.release(pos2(100.0, 100.0), NO_MODS));
    assert_eq!(ds.items.len(), n);
}

#[test]
fn spans_and_serde() {
    assert_eq!(format_span(90 * 60_000), "1h 30m");
    assert_eq!(format_span(-(2 * 1440 + 180) * 60_000), "-2d 3h");
    let d = Drawing::new(
        DrawingTool::FibRetracement,
        vec![
            Anchor {
                time_ms: 1,
                price: 2.0
            };
            2
        ],
    );
    let json = serde_json::to_string(&d).unwrap();
    let back: Drawing = serde_json::from_str(&json).unwrap();
    assert_eq!(back.points, d.points);
    assert_eq!(back.tool, d.tool);
    assert_eq!(back.props, d.props);
}

#[test]
fn only_current_tools_are_available() {
    let available: Vec<_> = DrawingTool::available().collect();
    assert_eq!(available.len(), 17);
    for t in DrawingTool::LINES
        .iter()
        .chain(&DrawingTool::FIB)
        .chain(&DrawingTool::MEASURERS)
    {
        assert!(t.is_available(), "{t:?}");
    }
    assert!(DrawingTool::Rectangle.is_available());
    assert!(!DrawingTool::Ellipse.is_available());
    // Every tool has defaults whose family round-trips.
    for t in DrawingTool::ALL {
        let p = Props::defaults(t);
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(Props::overlay(t, &v).unwrap(), p, "{t:?}");
    }
}

#[test]
fn rgba_hex() {
    let c = Rgba::from_hex("#2962FF").unwrap();
    assert_eq!(c, Rgba::new(0x29, 0x62, 0xff, 255));
    assert_eq!(c.to_hex(), "#2962ff");
    assert_eq!(Rgba::from_hex("#2962ff33").unwrap().a(), 0x33);
    assert_eq!(Rgba::from_hex("#12345"), None);
    assert_eq!(Rgba::from_hex("#zz62ff"), None);
    assert_eq!(c.with_opacity(0.2).a(), 51);
    let json = serde_json::to_string(&c.with_opacity(0.2)).unwrap();
    assert_eq!(json, "\"#2962ff33\"");
    // ecolor's legacy arrays are premultiplied.
    let back: Rgba = serde_json::from_str("[1, 2, 3, 255]").unwrap();
    assert_eq!(back, Rgba::new(1, 2, 3, 255));
    // Low opacity keeps its exact color (premultiplied would lose it).
    let faint = Rgba::new(0x29, 0x62, 0xff, 5);
    let back: Rgba = serde_json::from_str(&serde_json::to_string(&faint).unwrap()).unwrap();
    assert_eq!(back, faint);
}

const FIXTURE: &str = include_str!("fixtures/drawings_v1.json");

#[test]
fn v1_fixture_migrates() {
    let map: DrawingMap = serde_json::from_str(FIXTURE).unwrap();
    let btc = &map["BTCUSDT"];
    assert!(btc.unknown.is_empty());
    assert_eq!(btc.items.len(), 12);
    assert_eq!(map["ETHUSDT"].items.len(), 1);

    let trend = &btc.items[0];
    assert_eq!(trend.tool, DrawingTool::TrendLine);
    assert_eq!(trend.color(), Color32::from_rgb(242, 54, 69));
    assert_eq!(trend.width(), 3.0);
    assert_eq!(trend.style(), LineStyle::Dashed);
    assert!(trend.locked);
    assert_eq!(trend.points[0].price, 35000.5);

    // Ray keeps its tool's extension; the old file had no style or locked.
    let Props::Line(ray) = &btc.items[1].props else {
        panic!("line props");
    };
    assert_eq!(
        ray.extend,
        Extend {
            left: false,
            right: true
        }
    );
    assert_eq!(btc.items[2].style(), LineStyle::Solid);
    assert!(!btc.items[2].locked);

    // Fib: the legacy color was the trend line's; width and style the
    // levels'.
    let fib = &btc.items[7];
    let Props::Fib(fp) = &fib.props else {
        panic!("fib props");
    };
    assert_eq!(fp.trend_line.color, Rgba::new(255, 152, 0, 255));
    assert_eq!(fp.trend_line.style, LineStyle::Dashed);
    assert_eq!(fp.levels_line.width, 2);
    assert_eq!(fp.levels_line.style, LineStyle::Dashed);
    assert_eq!(fp.levels.iter().filter(|l| l.on).count(), FIB_LEVELS.len());

    // Rectangle: the background follows the (blue) legacy color at 20 %.
    let Props::Shape(rect) = &btc.items[8].props else {
        panic!("shape props");
    };
    assert_eq!(rect.border.color, Rgba::new(41, 98, 255, 255));
    assert_eq!(rect.background.color, Rgba::new(41, 98, 255, 51));
    assert_eq!(btc.items[9].tool, DrawingTool::PriceRange);
}

/// Today's (pre-props) drawing, as the previous build reads it.
#[derive(Debug, Deserialize, Serialize)]
struct OldDrawing {
    tool: OldTool,
    points: Vec<Anchor>,
    color: Color32,
    width: f32,
    #[serde(default)]
    style: LineStyle,
    #[serde(default)]
    locked: bool,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
enum OldTool {
    TrendLine,
    Ray,
    ExtendedLine,
    HorizontalLine,
    HorizontalRay,
    VerticalLine,
    CrossLine,
    FibRetracement,
    Rectangle,
    PriceRange,
    DateRange,
    DateAndPriceRange,
}

#[test]
fn saved_files_stay_readable_by_older_builds() {
    let map: DrawingMap = serde_json::from_str(FIXTURE).unwrap();
    let json = serde_json::to_value(&map).unwrap();
    // The previous build reads what we write.
    let old: std::collections::BTreeMap<String, Vec<OldDrawing>> =
        serde_json::from_value(json.clone()).unwrap();
    let trend = &old["BTCUSDT"][0];
    assert_eq!(trend.tool, OldTool::TrendLine);
    assert_eq!(trend.color, Color32::from_rgb(242, 54, 69));
    assert_eq!(trend.width, 3.0);
    assert_eq!(trend.style, LineStyle::Dashed);
    assert!(trend.locked);
    // Our own write reads back identically (props included).
    let back: DrawingMap = serde_json::from_value(json.clone()).unwrap();
    for (k, list) in &map {
        let (a, b) = (&list.items, &back[k].items);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b) {
            assert_eq!((x.tool, &x.points, &x.props), (y.tool, &y.points, &y.props));
        }
    }
    assert_eq!(json["BTCUSDT"][0]["props"]["kind"], "Line");
    // An old build writes back without `props`: we migrate again.
    let old_json = serde_json::to_value(&old).unwrap();
    let again: DrawingMap = serde_json::from_value(old_json).unwrap();
    assert_eq!(
        again["BTCUSDT"].items[0].color(),
        Color32::from_rgb(242, 54, 69)
    );
}

#[test]
fn saved_props_overlay_the_tools_defaults() {
    // A Ray whose saved props only set the line color (as if written by a
    // build with fewer fields) plus a key from a newer build.
    let json = r##"{
        "tool": "Ray",
        "points": [{"time_ms": 0, "price": 1.0}, {"time_ms": 60000, "price": 2.0}],
        "props": {"kind": "Line", "line": {"color": "#FF0000FF"}, "from_the_future": 1},
        "color": [0, 0, 255, 255],
        "width": 1.0
    }"##;
    let d: Drawing = serde_json::from_str(json).unwrap();
    let Props::Line(p) = &d.props else {
        panic!("line props");
    };
    // `props` wins over the legacy fields; missing fields are the Ray's.
    assert_eq!(p.line.color, Rgba::new(255, 0, 0, 255));
    assert_eq!(p.line.width, 2);
    assert!(p.extend.right && !p.extend.left);
    assert_eq!(p.ends.right, LineEnd::Normal);
}

#[test]
fn unreadable_drawings_are_skipped_and_kept() {
    let json = r##"[
        {"tool": "TrendLine", "points": [{"time_ms": 0, "price": 1.0}, {"time_ms": 60000, "price": 2.0}],
         "color": [41, 98, 255, 255], "width": 2.0},
        {"tool": "Pitchfork", "points": [], "color": [41, 98, 255, 255], "width": 2.0},
        {"tool": "TrendLine", "color": [41, 98, 255, 255], "width": 2.0},
        {"tool": "TrendLine", "points": [{"time_ms": 0, "price": 1.0}], "width": 2.0},
        {"tool": "Ray", "points": [{"time_ms": 0, "price": 1.0}, {"time_ms": 60000, "price": 2.0}],
         "props": {"kind": "Fib"}},
        {"tool": "Ray", "points": [{"time_ms": 0, "price": 1.0}, {"time_ms": 60000, "price": 2.0}],
         "props": {"kind": "Line", "ends": {"right": "Circle"}}},
        "garbage",
        {"tool": "HorizontalLine", "points": [{"time_ms": 0, "price": 5.0}],
         "color": [8, 153, 129, 255], "width": 1.0}
    ]"##;
    let list: DrawingList = serde_json::from_str(json).unwrap();
    assert_eq!(list.items.len(), 2);
    assert_eq!(list.items[1].tool, DrawingTool::HorizontalLine);
    assert_eq!(list.unknown.len(), 6);
    assert_eq!(list.unknown[0]["tool"], "Pitchfork");
    assert!(!list.is_empty());

    // Saving writes the unreadable ones back unchanged.
    let out = serde_json::to_value(&list).unwrap();
    let out = out.as_array().unwrap();
    assert_eq!(out.len(), 8);
    let raw: Vec<serde_json::Value> = serde_json::from_str(json).unwrap();
    for u in &raw[1..7] {
        assert!(out.contains(u), "{u} kept");
    }

    // A map value that is not a list doesn't fail the map; neither does
    // a `drawings` value that is not a map.
    #[derive(Deserialize)]
    struct Settings {
        #[serde(default, deserialize_with = "deserialize_drawing_map")]
        drawings: DrawingMap,
        other: u32,
    }
    let s: Settings =
        serde_json::from_str(r#"{"drawings": {"A": 5, "B": []}, "other": 1}"#).unwrap();
    assert_eq!(s.other, 1);
    assert_eq!(s.drawings["A"].unknown.len(), 1);
    assert!(s.drawings["B"].is_empty());
    let s: Settings = serde_json::from_str(r#"{"drawings": [1, 2], "other": 2}"#).unwrap();
    assert!(s.drawings.is_empty());
    let s: Settings = serde_json::from_str(r#"{"other": 3}"#).unwrap();
    assert!(s.drawings.is_empty());
}

#[test]
fn toolbar_edits_reach_the_saved_legacy_fields() {
    let mut d = Drawing::new(
        DrawingTool::Rectangle,
        vec![
            Anchor {
                time_ms: 0,
                price: 1.0
            };
            2
        ],
    );
    d.set_color(Color32::from_rgb(8, 153, 129));
    d.set_width(3.0);
    d.set_style(LineStyle::Dotted);
    let Props::Shape(p) = &d.props else {
        panic!("shape props");
    };
    // The background has its own color now: the border change leaves it.
    assert_eq!(p.background.color, Rgba::new(0x9c, 0x27, 0xb0, 51));
    assert_eq!(p.border.color, Rgba::new(8, 153, 129, 255));
    let v = serde_json::to_value(&d).unwrap();
    assert_eq!(v["color"], serde_json::json!([8, 153, 129, 255]));
    assert_eq!(v["width"], 3.0);
    assert_eq!(v["style"], "Dotted");
    assert!(v.get("text").is_none() && v.get("hidden").is_none());
}
