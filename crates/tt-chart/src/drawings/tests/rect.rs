//! The rectangle as on TradingView: eight handles (hit testing, resize
//! math, flips, Shift, cursors), axis bands and tags, text placement and
//! the defaults.

use super::super::props::{HAlign, VAlign};
use super::super::shapes::{self, ResizeCursor};
use super::{NO_MODS, mapper, series};
use crate::draw::contrast_text;
use crate::drawings::*;
use ecolor::Color32;
use emath::{Pos2, Rect, pos2, vec2};

const MIN: i64 = 60_000;
const SHIFT: Mods = Mods {
    shift: true,
    ctrl: false,
};

fn times() -> Vec<i64> {
    (0..100).map(|i| i * MIN).collect()
}

fn pane() -> Rect {
    Rect::from_min_max(pos2(0.0, 0.0), pos2(1000.0, 500.0))
}

fn at(bar: i64, price: f64) -> Anchor {
    Anchor {
        time_ms: bar * MIN,
        price,
    }
}

/// A selected rectangle from bar 20 / price 80 to bar 40 / price 40.
fn rect(m: &Mapper<'_>) -> (Drawings, u64) {
    let mut ds = Drawings::empty();
    ds.items = vec![Drawing::new(
        DrawingTool::Rectangle,
        vec![at(20, 80.0), at(40, 40.0)],
    )];
    let id = ds.items[0].id;
    ds.layout(m, pane(), pane());
    ds.select_only(id);
    (ds, id)
}

fn handle(ds: &Drawings, id: u64, k: usize) -> Pos2 {
    shapes::rect_handles(ds.screen_of(id).unwrap())[k]
}

/// Drags handle `k` by `by` (points) and returns the rectangle's points.
fn drag(k: usize, by: emath::Vec2, mods: Mods) -> [Anchor; 2] {
    let t = times();
    let m = mapper(&t);
    let s = series(100);
    let (mut ds, id) = rect(&m);
    let h = handle(&ds, id, k);
    assert_eq!(ds.hit(h), Some((id, DrawingPart::Handle(k))));
    ds.press(h, mods, false, &m, &s);
    ds.motion(h + by, mods, &m, &s);
    assert!(ds.release(h + by, mods));
    [ds.items[0].points[0], ds.items[0].points[1]]
}

#[test]
fn eight_handles_corners_then_edge_midpoints() {
    let t = times();
    let m = mapper(&t);
    let (ds, id) = rect(&m);
    let pts = ds.screen_of(id).unwrap();
    let (a, b) = (pts[0], pts[1]);
    let hs = shapes::rect_handles(pts);
    let mid = a + (b - a) * 0.5;
    assert_eq!(
        hs,
        [
            a,
            b,
            pos2(a.x, b.y),
            pos2(b.x, a.y),
            pos2(a.x, mid.y),
            pos2(b.x, mid.y),
            pos2(mid.x, a.y),
            pos2(mid.x, b.y),
        ]
    );
    // Every handle hits (slightly off center too); the inside is the body.
    for (k, h) in hs.iter().enumerate() {
        assert_eq!(ds.hit(*h), Some((id, DrawingPart::Handle(k))), "{k}");
        assert_eq!(
            ds.hit(*h + vec2(3.0, -3.0)),
            Some((id, DrawingPart::Handle(k))),
            "{k} off center"
        );
    }
    assert_eq!(
        ds.hit(mid + vec2(20.0, 20.0)),
        Some((id, DrawingPart::Body))
    );
    // Corners are circles, edges squares.
    let mut square = Vec::new();
    hit::for_each_handle(DrawingTool::Rectangle, pts, |_, sq| square.push(sq));
    assert_eq!(square, [false, false, false, false, true, true, true, true]);
}

#[test]
fn edge_handles_move_one_side_corners_two() {
    let (p0, p1) = (at(20, 80.0), at(40, 40.0));
    // 10 px per bar, 5 px per price unit (0..100 over 500 px).
    // Left edge (point 0's time) right by 5 bars; prices untouched.
    let [a, b] = drag(4, vec2(50.0, 30.0), NO_MODS);
    assert_eq!((a.time_ms, b.time_ms), (25 * MIN, p1.time_ms));
    assert_eq!((a.price, b.price), (p0.price, p1.price));
    // Right edge.
    let [a, b] = drag(5, vec2(-50.0, 30.0), NO_MODS);
    assert_eq!((a.time_ms, b.time_ms), (p0.time_ms, 35 * MIN));
    assert_eq!((a.price, b.price), (p0.price, p1.price));
    // Top edge (point 0's price) down by 10.
    let [a, b] = drag(6, vec2(70.0, 50.0), NO_MODS);
    assert_eq!((a.time_ms, b.time_ms), (p0.time_ms, p1.time_ms));
    assert!(
        (a.price - 70.0).abs() < 1e-4 && b.price == p1.price,
        "{a:?}"
    );
    // Bottom edge.
    let [a, b] = drag(7, vec2(-70.0, -50.0), NO_MODS);
    assert_eq!((a.time_ms, b.time_ms), (p0.time_ms, p1.time_ms));
    assert!(
        a.price == p0.price && (b.price - 50.0).abs() < 1e-4,
        "{b:?}"
    );
    // Corner 2 (point 0's time, point 1's price): both.
    let [a, b] = drag(2, vec2(-50.0, 50.0), NO_MODS);
    assert_eq!((a.time_ms, b.time_ms), (15 * MIN, p1.time_ms));
    assert!(a.price == p0.price && (b.price - 30.0).abs() < 1e-4);
    // Corner 3 (point 1's time, point 0's price).
    let [a, b] = drag(3, vec2(50.0, -50.0), NO_MODS);
    assert_eq!((a.time_ms, b.time_ms), (p0.time_ms, 45 * MIN));
    assert!((a.price - 90.0).abs() < 1e-4 && b.price == p1.price);
}

