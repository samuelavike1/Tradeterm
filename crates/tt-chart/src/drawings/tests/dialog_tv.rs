//! The TradingView-measured settings dialog (horizontal ray reference):
//! bar numbers, Ticks / Ranges visibility, renaming, defaults and the text
//! gap on lines.

use super::super::form::{self, DialogLayout};
use super::super::props::{HAlign, Props, VAlign};
use super::*;
use crate::time_axis::SlotTimes;
use tt_core::Timeframe;

fn at(price: f64) -> Anchor {
    Anchor { time_ms: 0, price }
}

#[test]
fn bar_numbers_map_to_anchor_times_through_the_slots() {
    // Five 1m bars with a gap after the third.
    let times = [0, 60_000, 120_000, 600_000, 660_000];
    let slots = SlotTimes::new(&times, 60_000);
    // Bar times map to their index and back.
    for (i, t) in times.iter().enumerate() {
        assert_eq!(form::bar_of_time(&slots, *t), Some(i as i64));
        assert_eq!(form::time_of_bar(&slots, i as i64), Some(*t));
    }
    // A time between bars rounds to the nearer slot.
    assert_eq!(form::bar_of_time(&slots, 130_000), Some(2));
    assert_eq!(form::bar_of_time(&slots, 500_000), Some(3));
    // Beyond the data: extrapolated by the interval, both ways.
    assert_eq!(form::time_of_bar(&slots, 7), Some(660_000 + 3 * 60_000));
    assert_eq!(form::bar_of_time(&slots, 660_000 + 3 * 60_000), Some(7));
    assert_eq!(form::time_of_bar(&slots, -2), Some(-120_000));
    assert_eq!(form::bar_of_time(&slots, -120_000), Some(-2));
    // No data: no bar numbers.
    let empty = SlotTimes::new(&[], 60_000);
    assert_eq!(form::bar_of_time(&empty, 0), None);
    assert_eq!(form::time_of_bar(&empty, 0), None);
    // Editing the bar moves the anchor time; the price stays.
    let mut pts = vec![Anchor {
        time_ms: times[1],
        price: 5.0,
    }];
    let t = form::time_of_bar(&slots, 4).unwrap();
    assert!(form::set_point(&mut pts, 0, Some(t), None));
    assert_eq!((pts[0].time_ms, pts[0].price), (660_000, 5.0));
    assert_eq!(form::point_label_price_bar(0), "#1 (price, bar)");
}

#[test]
fn visibility_ticks_and_ranges_load_from_old_files_and_round_trip() {
    // A file from before Ticks / Ranges: both on, other rows as saved.
    let old = r#"{"tool": "HorizontalRay", "points": [{"time_ms": 0, "price": 5.0}],
        "visibility": {"seconds": {"on": false, "min": 1, "max": 59}}}"#;
    let mut d: Drawing = serde_json::from_str(old).unwrap();
    assert!(d.visibility.ticks && d.visibility.ranges);
    assert!(!d.visibility.seconds.on);
    assert_eq!(d.visibility.minutes, Visibility::ALL.minutes);
    // Off survives a save / load.
    d.visibility.ticks = false;
    d.visibility.ranges = false;
    let json = serde_json::to_value(&d).unwrap();
    assert_eq!(json["visibility"]["ticks"], false);
    assert_eq!(json["visibility"]["ranges"], false);
    let back: Drawing = serde_json::from_value(json).unwrap();
    assert!(!back.visibility.ticks && !back.visibility.ranges);
    // All on is the default and is not written.
    let all = Drawing::new(DrawingTool::HorizontalRay, vec![at(1.0)]);
    let json = serde_json::to_value(&all).unwrap();
    assert!(json.get("visibility").is_none());
    // Presets: ticks only with "and below", ranges only with "all".
    let tf = Timeframe::M15;
    let below = Visibility::preset(VisibilityPreset::CurrentAndBelow, tf);
    assert!(below.ticks && !below.ranges);
    let above = Visibility::preset(VisibilityPreset::CurrentAndAbove, tf);
    assert!(!above.ticks && !above.ranges);
    let all = Visibility::preset(VisibilityPreset::All, tf);
    assert!(all.ticks && all.ranges && all.is_all());
}

#[test]
fn a_renamed_drawing_keeps_its_name() {
    let mut d = Drawing::new(DrawingTool::HorizontalRay, vec![at(1.0)]);
    assert_eq!(d.title(), "Horizontal ray");
    // Unnamed: no key in the file.
    let json = serde_json::to_value(&d).unwrap();
    assert!(json.get("name").is_none());
    d.name = "Weekly high".into();
    assert_eq!(d.title(), "Weekly high");
    let json = serde_json::to_string(&d).unwrap();
    let back: Drawing = serde_json::from_str(&json).unwrap();
    assert_eq!(back.name, "Weekly high");
    assert_eq!(back.title(), "Weekly high");
    // Inside a symbol's list too (how the settings file stores them).
    let list = DrawingList::new(vec![d.clone()]);
    let back = DrawingList::from_value(serde_json::to_value(&list).unwrap());
    assert_eq!(back.items[0].name, "Weekly high");
    // Blank names fall back to the tool's label.
    d.name = "  ".into();
    assert_eq!(d.title(), "Horizontal ray");
    // A style template never carries the name.
    let mut other = Drawing::new(DrawingTool::HorizontalRay, vec![at(2.0)]);
    other.name = "Mine".into();
    assert!(other.apply_style(&d.props));
    assert_eq!(other.name, "Mine");
}

