//! Line tools, channel, Fib and measurers: geometry, stats text, rendering
//! smoke tests and serde of their props.

use super::super::channel::{self, Geo};
use super::super::fib::{self, Levels};
use super::super::lines;
use super::super::measure::{self, measurer_rect, volume_between};
use super::super::props::{FibProps, RangeProps, Stats, StatsPosition};
use super::super::stats::{
    self, Axes, Buf, Lines, StatInputs, format_span, write_price, write_signed_price,
};
use super::*;
use crate::ScaleMode;
use crate::draw::Scene;
use crate::render::Canvas;
use crate::scale::{format_price, format_signed_price};
use crate::{ChartStyle, ChartType};
use emath::{Align2, Vec2};
use tt_theme::Theme;

fn anchor(time_ms: i64, price: f64) -> Anchor {
    Anchor { time_ms, price }
}

#[test]
fn line_props_drive_the_extension() {
    let pane = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0));
    let (a, b) = (pos2(10.0, 10.0), pos2(20.0, 20.0));
    let lp = |tool| match Props::defaults(tool) {
        Props::Line(p) => p,
        _ => panic!("line props"),
    };
    assert_eq!(
        lines::segment(&lp(DrawingTool::TrendLine), a, b, pane),
        (a, b)
    );
    let (s, e) = lines::segment(&lp(DrawingTool::Ray), a, b, pane);
    assert_eq!(s, a);
    assert!(e.distance(pos2(100.0, 100.0)) < 1e-3);
    let (s, e) = lines::segment(&lp(DrawingTool::ExtendedLine), a, b, pane);
    assert!(s.distance(pos2(0.0, 0.0)) < 1e-3 && e.distance(pos2(100.0, 100.0)) < 1e-3);
    // Extend left only.
    let mut p = lp(DrawingTool::TrendLine);
    p.extend = Extend {
        left: true,
        right: false,
    };
    let (s, e) = lines::segment(&p, a, b, pane);
    assert!(s.distance(pos2(0.0, 0.0)) < 1e-3);
    assert_eq!(e, b);
    // Arrow and info line defaults.
    assert_eq!(lp(DrawingTool::Arrow).ends.right, LineEnd::Arrow);
    let info = lp(DrawingTool::InfoLine);
    assert!(info.stats.always && info.stats.angle);
    assert_eq!(info.stats.position, StatsPosition::Center);
}

#[test]
fn angles_are_counter_clockwise_on_screen() {
    let o = pos2(100.0, 100.0);
    assert!((stats::angle_deg(o, pos2(200.0, 100.0))).abs() < 1e-4);
    assert!((stats::angle_deg(o, pos2(200.0, 0.0)) - 45.0).abs() < 1e-4);
    assert!((stats::angle_deg(o, pos2(100.0, 200.0)) + 90.0).abs() < 1e-4);
    assert!((stats::angle_deg(o, pos2(0.0, 100.0)) - 180.0).abs() < 1e-4);
    assert_eq!(stats::angle_deg(o, o), 0.0);
}

