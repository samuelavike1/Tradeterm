//! Shapes: rectangle (and later circle, ellipse, polyline).
//!
//! The rectangle follows TradingView: eight handles (circles at the
//! corners, squares at the edge midpoints), edge handles move one side and
//! corner handles two, and the text sits inside on the middle line or above
//! / below the box.

use super::props::{HAlign, Props, ShapeProps, VAlign};
use super::render::styled_line;
use super::{Anchor, Drawing};
use crate::render::{Canvas, TextOpts};
use emath::{Align, Align2, Pos2, Rect, pos2};

/// Text inset from the rectangle's left / right border (TradingView: the
/// text's right edge sits about 8 px inside the border).
pub(super) const TEXT_PAD_X: f32 = 8.0;
/// Gap between the border and text above (Top) or below (Bottom) it.
pub(super) const TEXT_PAD_Y: f32 = 4.0;
/// Gap the middle line leaves before the text on it.
pub(super) const MIDDLE_LINE_GAP: f32 = 8.0;

/// The eight handles of a rectangle whose two points are `pts[0]` and
/// `pts[1]` (unordered): 0 and 1 are the points, 2 and 3 the other two
/// corners, 4..8 the edge midpoints (see [`RectHandle`]).
pub(super) fn rect_handles(pts: &[Pos2]) -> [Pos2; 8] {
    let (a, b) = (pts[0], pts[1]);
    let (mx, my) = ((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    [
        a,
        b,
        pos2(a.x, b.y),
        pos2(b.x, a.y),
        pos2(a.x, my),
        pos2(b.x, my),
        pos2(mx, a.y),
        pos2(mx, b.y),
    ]
}

/// What handle `k` of a rectangle moves: which point's time (x) and which
/// point's price (y). Corners move both, edges one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RectHandle {
    /// Index of the point whose time follows the pointer.
    pub x: Option<usize>,
    /// Index of the point whose price follows the pointer.
    pub y: Option<usize>,
}

impl RectHandle {
    pub fn of(k: usize) -> Option<Self> {
        let (x, y) = match k {
            0 => (Some(0), Some(0)),
            1 => (Some(1), Some(1)),
            2 => (Some(0), Some(1)),
            3 => (Some(1), Some(0)),
            4 => (Some(0), None),
            5 => (Some(1), None),
            6 => (None, Some(0)),
            7 => (None, Some(1)),
            _ => return None,
        };
        Some(Self { x, y })
    }

    pub fn is_corner(self) -> bool {
        self.x.is_some() && self.y.is_some()
    }
}

/// Moves handle `k` of a rectangle to `a`: an edge handle changes one side
/// only, a corner two. Dragging past the opposite side simply swaps which
/// point is on which side (the points are not ordered), so it flips.
pub(super) fn drag_rect_handle(points: &mut [Anchor], k: usize, a: Anchor) {
    let Some(h) = RectHandle::of(k) else {
        return;
    };
    if let Some(i) = h.x
        && let Some(p) = points.get_mut(i)
    {
        p.time_ms = a.time_ms;
    }
    if let Some(i) = h.y
        && let Some(p) = points.get_mut(i)
    {
        p.price = a.price;
    }
}

/// The corner opposite corner handle `k` (anchor and screen point): what
/// Shift keeps a square relative to. `None` for edge handles.
pub(super) fn opposite_corner(k: usize, points: &[Anchor], pts: &[Pos2]) -> Option<(Anchor, Pos2)> {
    let h = RectHandle::of(k).filter(|h| h.is_corner())?;
    let (xi, yi) = (1 - h.x?, 1 - h.y?);
    let a = Anchor {
        time_ms: points.get(xi)?.time_ms,
        price: points.get(yi)?.price,
    };
    Some((a, pos2(pts.get(xi)?.x, pts.get(yi)?.y)))
}

/// Pointer shape over a resize handle (TradingView: diagonal over the
/// corners, horizontal over the left / right edges, vertical over the top /
/// bottom edges).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeCursor {
    /// Top-left / bottom-right corner.
    Nwse,
    /// Top-right / bottom-left corner.
    Nesw,
    /// Left or right edge.
    Ew,
    /// Top or bottom edge.
    Ns,
}

/// The cursor over handle `k` of a rectangle with screen points `pts`.
pub(super) fn rect_cursor(pts: &[Pos2], k: usize) -> Option<ResizeCursor> {
    let h = RectHandle::of(k)?;
    let (a, b) = (*pts.first()?, *pts.get(1)?);
    Some(match (h.x, h.y) {
        (Some(xi), Some(yi)) => {
            // The handle's offset from the opposite corner: same signs is
            // the top-left / bottom-right diagonal.
            let (p, q) = ([a, b][xi], [a, b][yi]);
            let (o, r) = ([a, b][1 - xi], [a, b][1 - yi]);
            if (p.x - o.x) * (q.y - r.y) >= 0.0 {
                ResizeCursor::Nwse
            } else {
                ResizeCursor::Nesw
            }
        }
        (Some(_), None) => ResizeCursor::Ew,
        _ => ResizeCursor::Ns,
    })
}