#[test]
fn dragging_past_the_opposite_side_flips() {
    // Left edge 30 bars right: past the right edge (bar 40) to bar 50.
    let [a, b] = drag(4, vec2(300.0, 0.0), NO_MODS);
    assert_eq!((a.time_ms, b.time_ms), (50 * MIN, 40 * MIN));
    // Top edge below the bottom (price 40) to price 20.
    let [a, b] = drag(6, vec2(0.0, 300.0), NO_MODS);
    assert!((a.price - 20.0).abs() < 1e-4 && b.price == 40.0);
    // Corner 0 past both sides: the rectangle flips diagonally.
    let [a, b] = drag(0, vec2(300.0, 300.0), NO_MODS);
    assert_eq!(a.time_ms, 50 * MIN);
    assert!((a.price - 20.0).abs() < 1e-4 && b == at(40, 40.0));
    // The handles follow: the dragged corner's cursor turns with it.
    let t = times();
    let m = mapper(&t);
    let flipped = [m.screen(a), m.screen(b)];
    assert_eq!(shapes::rect_cursor(&flipped, 0), Some(ResizeCursor::Nwse));
    let one_flip = [m.screen(at(50, 80.0)), m.screen(at(40, 40.0))];
    assert_eq!(shapes::rect_cursor(&one_flip, 0), Some(ResizeCursor::Nesw));
}

#[test]
fn shift_keeps_corner_drags_square() {
    let t = times();
    let m = mapper(&t);
    for k in 0..4 {
        let [a, b] = drag(k, vec2(-40.0, 25.0), SHIFT);
        let (sa, sb) = (m.screen(a), m.screen(b));
        let (w, h) = ((sb.x - sa.x).abs(), (sb.y - sa.y).abs());
        assert!((w - h).abs() <= 10.0, "corner {k}: {w} x {h}");
    }
    // Edges ignore Shift: one side only.
    let [a, b] = drag(5, vec2(-50.0, 30.0), SHIFT);
    assert_eq!((a.price, b.price), (80.0, 40.0));
    assert_eq!(b.time_ms, 35 * MIN);
}

#[test]
fn handle_cursors_as_tradingview() {
    let t = times();
    let m = mapper(&t);
    let s = series(100);
    let (mut ds, id) = rect(&m);
    // Point 0 is top-left, point 1 bottom-right.
    let want = [
        ResizeCursor::Nwse,
        ResizeCursor::Nwse,
        ResizeCursor::Nesw,
        ResizeCursor::Nesw,
        ResizeCursor::Ew,
        ResizeCursor::Ew,
        ResizeCursor::Ns,
        ResizeCursor::Ns,
    ];
    for (k, w) in want.iter().enumerate() {
        assert_eq!(ds.resize_cursor(handle(&ds, id, k)), Some(*w), "{k}");
    }
    let body = handle(&ds, id, 0) + vec2(40.0, 40.0);
    assert_eq!(ds.resize_cursor(body), None);
    // While dragging an edge the cursor stays, wherever the pointer is.
    let h = handle(&ds, id, 6);
    ds.press(h, NO_MODS, false, &m, &s);
    ds.motion(h + vec2(30.0, 40.0), NO_MODS, &m, &s);
    assert_eq!(ds.resize_cursor(pos2(0.0, 0.0)), Some(ResizeCursor::Ns));
    ds.release(h + vec2(30.0, 40.0), NO_MODS);
    // Locked: no resize pointer.
    ds.items[0].locked = true;
    assert_eq!(ds.resize_cursor(handle(&ds, id, 0)), None);
}

#[test]
fn axis_bands_span_the_two_prices_and_times() {
    let price_axis = Rect::from_min_max(pos2(1000.0, 0.0), pos2(1080.0, 500.0));
    let time_axis = Rect::from_min_max(pos2(0.0, 500.0), pos2(1000.0, 528.0));
    let pts = [pos2(300.0, 260.0), pos2(120.0, 100.0)];
    let (p, t) = shapes::axis_bands(&pts, price_axis, time_axis, pane());
    assert_eq!(
        p,
        Some(Rect::from_min_max(pos2(1000.0, 100.0), pos2(1080.0, 260.0)))
    );
    assert_eq!(
        t,
        Some(Rect::from_min_max(pos2(120.0, 500.0), pos2(300.0, 528.0)))
    );
    // Clipped to the pane; off screen: none.
    let pts = [pos2(-50.0, -40.0), pos2(80.0, 40.0)];
    let (p, t) = shapes::axis_bands(&pts, price_axis, time_axis, pane());
    assert_eq!(p.unwrap().y_range(), emath::Rangef::new(0.0, 40.0));
    assert_eq!(t.unwrap().x_range(), emath::Rangef::new(0.0, 80.0));
    let pts = [pos2(1100.0, 600.0), pos2(1200.0, 700.0)];
    assert_eq!(
        shapes::axis_bands(&pts, price_axis, time_axis, pane()),
        (None, None)
    );
}