#[test]
fn horizontal_rays_use_the_comfortable_dialog_and_tv_defaults() {
    assert_eq!(
        form::dialog_layout(DrawingTool::HorizontalRay),
        DialogLayout::Comfortable
    );
    // Every available tool uses it; the others keep the generic form.
    for t in DrawingTool::ALL {
        let want = if t.is_available() {
            DialogLayout::Comfortable
        } else {
            DialogLayout::Generic
        };
        assert_eq!(form::dialog_layout(t), want, "{t:?}");
    }
    let Props::HLine(p) = Props::defaults(DrawingTool::HorizontalRay) else {
        panic!("horizontal ray props");
    };
    assert_eq!(p.line.width, 1);
    assert_eq!((p.text.h, p.text.v), (HAlign::Center, VAlign::Middle));
    assert_eq!(p.text_style.size, 12);
}

#[test]
fn text_on_a_line_breaks_it_around_the_text() {
    use super::super::lines::gap_span;
    // 40 px text centered on a 200 px line, 6 px each side.
    assert_eq!(gap_span(100.0, 26.0, 200.0), (74.0, 126.0));
    // Clamped at the ends (text near the start or longer than the line).
    assert_eq!(gap_span(10.0, 26.0, 200.0), (0.0, 36.0));
    assert_eq!(gap_span(50.0, 80.0, 100.0), (0.0, 100.0));
}

#[test]
fn coordinates_rows_follow_the_placement() {
    use DrawingTool::*;
    for t in DrawingTool::available() {
        let n = t.placement().min_points();
        let rows = form::coordinate_rows(t, n);
        // A channel shows two points and "Price offset"; others all.
        assert_eq!(rows, if t == ParallelChannel { 2 } else { n }, "{t:?}");
        for k in 0..rows {
            let want = match t {
                HorizontalLine => (false, true),
                VerticalLine => (true, false),
                _ => (true, true),
            };
            assert_eq!(form::coordinate_parts(t, k), want, "{t:?} #{k}");
        }
    }
    // Price offset: the third point against line 0 at its bar.
    let mut pts = vec![at(10.0), at(20.0), at(18.0)];
    let bars = [0.0, 10.0, 5.0];
    assert_eq!(form::channel_offset(&pts, bars), Some(3.0));
    assert!(form::set_channel_offset(&mut pts, bars, -2.0));
    assert_eq!(pts[2].price, 13.0);
    assert_eq!(form::channel_offset(&pts, bars), Some(-2.0));
    assert!(!form::set_channel_offset(&mut pts, bars, f64::NAN));
}

#[test]
fn floating_toolbar_buttons_follow_tradingview() {
    use super::super::props::Caps;
    use DrawingTool::*;
    let caps = |t| form::toolbar_caps(t, &Props::defaults(t));
    for t in [TrendLine, Ray, InfoLine, ExtendedLine, Arrow, VerticalLine] {
        assert_eq!(caps(t), Caps::LINE | Caps::TEXT, "{t:?}");
    }
    assert_eq!(caps(TrendAngle), Caps::LINE);
    assert_eq!(caps(CrossLine), Caps::LINE);
    assert_eq!(caps(Rectangle), Caps::LINE | Caps::FILL | Caps::TEXT);
    // No style button on channels, Fibs and ranges.
    for t in [ParallelChannel, PriceRange, DateRange, DateAndPriceRange] {
        assert_eq!(
            caps(t),
            Caps::LINE_COLOR | Caps::LINE_WIDTH | Caps::FILL | Caps::TEXT,
            "{t:?}"
        );
    }
    for t in [FibRetracement, FibExtension] {
        assert_eq!(caps(t), Caps::LINE_COLOR | Caps::LINE_WIDTH, "{t:?}");
    }
}

#[test]
fn check_menus_summarize_like_tradingview() {
    let summary = |t: DrawingTool, label: &str| {
        let p = Props::defaults(t);
        let f = form::style_fields(t, &p);
        let f = f.iter().find(|f| f.label == label).unwrap();
        f.menu_summary(&form::props_value(&p))
    };
    assert_eq!(
        summary(DrawingTool::ExtendedLine, "Extend"),
        "Extend left line, extend right line"
    );
    assert_eq!(summary(DrawingTool::TrendLine, "Extend"), "Don't extend");
    assert_eq!(summary(DrawingTool::TrendLine, "Stats"), "Hidden");
    assert_eq!(
        summary(DrawingTool::DateRange, "Stats"),
        "Bars range, date/time range"
    );
}

#[test]
fn channel_levels_edit_their_line_and_the_main_lines_the_channel() {
    use super::super::props::Stroke;
    let mut p = Props::defaults(DrawingTool::ParallelChannel);
    let coeffs: Vec<f64> = form::levels(&p).iter().map(|l| l.coeff).collect();
    // TradingView lists them in coefficient order.
    assert_eq!(coeffs, [-0.25, 0.0, 0.25, 0.5, 0.75, 1.0, 1.25]);
    assert!(form::level::is_main(&p, 1) && form::level::is_main(&p, 5));
    assert!(!form::level::is_main(&p, 3));
    let red = Stroke::solid(super::super::model::RED, 3);
    // A main level sets the channel's line (both lines follow).
    assert!(form::level::set_stroke(&mut p, 1, red));
    assert_eq!(form::level::stroke(&p, 5), Some(red));
    let Props::Channel(c) = &p else {
        panic!("channel props")
    };
    assert_eq!(c.line, red);
    // Another level gets its own.
    let blue = Stroke::solid(super::super::model::BLUE, 1);
    assert!(form::level::set_stroke(&mut p, 3, blue));
    assert_eq!(form::level::stroke(&p, 3), Some(blue));
    assert_eq!(form::level::stroke(&p, 1), Some(red));
}
