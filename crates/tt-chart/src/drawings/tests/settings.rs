//! Slice A: templates, visibility, the dialog's form model and preview /
//! commit.

use super::super::form::{self, FieldKind};
use super::super::model::BLUE;
use super::super::props::{LineEnd, Props};
use super::super::templates::{DrawingTemplates, tool_name};
use super::*;
use tt_core::{TimeUnit, Timeframe};

fn at(price: f64) -> Anchor {
    Anchor {
        time_ms: 0,
        price,
    }
}

fn trend() -> Drawing {
    Drawing::new(DrawingTool::TrendLine, vec![at(1.0), at(2.0)])
}

#[test]
fn templates_round_trip() {
    let mut t = DrawingTemplates::default();
    let mut p = Props::defaults(DrawingTool::TrendLine);
    p.set_line_color(Rgba::new(1, 2, 3, 128));
    p.set_line_width(4);
    assert!(!t.save_named(DrawingTool::TrendLine, "Support", &p));
    assert!(t.save_named(DrawingTool::TrendLine, "Support", &p), "replaces");
    t.set_default(DrawingTool::TrendLine, &p);
    // Another family's style is never stored for a tool.
    t.set_default(DrawingTool::Rectangle, &p);
    assert!(!t.has_default(DrawingTool::Rectangle));

    let json = serde_json::to_string(&t).unwrap();
    let back: DrawingTemplates = serde_json::from_str(&json).unwrap();
    assert_eq!(back, t);
    assert_eq!(back.named(DrawingTool::TrendLine).len(), 1);
    assert_eq!(back.named_props(DrawingTool::TrendLine, "Support"), Some(p.clone()));
    assert_eq!(back.new_drawing_props(DrawingTool::TrendLine), p);
    // Templates are per tool.
    assert!(back.named(DrawingTool::Ray).is_empty());
    assert_eq!(
        back.new_drawing_props(DrawingTool::Ray),
        Props::defaults(DrawingTool::Ray)
    );
    assert_eq!(back.defaults(), vec![(DrawingTool::TrendLine, p)]);

    let mut back = back;
    back.clear_default(DrawingTool::TrendLine);
    assert!(!back.has_default(DrawingTool::TrendLine));
    assert!(back.remove_named(DrawingTool::TrendLine, "Support"));
    assert_eq!(back.to_value(), serde_json::json!({}));
}

#[test]
fn templates_load_tolerantly_and_keep_what_they_cannot_read() {
    let json = serde_json::json!({
        "TrendLine": {
            "default": { "kind": "Line", "line": { "width": 3 }, "future_key": 1 },
            "named": [
                { "name": "Thin", "props": { "line": { "width": 1 } } },
                { "props": { "line": { "width": 2 } } },
                "garbage"
            ]
        },
        "ToolFromTheFuture": { "default": { "kind": "Wave" }, "named": [] },
        "Rectangle": 5
    });
    let t = DrawingTemplates::from_value(json.clone());
    // The default overlays the tool's defaults (unknown keys ignored).
    let Props::Line(lp) = t.new_drawing_props(DrawingTool::TrendLine) else {
        panic!("line props");
    };
    assert_eq!(lp.line.width, 3);
    assert_eq!(lp.line.color, Rgba::opaque(BLUE));
    assert_eq!(t.named(DrawingTool::TrendLine).len(), 1);
    assert!(t.named_props(DrawingTool::TrendLine, "Thin").is_some());
    // Unknown tools and bad entries are written back unchanged.
    let out = t.to_value();
    assert_eq!(out["ToolFromTheFuture"], json["ToolFromTheFuture"]);
    assert_eq!(out["Rectangle"], json["Rectangle"]);
    assert_eq!(out["TrendLine"]["named"].as_array().unwrap().len(), 3);
    // Not a map at all: empty, no error.
    assert_eq!(
        DrawingTemplates::from_value(serde_json::json!([1, 2])),
        DrawingTemplates::default()
    );
    // A default of another family is ignored when applied.
    let wrong = DrawingTemplates::from_value(serde_json::json!({
        "TrendLine": { "default": { "kind": "Fib" } }
    }));
    assert_eq!(wrong.default_props(DrawingTool::TrendLine), None);
    assert_eq!(tool_name(DrawingTool::FibRetracement), "FibRetracement");
}

