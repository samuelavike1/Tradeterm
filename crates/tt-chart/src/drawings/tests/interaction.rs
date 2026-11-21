//! Slice H: selection set, rubber band, clone, Shift constraints, nudge,
//! magnet modes, undo / redo, clipboard and visual order.

use super::{NO_MODS, mapper, series};
use crate::drawings::constrain::{self, Constraint};
use crate::drawings::history::MAX_HISTORY;
use crate::drawings::*;
use emath::{Pos2, Rect, pos2, vec2};

const MIN: i64 = 60_000;
const CTRL: Mods = Mods {
    shift: false,
    ctrl: true,
};
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

fn line(t0: i64, p0: f64, t1: i64, p1: f64) -> Drawing {
    Drawing::new(
        DrawingTool::TrendLine,
        vec![
            Anchor {
                time_ms: t0 * MIN,
                price: p0,
            },
            Anchor {
                time_ms: t1 * MIN,
                price: p1,
            },
        ],
    )
}

/// Two trend lines: A from bar 10 to 30 at price 80, B from bar 50 to 70
/// at price 20.
fn two_lines(m: &Mapper<'_>) -> (Drawings, u64, u64) {
    let mut ds = Drawings::empty();
    ds.items = vec![line(10, 80.0, 30, 80.0), line(50, 20.0, 70, 20.0)];
    let (a, b) = (ds.items[0].id, ds.items[1].id);
    ds.layout(m, pane(), pane());
    (ds, a, b)
}

fn mid(ds: &Drawings, id: u64) -> Pos2 {
    let pts = ds.screen_of(id).unwrap();
    pts[0] + (pts[1] - pts[0]) * 0.5
}

#[test]
fn ctrl_click_toggles_and_click_selects_one() {
    let t = times();
    let m = mapper(&t);
    let s = series(100);
    let (mut ds, a, b) = two_lines(&m);
    let (pa, pb) = (mid(&ds, a), mid(&ds, b));

    ds.press(pa, NO_MODS, false, &m, &s);
    ds.release(pa, NO_MODS);
    assert_eq!(ds.selection(), &[a]);
    ds.press(pb, CTRL, false, &m, &s);
    assert!(!ds.release(pb, CTRL));
    assert_eq!(ds.selection(), &[a, b]);
    // Ctrl + click again removes it.
    ds.press(pb, CTRL, false, &m, &s);
    ds.release(pb, CTRL);
    assert_eq!(ds.selection(), &[a]);
    ds.press(pb, CTRL, false, &m, &s);
    ds.release(pb, CTRL);
    // A plain click on a member of a multi-selection selects only it.
    ds.press(pa, NO_MODS, false, &m, &s);
    ds.release(pa, NO_MODS);
    assert_eq!(ds.selection(), &[a]);
    // Both selected show handles: the second's handle is hit first.
    ds.toggle_selected(b);
    let hb = ds.screen_of(b).unwrap()[1];
    assert_eq!(ds.hit(hb), Some((b, DrawingPart::Handle(1))));
}

#[test]
fn rubber_band_selects_what_it_touches() {
    let t = times();
    let m = mapper(&t);
    let s = series(100);
    let (mut ds, a, b) = two_lines(&m);
    let pa = mid(&ds, a);
    // A band crossing A's middle (no endpoint inside).
    let from = pa + vec2(-10.0, -30.0);
    let to = pa + vec2(10.0, 30.0);
    assert_eq!(
        ds.press(from, CTRL, false, &m, &s),
        DrawingPress::Handled,
        "Ctrl + press on empty space opens a band"
    );
    assert!(ds.banding());
    assert_eq!(ds.motion(to, CTRL, &m, &s), Repaint::Overlay);
    assert!(!ds.release(to, CTRL));
    assert!(!ds.banding());
    assert_eq!(ds.selection(), &[a]);
    // A band over both.
    ds.press(pos2(1.0, 1.0), CTRL, false, &m, &s);
    ds.release(pos2(999.0, 499.0), CTRL);
    assert_eq!(ds.selection(), &[a, b]);
    // A tiny band (a Ctrl + click) changes nothing.
    ds.press(pos2(900.0, 10.0), CTRL, false, &m, &s);
    ds.release(pos2(901.0, 10.0), CTRL);
    assert_eq!(ds.selection(), &[a, b]);
    let _ = b;
}