#[test]
fn channel_offset_handles_and_drags() {
    let g = Geo::of(&[pos2(0.0, 100.0), pos2(100.0, 50.0), pos2(50.0, 125.0)]).unwrap();
    // Line 0 at x = 50 is y = 75, so line 1 is 50 below.
    assert!((g.offset - 50.0).abs() < 1e-4);
    let (a, b) = g.level(0.5);
    assert_eq!((a, b), (pos2(0.0, 125.0), pos2(100.0, 75.0)));
    let h = g.handles();
    assert_eq!(h.len(), channel::HANDLES);
    assert_eq!(h[2], pos2(0.0, 150.0));
    assert_eq!(h[5], pos2(50.0, 125.0));

    let times: Vec<i64> = (0..100).map(|i| i * 60_000).collect();
    let m = mapper(&times);
    let s = series(100);
    let pane = Rect::from_min_max(pos2(0.0, 0.0), pos2(1000.0, 500.0));
    let mut ds = Drawings {
        tool: Some(DrawingTool::ParallelChannel),
        ..Drawings::empty()
    };
    // Three clicks: line 0 from bar 10 at 80 to bar 30 at 60, then 20 lower.
    let (p0, p1, p2) = (
        m.screen(anchor(10 * 60_000, 80.0)),
        m.screen(anchor(30 * 60_000, 60.0)),
        m.screen(anchor(20 * 60_000, 50.0)),
    );
    ds.press(p0, NO_MODS, false, &m, &s);
    ds.release(p0, NO_MODS);
    ds.motion(p1, NO_MODS, &m, &s);
    assert_eq!(ds.press(p1, NO_MODS, false, &m, &s), DrawingPress::Handled);
    ds.motion(p2, NO_MODS, &m, &s);
    assert_eq!(ds.press(p2, NO_MODS, false, &m, &s), DrawingPress::Placed);
    ds.layout(&m, pane, pane);
    let id = ds.items[0].id;
    // Line 1 is 20 below line 0 in price; its middle is at bar 20, 50.
    let mid1 = m.screen(anchor(20 * 60_000, 50.0));
    assert_eq!(ds.hit(mid1), Some((id, DrawingPart::Handle(5))));
    // Inside the band (background on) hits the body.
    let inside = m.screen(anchor(20 * 60_000, 60.0));
    assert_eq!(ds.hit(inside), Some((id, DrawingPart::Body)));
    // Drag line 1's middle 10 lower: only the offset changes.
    ds.press(mid1, NO_MODS, false, &m, &s);
    let to = m.screen(anchor(20 * 60_000, 40.0));
    ds.motion(to, NO_MODS, &m, &s);
    ds.release(to, NO_MODS);
    let d = &ds.items[0];
    assert!((d.points[0].price - 80.0).abs() < 1e-3);
    assert!((d.points[1].price - 60.0).abs() < 1e-3);
    // Point 2 sits on line 1 at line 0's start: 80 - 30.
    assert_eq!(d.points[2].time_ms, d.points[0].time_ms);
    assert!((d.points[2].price - 50.0).abs() < 1e-3, "{:?}", d.points);
    // Drag line 0's middle up 5: line 1 stays where it was.
    ds.layout(&m, pane, pane);
    let mid0 = m.screen(anchor(20 * 60_000, 70.0));
    assert_eq!(ds.hit(mid0), Some((id, DrawingPart::Handle(4))));
    ds.press(mid0, NO_MODS, false, &m, &s);
    let to = m.screen(anchor(20 * 60_000, 75.0));
    ds.motion(to, NO_MODS, &m, &s);
    ds.release(to, NO_MODS);
    let d = &ds.items[0];
    assert!((d.points[0].price - 85.0).abs() < 1e-3);
    assert!((d.points[2].price - 50.0).abs() < 1e-3, "{:?}", d.points);
}

#[test]
fn fib_level_prices() {
    let lin = PriceScale::linear();
    let log = PriceScale::new(ScaleMode::Logarithmic, 1.0);
    let mut fp = FibProps::default();
    assert!(!fp.log_scale, "log-based levels are off by default");
    // Retracement from 100 (point 1) to 200 (point 2): 0 at 200, 1 at 100.
    let lv = Levels::new(DrawingTool::FibRetracement, &fp, &[100.0, 200.0], lin);
    assert!((lv.price(0.0, lin) - 200.0).abs() < 1e-9);
    assert!((lv.price(1.0, lin) - 100.0).abs() < 1e-9);
    assert!((lv.price(0.618, lin) - 138.2).abs() < 1e-9);
    // Off: linear in price even on a log chart.
    let lv = Levels::new(DrawingTool::FibRetracement, &fp, &[100.0, 200.0], log);
    assert!((lv.price(0.5, log) - 150.0).abs() < 1e-9);
    // On (and a log chart): the geometric middle.
    fp.log_scale = true;
    let lv = Levels::new(DrawingTool::FibRetracement, &fp, &[100.0, 200.0], log);
    assert!((lv.price(0.5, log) - 200f64.sqrt() * 10.0).abs() < 1e-6);
    // On but a linear chart: linear.
    let lv = Levels::new(DrawingTool::FibRetracement, &fp, &[100.0, 200.0], lin);
    assert!((lv.price(0.5, lin) - 150.0).abs() < 1e-9);
    fp.log_scale = false;
    // Reverse: 0 at point 1.
    fp.reverse = true;
    let lv = Levels::new(DrawingTool::FibRetracement, &fp, &[100.0, 200.0], lin);
    assert!((lv.price(0.0, lin) - 100.0).abs() < 1e-9);
    assert!((lv.price(0.236, lin) - 123.6).abs() < 1e-9);
    fp.reverse = false;
    // Extension: p3 + (p2 - p1) × coeff.
    let lv = Levels::new(DrawingTool::FibExtension, &fp, &[100.0, 200.0, 150.0], lin);
    assert!((lv.price(0.0, lin) - 150.0).abs() < 1e-9);
    assert!((lv.price(1.0, lin) - 250.0).abs() < 1e-9);
    assert!((lv.price(1.618, lin) - 311.8).abs() < 1e-9);
    fp.reverse = true;
    let lv = Levels::new(DrawingTool::FibExtension, &fp, &[100.0, 200.0, 150.0], lin);
    assert!((lv.price(0.0, lin) - 250.0).abs() < 1e-9);
    assert!((lv.price(1.0, lin) - 150.0).abs() < 1e-9);
}