#[test]
fn applying_a_template_keeps_points_text_and_state() {
    let mut d = trend();
    d.text = "Breakout".into();
    d.locked = true;
    d.hidden = true;
    d.visibility.minutes.max = 30;
    let before = d.clone();
    let mut style = Props::defaults(DrawingTool::TrendLine);
    style.set_line_color(Rgba::new(9, 9, 9, 255));
    assert!(d.apply_style(&style));
    assert_eq!(d.props, style);
    assert_eq!(d.points, before.points);
    assert_eq!(d.text, before.text);
    assert_eq!((d.locked, d.hidden), (true, true));
    assert_eq!(d.visibility, before.visibility);
    // Another family's style does not apply.
    assert!(!d.apply_style(&Props::defaults(DrawingTool::Rectangle)));
    // The saved template has no coordinates or text.
    let v = super::super::templates::template_value(&style);
    for key in ["points", "text", "locked", "hidden", "visibility"] {
        assert!(v.get(key).is_none(), "{key} in a template");
    }
}

#[test]
fn new_drawings_start_from_the_tool_default() {
    let times: Vec<i64> = (0..100).map(|i| i * 60_000).collect();
    let m = mapper(&times);
    let s = series(100);
    let mut style = Props::defaults(DrawingTool::HorizontalLine);
    style.set_line_width(4);
    let mut ds = Drawings {
        tool: Some(DrawingTool::HorizontalLine),
        ..Drawings::empty()
    };
    ds.set_tool_defaults(vec![(DrawingTool::HorizontalLine, style.clone())]);
    assert_eq!(
        ds.press(pos2(100.0, 100.0), NO_MODS, false, &m, &s),
        DrawingPress::Placed
    );
    assert_eq!(ds.items[0].props, style);
}

#[test]
fn visibility_rows_and_presets() {
    let all = Visibility::ALL;
    for tf in [
        Timeframe::S1,
        Timeframe::M1,
        Timeframe::H4,
        Timeframe::D1,
        Timeframe::W1,
        Timeframe::MN1,
        Timeframe::new(TimeUnit::Minute, 90),
    ] {
        assert!(all.shows(tf), "{tf:?}");
    }
    // 90 and 120 minutes test the hours row.
    assert_eq!(
        Visibility::row_of(Timeframe::new(TimeUnit::Minute, 120)),
        (TimeUnit::Hour, 2)
    );
    let mut v = Visibility::ALL;
    v.minutes = Span {
        on: true,
        min: 5,
        max: 15,
    };
    v.hours.on = false;
    assert!(!v.shows(Timeframe::M1));
    assert!(v.shows(Timeframe::M5) && v.shows(Timeframe::M15));
    assert!(!v.shows(Timeframe::M30));
    assert!(!v.shows(Timeframe::H1));
    assert!(v.shows(Timeframe::D1));

    let tf = Timeframe::M15;
    let above = Visibility::preset(VisibilityPreset::CurrentAndAbove, tf);
    assert!(!above.shows(Timeframe::S1) && !above.shows(Timeframe::M5));
    assert!(above.shows(tf) && above.shows(Timeframe::M30) && above.shows(Timeframe::D1));
    let below = Visibility::preset(VisibilityPreset::CurrentAndBelow, tf);
    assert!(below.shows(Timeframe::S1) && below.shows(Timeframe::M5) && below.shows(tf));
    assert!(!below.shows(Timeframe::M30) && !below.shows(Timeframe::H1));
    let only = Visibility::preset(VisibilityPreset::CurrentOnly, tf);
    assert!(only.shows(tf));
    assert!(!only.shows(Timeframe::M5) && !only.shows(Timeframe::H1));
    assert!(Visibility::preset(VisibilityPreset::All, tf).is_all());
}