#[test]
fn dragging_moves_the_whole_selection_and_delete_removes_it() {
    let t = times();
    let m = mapper(&t);
    let s = series(100);
    let (mut ds, a, b) = two_lines(&m);
    ds.select_only(a);
    ds.toggle_selected(b);
    let pa = mid(&ds, a);
    ds.press(pa, NO_MODS, false, &m, &s);
    ds.motion(pa + vec2(100.0, 0.0), NO_MODS, &m, &s);
    assert!(ds.release(pa + vec2(100.0, 0.0), NO_MODS));
    assert_eq!(ds.items[0].points[0].time_ms, 20 * MIN);
    assert_eq!(ds.items[1].points[0].time_ms, 60 * MIN);
    assert_eq!(ds.selection(), &[a, b], "the selection survives the drag");
    // One undo step moves both back.
    assert!(ds.undo());
    assert_eq!(ds.items[0].points[0].time_ms, 10 * MIN);
    assert_eq!(ds.items[1].points[0].time_ms, 50 * MIN);
    assert!(ds.redo());
    assert_eq!(ds.items[1].points[0].time_ms, 60 * MIN);

    // Locked drawings stay put.
    ds.layout(&m, pane(), pane());
    ds.items[1].locked = true;
    let pa = mid(&ds, a);
    ds.press(pa, NO_MODS, false, &m, &s);
    ds.motion(pa + vec2(-100.0, 0.0), NO_MODS, &m, &s);
    ds.release(pa + vec2(-100.0, 0.0), NO_MODS);
    assert_eq!(ds.items[0].points[0].time_ms, 10 * MIN);
    assert_eq!(ds.items[1].points[0].time_ms, 60 * MIN);

    assert!(ds.delete_selected());
    assert!(ds.items.is_empty());
    assert!(ds.selection().is_empty());
    assert!(ds.undo());
    assert_eq!(ds.items.len(), 2);
    assert_eq!((ds.items[0].id, ds.items[1].id), (a, b), "back in order");
}

#[test]
fn ctrl_drag_clones() {
    let t = times();
    let m = mapper(&t);
    let s = series(100);
    let (mut ds, a, _) = two_lines(&m);
    let pa = mid(&ds, a);
    ds.press(pa, CTRL, false, &m, &s);
    // Within the click distance nothing happens yet.
    assert_eq!(ds.motion(pa + vec2(2.0, 0.0), CTRL, &m, &s), Repaint::None);
    assert_eq!(ds.items.len(), 2);
    ds.motion(pa + vec2(100.0, 0.0), CTRL, &m, &s);
    assert!(ds.release(pa + vec2(100.0, 0.0), CTRL));
    assert_eq!(ds.items.len(), 3);
    assert_eq!(ds.items[0].points[0].time_ms, 10 * MIN, "original stays");
    let clone = &ds.items[2];
    assert_ne!(clone.id, a);
    assert_eq!(clone.points[0].time_ms, 20 * MIN);
    assert_eq!(ds.selection(), &[clone.id]);
    assert!(ds.undo());
    assert_eq!(ds.items.len(), 2);
}

#[test]
fn shift_constrains_angles_squares_and_axes() {
    // 45° steps, projected so the pointer stays close.
    let o = pos2(100.0, 100.0);
    assert_eq!(
        constrain::apply(Constraint::Angle, o, pos2(200.0, 108.0)),
        pos2(200.0, 100.0)
    );
    let d = constrain::apply(Constraint::Angle, o, pos2(200.0, 190.0));
    assert!((d.x - d.y).abs() < 1e-3, "{d:?}");
    assert_eq!(
        constrain::apply(Constraint::Angle, o, pos2(103.0, 40.0)),
        pos2(100.0, 40.0)
    );
    assert_eq!(
        constrain::apply(Constraint::Square, o, pos2(150.0, 80.0)),
        pos2(150.0, 50.0)
    );
    assert_eq!(
        constrain::constraint(DrawingTool::ParallelChannel, 2, 3),
        None
    );

    let t = times();
    let m = mapper(&t);
    let s = series(100);
    let mut ds = Drawings {
        tool: Some(DrawingTool::TrendLine),
        ..Drawings::empty()
    };
    // Placing a trend line with Shift: horizontal.
    let p0 = m.screen(Anchor {
        time_ms: 10 * MIN,
        price: 50.0,
    });
    ds.press(p0, NO_MODS, false, &m, &s);
    ds.release(p0, NO_MODS);
    ds.motion(p0 + vec2(200.0, 12.0), SHIFT, &m, &s);
    ds.press(p0 + vec2(200.0, 12.0), SHIFT, false, &m, &s);
    let d = ds.items.last().unwrap();
    assert!((d.points[1].price - 50.0).abs() < 1e-9, "{:?}", d.points);
    assert_eq!(d.points[1].time_ms, 30 * MIN);

    // A rectangle with Shift: a square on screen.
    ds.tool = Some(DrawingTool::Rectangle);
    let r0 = p0 + vec2(400.0, 50.0);
    ds.press(r0, NO_MODS, false, &m, &s);
    ds.motion(r0 + vec2(100.0, 40.0), SHIFT, &m, &s);
    assert!(ds.release(r0 + vec2(100.0, 40.0), SHIFT));
    let r = ds.items.last().unwrap();
    let (a, b) = (m.screen(r.points[0]), m.screen(r.points[1]));
    assert!(((b.x - a.x) - (b.y - a.y)).abs() < 1.0, "{a:?} {b:?}");

    // Shift + drag body: horizontal only.
    ds.layout(&m, pane(), pane());
    let id = ds.items[0].id;
    let pm = mid(&ds, id);
    ds.press(pm, SHIFT, false, &m, &s);
    assert_eq!(ds.motion(pm + vec2(1.0, 1.0), SHIFT, &m, &s), Repaint::None);
    ds.motion(pm + vec2(100.0, 30.0), SHIFT, &m, &s);
    ds.release(pm + vec2(100.0, 30.0), SHIFT);
    let d = &ds.items[0];
    assert_eq!(d.points[0].time_ms, 20 * MIN);
    assert!((d.points[0].price - 50.0).abs() < 1e-9);

    // Shift + drag a handle: 45° relative to the other end.
    ds.layout(&m, pane(), pane());
    let h = ds.screen_of(id).unwrap()[1];
    ds.select_only(id);
    ds.press(h, NO_MODS, false, &m, &s);
    ds.motion(h + vec2(10.0, -190.0), SHIFT, &m, &s);
    assert!(ds.release(h + vec2(10.0, -190.0), SHIFT));
    let d = &ds.items[0];
    let (a, b) = (m.screen(d.points[0]), m.screen(d.points[1]));
    assert!(((b.x - a.x) + (b.y - a.y)).abs() < 6.0, "45°: {a:?} {b:?}");
}