#[test]
fn fib_labels_and_levels() {
    let mut fp = FibProps::default();
    assert_eq!(fp.levels.len(), 24);
    assert_eq!(fp.levels.iter().filter(|l| l.on).count(), 11);
    let label = |fp: &FibProps| {
        let mut b = Buf::new();
        fib::write_label(&mut b, fp, 0.618, 1234.5, 2);
        b.as_str().to_owned()
    };
    assert_eq!(label(&fp), "0.618 (1,234.50)");
    fp.percents = true;
    assert_eq!(label(&fp), "61.8% (1,234.50)");
    fp.show_prices = false;
    assert_eq!(label(&fp), "61.8%");
    fp.show_prices = true;
    fp.show_levels = false;
    assert_eq!(label(&fp), "1,234.50");
    // The extension's levels start at the end of the move / correction.
    let pts = [pos2(10.0, 0.0), pos2(50.0, 0.0), pos2(80.0, 0.0)];
    assert_eq!(fib::x_range(DrawingTool::FibExtension, &pts), (50.0, 80.0));
    assert_eq!(
        fib::x_range(DrawingTool::FibRetracement, &pts),
        (10.0, 50.0)
    );
}

#[test]
fn stack_formatting_matches_the_allocating_one() {
    for (v, d) in [
        (0.0, 2),
        (-0.001, 2),
        (1234567.891, 2),
        (-1234.5, 1),
        (999.0, 0),
        (0.000123, 6),
    ] {
        let mut b = Buf::new();
        write_price(&mut b, v, d).unwrap();
        assert_eq!(b.as_str(), format_price(v, d), "{v} {d}");
        let mut b = Buf::new();
        write_signed_price(&mut b, v, d).unwrap();
        assert_eq!(b.as_str(), format_signed_price(v, d), "{v} {d}");
    }
    assert_eq!(format_span(90 * 60_000), "1h 30m");
    assert_eq!(format_span(-(2 * 1440 + 180) * 60_000), "-2d 3h");
    assert_eq!(format_span(45_000), "45s");
    assert_eq!(format_span(125_000), "2m 5s");
    assert_eq!(format_span(17 * 86_400_000), "2w 3d");
    assert_eq!(format_span(14 * 86_400_000), "2w");
    // Too long for the buffer: the write fails, the text stays valid.
    let mut b = Buf::new();
    assert!(std::fmt::Write::write_str(&mut b, &"x".repeat(200)).is_err());
    assert!(b.is_empty());
}

fn lines_of(st: &Stats, axes: Axes, inp: &StatInputs) -> Vec<String> {
    let mut l = Lines::default();
    stats::stats_lines(st, axes, inp, &mut l);
    l.iter().map(str::to_owned).collect()
}