#[test]
fn drawings_outside_their_intervals_are_not_drawn_or_hit() {
    let times: Vec<i64> = (0..100).map(|i| i * 60_000).collect();
    let m = mapper(&times);
    let pane = Rect::from_min_max(pos2(0.0, 0.0), pos2(1000.0, 500.0));
    let mut d = Drawing::new(DrawingTool::HorizontalLine, vec![at(50.0)]);
    d.visibility = Visibility::preset(VisibilityPreset::CurrentOnly, Timeframe::H1);
    let id = d.id;
    let mut ds = Drawings::empty();
    ds.set_items(vec![d]);
    ds.layout(&m, pane, pane);
    let p = pos2(500.0, m.vp.y_of(50.0));
    // Unknown interval: shown.
    assert!(ds.hit(p).is_some());
    ds.set_timeframe(Some(Timeframe::H1));
    assert_eq!(ds.hit(p).map(|h| h.0), Some(id));
    ds.select_only(id);
    ds.set_timeframe(Some(Timeframe::M5));
    assert!(ds.hit(p).is_none());
    assert!(ds.selection().is_empty(), "hidden drawings leave the selection");
    assert!(!ds.is_shown(&ds.items[0]));
}

#[test]
fn form_fields_edit_props_by_path() {
    let mut p = Props::defaults(DrawingTool::TrendLine);
    let fields = form::style_fields(DrawingTool::TrendLine, &p);
    // Every field's path exists in the props.
    let v = form::props_value(&p);
    for f in &fields {
        if f.kind != FieldKind::Section {
            assert!(form::get(&v, &f.path).is_some(), "{}", f.path);
        }
        if let Some(c) = &f.check {
            assert!(form::get(&v, c).is_some(), "{c}");
        }
    }
    assert!(form::set(&mut p, "/ends/right", form::choice_value("Arrow")));
    assert!(form::set(&mut p, "/stats/angle", serde_json::Value::Bool(true)));
    assert!(form::set_color(&mut p, "/line/color", Rgba::new(1, 2, 3, 4)));
    // Unreadable values leave the props unchanged.
    assert!(!form::set(&mut p, "/ends/left", form::choice_value("Spiral")));
    assert!(!form::set(&mut p, "/no/such/path", serde_json::Value::Bool(true)));
    let Props::Line(lp) = &p else {
        panic!("line props");
    };
    assert_eq!(lp.ends.right, LineEnd::Arrow);
    assert!(lp.stats.angle);
    assert_eq!(lp.line.color, Rgba::new(1, 2, 3, 4));
    assert_eq!(form::get_choice(&form::props_value(&p), "/ends/right"), "Arrow");
}

#[test]
fn every_family_form_points_at_real_fields() {
    for tool in DrawingTool::ALL {
        let p = Props::defaults(tool);
        let v = form::props_value(&p);
        for f in form::style_fields(tool, &p)
            .into_iter()
            .chain(form::text_fields(tool, &p))
        {
            if f.kind != FieldKind::Section {
                assert!(form::get(&v, &f.path).is_some(), "{tool:?} {}", f.path);
            }
            if let Some(c) = &f.check {
                assert!(form::get(&v, c).is_some(), "{tool:?} {c}");
            }
        }
    }
    // Text tabs: lines, shapes, channels, ranges; not Fib or cross lines.
    let has = |t| !form::text_fields(t, &Props::defaults(t)).is_empty();
    assert!(has(DrawingTool::TrendLine) && has(DrawingTool::Rectangle));
    assert!(has(DrawingTool::ParallelChannel) && has(DrawingTool::PriceRange));
    assert!(!has(DrawingTool::FibRetracement) && !has(DrawingTool::CrossLine));
}

#[test]
fn level_helpers() {
    let mut fib = Props::defaults(DrawingTool::FibRetracement);
    assert_eq!(form::levels(&fib).len(), 24);
    assert!(form::level::set_on(&mut fib, 11, true));
    assert!(form::level::set_coeff(&mut fib, 11, 1.5));
    assert!(!form::level::set_coeff(&mut fib, 11, f64::NAN));
    assert!(form::level::set_all_colors(&mut fib, Rgba::new(5, 5, 5, 255)));
    assert!(form::levels(&fib).iter().all(|l| l.color == Rgba::new(5, 5, 5, 255)));
    assert_eq!(form::levels(&fib)[11].coeff, 1.5);

    // A channel level drawn in the channel's line gets its own stroke.
    let mut ch = Props::defaults(DrawingTool::ParallelChannel);
    let red = Rgba::new(255, 0, 0, 255);
    assert!(form::level::set_color(&mut ch, 0, red));
    assert_eq!(form::level::color(&ch, 0), red);
    let Props::Channel(cp) = &ch else {
        panic!("channel props");
    };
    assert_eq!(cp.levels[0].stroke.map(|s| s.width), Some(cp.line.width));
}