#[test]
fn arrow_nudges_coalesce_into_one_undo_step() {
    let t = times();
    let m = mapper(&t);
    let (mut ds, a, b) = two_lines(&m);
    assert!(!ds.nudge(1, 0.0, &m), "nothing selected");
    ds.select_only(a);
    assert!(ds.nudge(1, 0.0, &m));
    assert!(ds.nudge(1, 0.5, &m));
    assert!(ds.nudge(0, 0.5, &m));
    let d = &ds.items[0];
    assert_eq!(d.points[0].time_ms, 12 * MIN);
    assert!((d.points[0].price - 81.0).abs() < 1e-9);
    assert_eq!(ds.items[1].points[0].time_ms, 50 * MIN, "B not selected");
    assert_eq!(ds.history.len(), 1);
    assert!(ds.undo());
    assert_eq!(ds.items[0].points[0].time_ms, 10 * MIN);
    assert!(!ds.history.can_undo());
    // A nudge of another selection is its own step.
    ds.select_only(a);
    ds.nudge(1, 0.0, &m);
    ds.select_only(b);
    ds.nudge(1, 0.0, &m);
    assert_eq!(ds.history.len(), 2);
}

#[test]
fn magnet_weak_strong_and_ctrl_inversion() {
    let t = times();
    let m = mapper(&t);
    let s = series(100);
    // Bars span 40-60; y of 90 is far (150 pt) from the high.
    let x = m
        .screen(Anchor {
            time_ms: 10 * MIN,
            price: 0.0,
        })
        .x;
    let far = pos2(x, m.vp.y_of(90.0));
    let near = pos2(x, m.vp.y_of(61.0));
    assert_eq!(m.snapped(near, MagnetMode::Weak, &s).price, 60.0);
    assert!((m.snapped(far, MagnetMode::Weak, &s).price - 90.0).abs() < 1e-3);
    assert_eq!(m.snapped(far, MagnetMode::Strong, &s).price, 60.0);
    assert_eq!(MagnetMode::Off.with_ctrl(true), MagnetMode::Strong);
    assert_eq!(MagnetMode::Weak.with_ctrl(true), MagnetMode::Off);
    assert_eq!(MagnetMode::Strong.with_ctrl(false), MagnetMode::Strong);

    // Ctrl while placing with the magnet off: strong.
    let mut ds = Drawings {
        tool: Some(DrawingTool::HorizontalLine),
        ..Drawings::empty()
    };
    ds.press(far, CTRL, false, &m, &s);
    assert_eq!(ds.items[0].points[0].price, 60.0);
}