/// The rectangle: background (`ShapeProps::background`), border, optional
/// middle line, extended left / right to the pane's edges.
pub(super) fn rectangle(c: &mut dyn Canvas, d: &Drawing, pts: &[Pos2], pane: Rect) {
    let fallback;
    let sp = match &d.props {
        Props::Shape(p) => p,
        _ => {
            fallback = ShapeProps::default();
            &fallback
        }
    };
    let r = Rect::from_two_pos(pts[0], pts[1]);
    let (left, right) = (
        if sp.extend.left {
            pane.left()
        } else {
            r.left()
        },
        if sp.extend.right {
            pane.right()
        } else {
            r.right()
        },
    );
    let wide = Rect::from_min_max(pos2(left, r.top()), pos2(right, r.bottom()));
    if sp.background.on {
        c.fill_rect(wide, sp.background.color.to_color32());
    }
    let (w, col, style) = (
        sp.border.width as f32,
        sp.border.color.to_color32(),
        sp.border.style,
    );
    // Top and bottom run the extended width; the sides stay at the points.
    styled_line(c, wide.left_top(), wide.right_top(), w, col, style);
    styled_line(c, wide.left_bottom(), wide.right_bottom(), w, col, style);
    if !sp.extend.left {
        styled_line(c, r.left_top(), r.left_bottom(), w, col, style);
    }
    if !sp.extend.right {
        styled_line(c, r.right_top(), r.right_bottom(), w, col, style);
    }
    // The text box, when there is text (the middle line stops short of it).
    let text = (sp.text.on && !d.text.is_empty()).then(|| {
        let ts = &sp.text_style;
        let size = c.text_size_styled(&d.text, ts.size as f32, ts.bold);
        let (pos, align) = text_anchor(r, sp.text.h, sp.text.v);
        (pos, align, align.anchor_size(pos, size))
    });
    if sp.middle_line_on {
        let m = &sp.middle_line;
        let (mw, mc, ms) = (m.width as f32, m.color.to_color32(), m.style);
        let y = r.center().y;
        let on_line = text.filter(|_| sp.text.v == VAlign::Middle).map(|t| t.2);
        for (x0, x1) in middle_line_pieces(left, right, on_line) {
            if x1 > x0 {
                styled_line(c, pos2(x0, y), pos2(x1, y), mw, mc, ms);
            }
        }
    }
    if let Some((pos, align, _)) = text {
        let ts = &sp.text_style;
        c.text_rich(
            pos,
            align,
            &d.text,
            ts.size as f32,
            ts.color.to_color32(),
            TextOpts {
                bold: ts.bold,
                italic: ts.italic,
                rotation: 0.0,
            },
        );
    }
}

/// Where the rectangle's text goes (TradingView): above the box (Top),
/// inside it on the middle line ("Inside") or below it (Bottom); inset from
/// the left / right border, or centered.
pub(super) fn text_anchor(r: Rect, h: HAlign, v: VAlign) -> (Pos2, Align2) {
    let (x, hx) = match h {
        HAlign::Left => (r.left() + TEXT_PAD_X, Align::Min),
        HAlign::Center => (r.center().x, Align::Center),
        HAlign::Right => (r.right() - TEXT_PAD_X, Align::Max),
    };
    let (y, vy) = match v {
        VAlign::Top => (r.top() - TEXT_PAD_Y, Align::Max),
        VAlign::Middle => (r.center().y, Align::Center),
        VAlign::Bottom => (r.bottom() + TEXT_PAD_Y, Align::Min),
    };
    (pos2(x, y), Align2([hx, vy]))
}

/// The middle line from `left` to `right`, broken around `text` (the text
/// box on the line, if any) with [`MIDDLE_LINE_GAP`] on each side.
pub(super) fn middle_line_pieces(left: f32, right: f32, text: Option<Rect>) -> [(f32, f32); 2] {
    match text {
        None => [(left, right), (right, right)],
        Some(t) => [
            (left, (t.left() - MIDDLE_LINE_GAP).min(right)),
            ((t.right() + MIDDLE_LINE_GAP).max(left), right),
        ],
    }
}

/// The bands a selected rectangle shows on the axes (TradingView): on the
/// price axis between its two prices and on the time axis between its two
/// times, clipped to the pane. `None` where the band is off screen.
pub(super) fn axis_bands(
    pts: &[Pos2],
    price_axis: Rect,
    time_axis: Rect,
    pane: Rect,
) -> (Option<Rect>, Option<Rect>) {
    let (Some(a), Some(b)) = (pts.first(), pts.get(1)) else {
        return (None, None);
    };
    let (top, bottom) = (a.y.min(b.y).max(pane.top()), a.y.max(b.y).min(pane.bottom()));
    let (left, right) = (a.x.min(b.x).max(pane.left()), a.x.max(b.x).min(pane.right()));
    let price = (top < bottom).then(|| {
        Rect::from_min_max(pos2(price_axis.left(), top), pos2(price_axis.right(), bottom))
    });
    let time = (left < right).then(|| {
        Rect::from_min_max(pos2(left, time_axis.top()), pos2(right, time_axis.bottom()))
    });
    (price, time)
}