#[test]
fn tags_use_the_opaque_line_color_with_readable_text() {
    let mut d = Drawing::new(DrawingTool::Rectangle, vec![at(1, 1.0), at(2, 2.0)]);
    // The default border is translucent; tags and handle rings are not.
    assert!(d.props.line_color().a() < 255);
    assert_eq!(d.solid_color(), Color32::from_rgb(0x29, 0x62, 0xff));
    assert_eq!(contrast_text(d.solid_color()), Color32::WHITE);
    d.set_color(Color32::from_rgb(0xff, 0xeb, 0x3b));
    assert_ne!(
        contrast_text(d.solid_color()),
        Color32::WHITE,
        "dark on yellow"
    );
    assert_ne!(contrast_text(Color32::WHITE), Color32::WHITE);
    assert_eq!(
        contrast_text(Color32::from_rgb(0xf2, 0x36, 0x45)),
        Color32::WHITE
    );
    assert_eq!(
        contrast_text(Color32::from_rgb(0x9c, 0x27, 0xb0)),
        Color32::WHITE
    );
}

#[test]
fn text_sits_inside_on_the_middle_line_or_above_below() {
    let r = Rect::from_min_max(pos2(100.0, 100.0), pos2(300.0, 200.0));
    let size = vec2(40.0, 14.0);
    let place = |h, v| {
        let (pos, align) = shapes::text_anchor(r, h, v);
        align.anchor_size(pos, size)
    };
    // Inside (middle), right: inset from the right border, centered on the
    // middle line.
    let t = place(HAlign::Right, VAlign::Middle);
    assert_eq!(t.right(), r.right() - shapes::TEXT_PAD_X);
    assert_eq!(t.center().y, r.center().y);
    assert!(r.contains_rect(t));
    let t = place(HAlign::Left, VAlign::Middle);
    assert_eq!(t.left(), r.left() + shapes::TEXT_PAD_X);
    let t = place(HAlign::Center, VAlign::Middle);
    assert_eq!(t.center(), r.center());
    // Top / Bottom: above / below the box.
    let t = place(HAlign::Right, VAlign::Top);
    assert_eq!(t.bottom(), r.top() - shapes::TEXT_PAD_Y);
    let t = place(HAlign::Left, VAlign::Bottom);
    assert_eq!(t.top(), r.bottom() + shapes::TEXT_PAD_Y);

    // The middle line stops short of text on it.
    let t = place(HAlign::Right, VAlign::Middle);
    let [a, b] = shapes::middle_line_pieces(r.left(), r.right(), Some(t));
    assert_eq!(a, (r.left(), t.left() - shapes::MIDDLE_LINE_GAP));
    assert!(b.0 >= b.1, "nothing right of right-aligned text: {b:?}");
    let t = place(HAlign::Center, VAlign::Middle);
    let [a, b] = shapes::middle_line_pieces(r.left(), r.right(), Some(t));
    assert!(a.1 < t.left() && b.0 > t.right() && b.1 == r.right());
    let [a, _] = shapes::middle_line_pieces(r.left(), r.right(), None);
    assert_eq!(a, (r.left(), r.right()));
}

#[test]
fn defaults_match_tradingview_and_old_rectangles_keep_their_look() {
    let Props::Shape(p) = Props::defaults(DrawingTool::Rectangle) else {
        panic!("shape");
    };
    assert_eq!(p.border.width, 1);
    assert_eq!(p.border.color, Rgba::new(0x29, 0x62, 0xff, 122));
    assert_eq!(p.background.color, Rgba::new(0x29, 0x62, 0xff, 51));
    assert!(p.middle_line_on && p.middle_line.style == LineStyle::Dashed);
    assert_eq!(p.text_style.color, Rgba::rgb(255, 255, 255));
    assert_eq!((p.text.h, p.text.v), (HAlign::Right, VAlign::Middle));
    // A pre-props rectangle: its color, no middle line, colored text.
    let mut old = Props::defaults(DrawingTool::Rectangle);
    old.apply_legacy(Some(Color32::from_rgb(242, 54, 69)), Some(2.0), None);
    let Props::Shape(o) = old else {
        panic!("shape")
    };
    assert_eq!(o.border.color, Rgba::rgb(242, 54, 69));
    assert_eq!(o.border.width, 2);
    assert!(!o.middle_line_on);
    assert_eq!(o.text_style.color, Rgba::rgb(242, 54, 69));
    assert_eq!(o.background.color, Rgba::new(242, 54, 69, 51));
}