#[test]
fn undo_redo_covers_create_edit_remove_all_and_is_bounded() {
    let t = times();
    let m = mapper(&t);
    let s = series(100);
    let mut ds = Drawings {
        tool: Some(DrawingTool::HorizontalLine),
        keep_drawing: true,
        ..Drawings::empty()
    };
    ds.press(pos2(100.0, 100.0), NO_MODS, false, &m, &s);
    ds.press(pos2(100.0, 200.0), NO_MODS, false, &m, &s);
    assert_eq!(ds.items.len(), 2);
    let id = ds.items[1].id;
    ds.select_only(id);
    assert!(ds.edit_selected(|d| d.set_width(3.0)));
    assert!(ds.remove_all());
    assert!(ds.items.is_empty());
    assert!(ds.can_undo());

    assert!(ds.undo()); // remove all
    assert_eq!(ds.items.len(), 2);
    assert_eq!(ds.items[1].width(), 3.0);
    assert!(ds.undo()); // width
    assert_eq!(
        ds.items[1].width(),
        Drawing::new(DrawingTool::HorizontalLine, vec![]).width()
    );
    assert!(ds.undo()); // second line
    assert_eq!(ds.items.len(), 1);
    assert!(ds.undo()); // first line
    assert!(ds.items.is_empty());
    assert!(!ds.undo());
    assert!(ds.redo() && ds.redo() && ds.redo());
    assert_eq!(ds.items.len(), 2);
    assert_eq!(ds.items[1].width(), 3.0);
    // A new edit drops the redo stack.
    ds.select_only(id);
    ds.edit_selected(|d| d.locked = true);
    assert!(!ds.can_redo());

    // Replacing a drawing (the settings dialog) is one step.
    let mut d = ds.items[0].clone();
    d.text = "note".into();
    assert!(ds.replace_drawing(d));
    assert!(ds.undo());
    assert!(ds.items[0].text.is_empty());

    // Bounded.
    for _ in 0..MAX_HISTORY + 20 {
        ds.edit_selected(|d| d.locked = !d.locked);
    }
    assert_eq!(ds.history.len(), MAX_HISTORY);

    // A new symbol's drawings clear the history.
    ds.set_items(Vec::new());
    assert!(!ds.can_undo());
}

#[test]
fn paste_keeps_anchors_or_offsets_onto_copies() {
    let t = times();
    let m = mapper(&t);
    let (mut src, a, _) = two_lines(&m);
    src.select_only(a);
    let clip = src.copy_selected();
    assert_eq!(clip.len(), 1);

    // Another chart: same anchors.
    let mut other = Drawings::empty();
    assert!(other.paste(&clip, &m));
    assert_eq!(other.items[0].points, clip[0].points);
    assert_ne!(other.items[0].id, a);
    assert_eq!(other.selection(), &[other.items[0].id]);

    // The same chart: offset right (2 bars at 10 pt) and down.
    assert!(src.paste(&clip, &m));
    let p = &src.items[2];
    assert_eq!(p.points[0].time_ms, 12 * MIN);
    assert!(p.points[0].price < 80.0);
    // Again: offset twice (the first paste is in the way now).
    assert!(src.paste(&clip, &m));
    assert_eq!(src.items[3].points[0].time_ms, 14 * MIN);
    // Clone pastes the selection.
    assert!(src.clone_selected(&m));
    assert_eq!(src.items.len(), 5);
    assert!(src.undo());
    assert_eq!(src.items.len(), 4);
}

#[test]
fn visual_order_moves_the_selection() {
    let mut ds = Drawings::empty();
    ds.items = (0..4).map(|k| line(k, 1.0, k + 1, 1.0)).collect();
    let ids: Vec<u64> = ds.items.iter().map(|d| d.id).collect();
    let order = |ds: &Drawings| ds.items.iter().map(|d| d.id).collect::<Vec<_>>();
    ds.select_only(ids[1]);
    assert!(ds.reorder_selected(VisualOrder::BringToFront));
    assert_eq!(order(&ds), [ids[0], ids[2], ids[3], ids[1]]);
    assert!(
        !ds.reorder_selected(VisualOrder::BringForward),
        "already on top"
    );
    assert!(ds.reorder_selected(VisualOrder::SendBackward));
    assert_eq!(order(&ds), [ids[0], ids[2], ids[1], ids[3]]);
    ds.toggle_selected(ids[3]);
    assert!(ds.reorder_selected(VisualOrder::SendToBack));
    assert_eq!(order(&ds), [ids[1], ids[3], ids[0], ids[2]]);
    assert!(ds.reorder_selected(VisualOrder::BringForward));
    assert_eq!(order(&ds), [ids[0], ids[1], ids[3], ids[2]]);
    assert!(ds.undo());
    assert_eq!(order(&ds), [ids[1], ids[3], ids[0], ids[2]]);
    assert!(ds.undo() && ds.undo() && ds.undo());
    assert_eq!(order(&ds), ids);
}