#[test]
fn stats_text() {
    let inp = StatInputs {
        from: anchor(0, 100.0),
        to: anchor(90 * 60_000, 110.5),
        bars: 18,
        angle_deg: 30.0,
        precision: 2,
        tick: 0.01,
        volume: Some(12_345.0),
    };
    let all = Stats {
        price_range: true,
        percent: true,
        pips: true,
        bars: true,
        date_time: true,
        distance: true,
        angle: true,
        volume: true,
        ..Stats::default()
    };
    assert_eq!(
        lines_of(&all, Axes::BOTH, &inp),
        [
            "+10.50 (+10.50%) +1,050",
            "18 bars, 1h 30m",
            "Vol 12.35K",
            "30.00°"
        ]
    );
    let price_only = Axes {
        price: true,
        time: false,
    };
    assert_eq!(
        lines_of(&all, price_only, &inp),
        ["+10.50 (+10.50%) +1,050"]
    );
    let pct = Stats {
        percent: true,
        date_time: true,
        ..Stats::default()
    };
    assert_eq!(lines_of(&pct, Axes::BOTH, &inp), ["+10.50%", "1h 30m"]);
    assert!(lines_of(&Stats::default(), Axes::BOTH, &inp).is_empty());
    assert_eq!(stats::tick_or_precision(None, 3), 0.001);
    assert_eq!(stats::tick_or_precision(Some(0.5), 3), 0.5);
}

#[test]
fn measurer_boxes_and_volume() {
    let pane = Rect::from_min_max(pos2(0.0, 0.0), pos2(1000.0, 500.0));
    let pts = [pos2(100.0, 100.0), pos2(200.0, 300.0)];
    let mut d = Drawing::new(DrawingTool::PriceRange, vec![anchor(0, 1.0); 2]);
    let inner = Rect::from_two_pos(pts[0], pts[1]);
    assert_eq!(measurer_rect(&d, &pts, pane), inner);
    if let Props::Range(p) = &mut d.props {
        p.extend.right = true;
    }
    assert_eq!(measurer_rect(&d, &pts, pane).right(), 1000.0);
    assert_eq!(measurer_rect(&d, &pts, pane).left(), 100.0);
    d.tool = DrawingTool::DateRange;
    let r = measurer_rect(&d, &pts, pane);
    assert_eq!((r.top(), r.bottom(), r.right()), (100.0, 500.0, 200.0));
    assert!(!measure::axes(DrawingTool::DateRange).price);
    let v = [1.0, 2.0, 3.0, 4.0];
    assert_eq!(volume_between(&v, 2.2, 0.6), 1.0 + 2.0 + 3.0 - 1.0);
    assert_eq!(volume_between(&v, -5.0, 10.0), 10.0);
    assert_eq!(volume_between(&v, -5.0, -1.0), 0.0);
    assert_eq!(volume_between(&[], 0.0, 3.0), 0.0);
}

#[test]
fn new_props_round_trip() {
    let mut fp = FibProps {
        reverse: true,
        log_scale: true,
        background_opacity: 0.35,
        ..FibProps::default()
    };
    fp.levels[12].on = true;
    fp.levels[0].coeff = -0.27;
    let p = Props::Fib(fp);
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(Props::overlay(DrawingTool::FibExtension, &v).unwrap(), p);

    let mut rp = RangeProps {
        down_color: Rgba::new(1, 2, 3, 4),
        border_on: true,
        ..RangeProps::default()
    };
    rp.stats.volume = true;
    let p = Props::Range(rp);
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(Props::overlay(DrawingTool::PriceRange, &v).unwrap(), p);
    // Saved before the down color existed: the default (red).
    let mut old = v.clone();
    old.as_object_mut().unwrap().remove("down_color");
    let Props::Range(back) = Props::overlay(DrawingTool::PriceRange, &old).unwrap() else {
        panic!("range");
    };
    assert_eq!(back.down_color, Rgba::opaque(super::super::model::RED));

    for t in [
        DrawingTool::InfoLine,
        DrawingTool::TrendAngle,
        DrawingTool::Arrow,
        DrawingTool::ParallelChannel,
    ] {
        let mut d = Drawing::new(t, vec![anchor(0, 1.0); t.placement().min_points()]);
        d.set_color(Color32::from_rgb(8, 153, 129));
        let back: Drawing = serde_json::from_value(serde_json::to_value(&d).unwrap()).unwrap();
        assert_eq!(back.props, d.props, "{t:?}");
        assert_eq!(back.points, d.points);
    }
}

/// Records drawn text; geometry is ignored.
#[derive(Default)]
struct Recorder {
    texts: Vec<String>,
}