#[test]
fn coordinates_time_format() {
    let t = 1_763_640_000_000; // 2025-11-20 12:00 UTC
    assert_eq!(form::format_time(t), "2025-11-20 12:00");
    assert_eq!(form::parse_time("2025-11-20 12:00"), Some(t));
    assert_eq!(form::parse_time(" 2025-11-20T12:00:00 "), Some(t));
    assert_eq!(form::parse_time("2025-11-20"), Some(t - 12 * 3_600_000));
    assert_eq!(form::parse_time("2025-02-31"), None);
    assert_eq!(form::parse_time("2025-11-20 25:00"), None);
    assert_eq!(form::parse_time("yesterday"), None);
    let mut pts = vec![at(1.0)];
    assert!(form::set_point(&mut pts, 0, Some(t), Some(2.0)));
    assert_eq!(pts[0], Anchor { time_ms: t, price: 2.0 });
    assert!(!form::set_point(&mut pts, 0, None, Some(f64::NAN)));
    assert_eq!(form::coordinate_parts(DrawingTool::HorizontalLine, 0), (false, true));
}

#[test]
fn preview_then_commit_is_one_undo_step_and_cancel_restores() {
    let mut ds = Drawings::empty();
    let (a, b) = (trend(), trend());
    let ids = [a.id, b.id];
    ds.set_items(vec![a, b]);
    let before = ds.items.clone();
    ds.preview(&ids, |d| d.set_width(4.0));
    assert!(ds.items.iter().all(|d| d.width() == 4.0));
    assert!(!ds.can_undo(), "the preview records nothing");
    // Cancel.
    ds.restore(&before);
    assert_eq!(ds.items, before);
    // Ok.
    ds.preview(&ids, |d| d.set_width(3.0));
    assert!(ds.commit_preview(&before));
    assert!(ds.items.iter().all(|d| d.width() == 3.0));
    assert!(ds.undo());
    assert_eq!(ds.items, before, "one undo step reverts both");
    // Nothing changed: no step.
    assert!(!ds.commit_preview(&ds.items.clone()));
}

#[test]
fn toolbar_style_edits_merge_per_control() {
    let mut ds = Drawings::empty();
    let d = trend();
    let id = d.id;
    ds.set_items(vec![d]);
    ds.select_only(id);
    for w in [2u8, 3, 4] {
        ds.edit_selected_style(1, |d| d.props.set_line_color(Rgba::new(w, 0, 0, 255)));
    }
    ds.edit_selected_style(2, |d| d.set_width(1.0));
    assert!(ds.undo());
    assert_eq!(ds.items[0].width(), 2.0);
    assert!(ds.undo(), "the three color edits are one step");
    assert_eq!(ds.items[0].props.line_color(), Rgba::opaque(BLUE));
    assert!(!ds.undo());
}

#[test]
fn rectangle_background_is_its_own_color() {
    let mut d = Drawing::new(DrawingTool::Rectangle, vec![at(1.0), at(2.0)]);
    d.props.set_line_color(Rgba::new(1, 1, 1, 255));
    d.props.set_fill_color(Rgba::new(2, 2, 2, 100));
    let Props::Shape(sp) = &d.props else {
        panic!("shape props");
    };
    assert_eq!(sp.border.color, Rgba::new(1, 1, 1, 255));
    assert_eq!(sp.background.color, Rgba::new(2, 2, 2, 100));
    assert!(d.props.capabilities().contains(Caps::FILL));
    assert!(Props::defaults(DrawingTool::PriceRange)
        .capabilities()
        .contains(Caps::FILL));
    assert!(!Props::defaults(DrawingTool::FibRetracement)
        .capabilities()
        .contains(Caps::FILL));
}