impl Canvas for Recorder {
    fn pixels_per_point(&self) -> f32 {
        1.0
    }
    fn set_clip(&mut self, _: Rect) {}
    fn fill_rect(&mut self, _: Rect, _: Color32) {}
    fn fill_rounded_rect(&mut self, _: Rect, _: f32, _: Color32) {}
    fn stroke_rect(&mut self, _: Rect, _: f32, _: Color32) {}
    fn line(&mut self, a: Pos2, b: Pos2, _: f32, _: Color32) {
        assert!(a.is_finite() && b.is_finite());
    }
    fn dashed_line(&mut self, a: Pos2, b: Pos2, _: f32, _: Color32, _: f32, _: f32) {
        assert!(a.is_finite() && b.is_finite());
    }
    fn polyline(&mut self, _: &[Pos2], _: f32, _: Color32) {}
    fn fill_area(&mut self, _: &[Pos2], _: f32, _: f32, _: Color32, _: Color32) {}
    fn fill_between(&mut self, _: &[Pos2], _: &[Pos2], _: Color32) {}
    fn fill_circle(&mut self, _: Pos2, _: f32, _: Color32) {}
    fn text_styled(
        &mut self,
        pos: Pos2,
        a: Align2,
        t: &str,
        size: f32,
        _: Color32,
        b: bool,
    ) -> Rect {
        self.texts.push(t.to_owned());
        a.anchor_size(pos, self.text_size_styled(t, size, b))
    }
    fn text_size_styled(&mut self, t: &str, size: f32, _: bool) -> Vec2 {
        vec2(t.chars().count() as f32 * size * 0.6, size)
    }
}

#[test]
fn every_line_fib_and_range_tool_renders() {
    let times: Vec<i64> = (0..100).map(|i| i * 60_000).collect();
    let m = mapper(&times);
    let s = series(100);
    let pane = Rect::from_min_max(pos2(0.0, 0.0), pos2(1000.0, 500.0));
    let mut ds = Drawings::empty();
    let tools = DrawingTool::LINES
        .iter()
        .chain(&DrawingTool::FIB)
        .chain(&DrawingTool::MEASURERS);
    for &t in tools {
        let pts = [
            anchor(600_000, 40.0),
            anchor(1_800_000, 60.0),
            anchor(2_400_000, 50.0),
        ];
        let mut d = Drawing::new(t, pts[..t.placement().min_points()].to_vec());
        d.text = "note".into();
        if let Props::Line(p) = &mut d.props {
            p.text.on = true;
            p.price_labels = true;
            p.middle_point = true;
            p.stats.always = true;
            p.stats.bars = true;
        }
        if let Props::Range(p) = &mut d.props {
            p.stats.volume = true;
        }
        ds.items.push(d);
    }
    ds.layout(&m, pane, pane);
    ds.selected = Some(ds.items[0].id);
    let (theme, style) = (Theme::dark(), ChartStyle::default());
    let scene = Scene {
        vp: m.vp,
        theme: &theme,
        style: &style,
        chart_type: ChartType::Candles,
        full: Rect::from_min_max(pos2(0.0, 0.0), pos2(1080.0, 528.0)),
        plot: pane,
        volume_rect: None,
        indicators: &[],
        panes: Vec::new(),
        range: 0..s.len(),
        legend_collapsed: false,
        main_collapsed: false,
        legend_hover: None,
        price_axis: Rect::from_min_max(pos2(1000.0, 0.0), pos2(1080.0, 500.0)),
        time_axis: Rect::from_min_max(pos2(0.0, 500.0), pos2(1000.0, 528.0)),
        display: &s,
        projection_from: None,
        last_raw: s.last(),
        bars: &[],
        decimated: false,
        crosshair: None,
        precision: 2,
        tick_size: Some(0.01),
        bar_interval_ms: 60_000,
        now_ms: None,
        bar_close_ms: None,
        title: "",
        drawings: &ds,
    };
    let mut c = Recorder::default();
    draw_base(&mut c, &scene);
    draw_overlay(&mut c, &scene);
    let texts = c.texts.join("|");
    // Info line stats, Fib labels, range stats and volume, the angle.
    assert!(texts.contains("+20.00 (+50.00%) +2,000"), "{texts}");
    assert!(texts.contains("0.618 (47.64)"), "{texts}");
    assert!(texts.contains("Vol "), "{texts}");
    assert!(texts.contains("20 bars, 20m"), "{texts}");
    assert!(texts.contains('°'), "{texts}");
    // Axis tags: lines with price labels, horizontal lines, the cross line.
    assert!(axis_tags(&scene).len() >= 2 * 6);
}
