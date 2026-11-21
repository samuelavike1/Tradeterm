//! Line tools: trend line, ray, extended line, info line, trend angle,
//! arrow (all `LineProps`), horizontal line / ray, vertical line, cross line.

use super::hit::extend;
use super::props::{HAlign, LineEnd, LineProps, Props, Stroke, TextLayout, TextStyle, VAlign};
use super::render::styled_line;
use super::stats::{self, Axes, Buf, Lines, StatInputs};
use super::{Drawing, DrawingTool};
use crate::draw::Scene;
use crate::render::{Canvas, TextOpts};
use emath::{Align, Align2, Pos2, Rect, Vec2, pos2, vec2};
use std::f32::consts::{FRAC_PI_2, PI};
use tt_theme::metrics::TEXT_LINE_GAP;

/// Tools drawn from `LineProps`.
pub(super) fn is_line_family(tool: DrawingTool) -> bool {
    use DrawingTool::*;
    matches!(
        tool,
        TrendLine | Ray | ExtendedLine | InfoLine | TrendAngle | Arrow
    )
}

/// The drawn segment of a two-point line: `a → b`, extended to the edges of
/// `pane` on the sides the props say.
pub(super) fn segment(lp: &LineProps, a: Pos2, b: Pos2, pane: Rect) -> (Pos2, Pos2) {
    if lp.extend.left || lp.extend.right {
        extend(a, b, pane, lp.extend.left, lp.extend.right)
    } else {
        (a, b)
    }
}

/// Arrow head length for a line width.
fn head_size(width: f32) -> f32 {
    6.0 + 2.0 * width
}

pub(super) fn draw(c: &mut dyn Canvas, s: &Scene<'_>, d: &Drawing, pts: &[Pos2]) {
    use DrawingTool::*;
    let pane = s.vp_rect();
    match (&d.props, d.tool) {
        (Props::Line(lp), _) if pts.len() >= 2 => line(c, s, d, lp, pts),
        (Props::HLine(hp), HorizontalLine | HorizontalRay) => {
            let (w, col, style) = parts(&hp.line);
            let y = pts[0].y;
            let x0 = if d.tool == HorizontalRay {
                pts[0].x
            } else {
                pane.left()
            };
            // TradingView shows the text whenever there is some (the dialog
            // has no "Show text" box for these tools).
            if d.text.is_empty() {
                styled_line(c, pos2(x0, y), pos2(pane.right(), y), w, col, style);
            } else {
                // The visible part: the text centers on what is on screen.
                let (a, b) = (pos2(x0.max(pane.left()), y), pos2(pane.right(), y));
                if x0 < a.x {
                    styled_line(c, pos2(x0, y), a, w, col, style);
                }
                line_with_text(c, a, b, &hp.line, &d.text, &hp.text_style, &hp.text);
            }
        }
        (Props::VLine(vp), VerticalLine) => {
            let (w, col, style) = parts(&vp.line);
            let span = if vp.extend { s.plot } else { pane };
            c.set_clip(span);
            let x = pts[0].x;
            let with_text = vp.text.on && !d.text.is_empty();
            // Text along the line and on it breaks it (as on other lines).
            let gap = (with_text && vp.text.vertical && vp.text.h == HAlign::Center).then(|| {
                let ts = &vp.text_style;
                let tw = c.text_size_styled(&d.text, ts.size as f32, ts.bold).x;
                let (y0, y1) = vertical_text_span(pane, tw, vp.text.v);
                (y0 - TEXT_LINE_GAP, y1 + TEXT_LINE_GAP)
            });
            match gap {
                Some((g0, g1)) => {
                    if g0 > span.top() {
                        styled_line(c, pos2(x, span.top()), pos2(x, g0), w, col, style);
                    }
                    if g1 < span.bottom() {
                        styled_line(c, pos2(x, g1), pos2(x, span.bottom()), w, col, style);
                    }
                }
                None => styled_line(
                    c,
                    pos2(x, span.top()),
                    pos2(x, span.bottom()),
                    w,
                    col,
                    style,
                ),
            }
            c.set_clip(pane);
            if with_text {
                vertical_text(c, x, pane, &d.text, &vp.text_style, &vp.text);
            }
        }
        (Props::Cross(cp), CrossLine) => {
            let (w, col, style) = parts(&cp.line);
            c.set_clip(s.plot);
            let x = pts[0].x;
            styled_line(
                c,
                pos2(x, s.plot.top()),
                pos2(x, s.plot.bottom()),
                w,
                col,
                style,
            );
            c.set_clip(pane);
            let y = pts[0].y;
            styled_line(
                c,
                pos2(pane.left(), y),
                pos2(pane.right(), y),
                w,
                col,
                style,
            );
        }
        _ => {}
    }
}

fn parts(st: &Stroke) -> (f32, ecolor::Color32, super::LineStyle) {
    (st.width as f32, st.color.to_color32(), st.style)
}

fn line(c: &mut dyn Canvas, s: &Scene<'_>, d: &Drawing, lp: &LineProps, pts: &[Pos2]) {
    let pane = s.vp_rect();
    let (w, col, style) = parts(&lp.line);
    let (p0, p1) = (pts[0], pts[1]);
    let (a, b) = segment(lp, p0, p1, pane);
    let with_text = lp.text.on && !d.text.is_empty();
    if with_text {
        // Text centered on the segment between the points (not the
        // extensions); a Middle text breaks the line behind it.
        let (ea, eb) = if (a - p0).length_sq() <= (a - p1).length_sq() {
            (a, b)
        } else {
            (b, a)
        };
        if (ea - p0).length_sq() > 0.25 {
            styled_line(c, ea, p0, w, col, style);
        }
        if (eb - p1).length_sq() > 0.25 {
            styled_line(c, p1, eb, w, col, style);
        }
        line_with_text(c, p0, p1, &lp.line, &d.text, &lp.text_style, &lp.text);
    } else {
        styled_line(c, a, b, w, col, style);
    }
    let dir = p1 - p0;
    if dir.length_sq() > 1.0 {
        let u = dir.normalized();
        if lp.ends.left == LineEnd::Arrow && !lp.extend.left {
            c.arrow_head(p0, -u, head_size(w), w, col);
        }
        if lp.ends.right == LineEnd::Arrow && !lp.extend.right {
            c.arrow_head(p1, u, head_size(w), w, col);
        }
    }
    if d.tool == DrawingTool::TrendAngle {
        trend_angle(c, lp, p0, p1);
    }
    if lp.middle_point {
        let m = p0 + dir * 0.5;
        c.fill_circle(m, 3.0 + w / 2.0, col);
        c.fill_circle(m, 1.5 + w / 2.0, s.theme.bg);
    }
    if lp.stats.always {
        line_stats(c, s, d, lp, pts);
    }
}

/// The trend angle's horizontal reference, arc and `{angle}°` label.
fn trend_angle(c: &mut dyn Canvas, lp: &LineProps, p0: Pos2, p1: Pos2) {
    let d = p1 - p0;
    let len = d.length();
    if len < 2.0 {
        return;
    }
    let col = lp.line.color.to_color32();
    let r = (len * 0.4).clamp(16.0, 60.0);
    c.dashed_line(p0, p0 + vec2(len.max(r + 8.0), 0.0), 1.0, col, 4.0, 4.0);
    // Screen angle (clockwise, as y grows downwards).
    let theta = d.y.atan2(d.x);
    const N: usize = 32;
    let mut arc = [Pos2::ZERO; N + 1];
    for (i, q) in arc.iter_mut().enumerate() {
        let t = theta * i as f32 / N as f32;
        *q = p0 + vec2(t.cos(), t.sin()) * r;
    }
    c.polyline(&arc, 1.0, col);
    let mut b = Buf::new();
    let _ = std::fmt::Write::write_fmt(&mut b, format_args!("{:.2}°", -theta.to_degrees()));
    let half = theta / 2.0;
    let at = p0 + vec2(half.cos(), half.sin()) * (r + 6.0);
    let ts = &lp.text_style;
    c.text_rich(
        at,
        Align2::LEFT_CENTER,
        b.as_str(),
        ts.size as f32,
        ts.color.to_color32(),
        TextOpts {
            bold: ts.bold,
            italic: ts.italic,
            rotation: 0.0,
        },
    );
}

/// The segment `a → b` in `st` with `text` along it (TradingView): a
/// `Middle` text sits on the line, which breaks behind it with
/// [`TEXT_LINE_GAP`] on each side; `Top` / `Bottom` text sits above / below
/// the unbroken line. Any angle (horizontal lines, trend lines); no
/// allocation (one text measure).
pub(super) fn line_with_text(
    c: &mut dyn Canvas,
    a: Pos2,
    b: Pos2,
    st: &Stroke,
    text: &str,
    ts: &TextStyle,
    lay: &TextLayout,
) {
    let (w, col, style) = parts(st);
    let (a, b) = if b.x < a.x { (b, a) } else { (a, b) };
    let d = b - a;
    let len = d.length();
    if lay.v != VAlign::Middle || text.is_empty() || len < 1.0 {
        styled_line(c, a, b, w, col, style);
        text_along(c, a, b, text, ts, lay);
        return;
    }
    let u = d / len;
    let tw = c.text_size_styled(text, ts.size as f32, ts.bold).x;
    // Where the text's center falls along the segment (as in `text_along`).
    let center = match lay.h {
        HAlign::Left => 4.0 + tw / 2.0,
        HAlign::Center => len / 2.0,
        HAlign::Right => len - 4.0 - tw / 2.0,
    };
    let (g0, g1) = gap_span(center, tw / 2.0 + TEXT_LINE_GAP, len);
    if g0 > 0.0 {
        styled_line(c, a, a + u * g0, w, col, style);
    }
    if g1 < len {
        styled_line(c, a + u * g1, b, w, col, style);
    }
    text_along(c, a, b, text, ts, lay);
}

/// The broken part `(start, end)` of a segment `len` long around `center`
/// (`half` each side), clamped to the segment.
pub(super) fn gap_span(center: f32, half: f32, len: f32) -> (f32, f32) {
    (
        (center - half).clamp(0.0, len),
        (center + half).clamp(0.0, len),
    )
}

/// Text along the line `a → b`, kept upright, placed by `layout`
/// (horizontal: start / middle / end; vertical: above / on / below).
pub(super) fn text_along(
    c: &mut dyn Canvas,
    a: Pos2,
    b: Pos2,
    text: &str,
    ts: &TextStyle,
    lay: &TextLayout,
) {
    let (a, b) = if b.x < a.x { (b, a) } else { (a, b) };
    let d = b - a;
    let mut rot = d.y.atan2(d.x);
    if rot > FRAC_PI_2 {
        rot -= PI;
    } else if rot < -FRAC_PI_2 {
        rot += PI;
    }
    let (hx, at) = match lay.h {
        HAlign::Left => (Align::Min, a + d.normalized() * 4.0),
        HAlign::Center => (Align::Center, a + d * 0.5),
        HAlign::Right => (Align::Max, b - d.normalized() * 4.0),
    };
    let n = Vec2::new(-rot.sin(), rot.cos());
    let (vy, at) = match lay.v {
        VAlign::Top => (Align::Max, at - n * 3.0),
        VAlign::Middle => (Align::Center, at),
        VAlign::Bottom => (Align::Min, at + n * 3.0),
    };
    c.text_rich(
        at,
        Align2([hx, vy]),
        text,
        ts.size as f32,
        ts.color.to_color32(),
        TextOpts {
            bold: ts.bold,
            italic: ts.italic,
            rotation: rot,
        },
    );
}

/// The y span `(top, bottom)` of a vertical line's rotated text `len`
/// long (as [`vertical_text`] places it).
pub(super) fn vertical_text_span(pane: Rect, len: f32, v: VAlign) -> (f32, f32) {
    match v {
        VAlign::Top => (pane.top() + 6.0, pane.top() + 6.0 + len),
        VAlign::Middle => (pane.center().y - len / 2.0, pane.center().y + len / 2.0),
        VAlign::Bottom => (pane.bottom() - 6.0 - len, pane.bottom() - 6.0),
    }
}

/// A vertical line's text: rotated along the line (or horizontal), at the
/// top / middle / bottom of the pane, left or right of the line.
fn vertical_text(
    c: &mut dyn Canvas,
    x: f32,
    pane: Rect,
    text: &str,
    ts: &TextStyle,
    lay: &TextLayout,
) {
    let opts = TextOpts {
        bold: ts.bold,
        italic: ts.italic,
        rotation: if lay.vertical { -FRAC_PI_2 } else { 0.0 },
    };
    let y = match lay.v {
        VAlign::Top => pane.top() + 6.0,
        VAlign::Middle => pane.center().y,
        VAlign::Bottom => pane.bottom() - 6.0,
    };
    let (x, side) = match lay.h {
        HAlign::Left => (x - 4.0, Align::Max),
        HAlign::Center => (x, Align::Center),
        HAlign::Right => (x + 4.0, Align::Min),
    };
    // Rotated -90°, the box's x runs up the screen and its y to the right.
    let anchor = if lay.vertical {
        let along = match lay.v {
            VAlign::Top => Align::Max,
            VAlign::Middle => Align::Center,
            VAlign::Bottom => Align::Min,
        };
        let across = match side {
            Align::Max => Align::Min,
            Align::Center => Align::Center,
            Align::Min => Align::Max,
        };
        Align2([along, across])
    } else {
        let vy = match lay.v {
            VAlign::Top => Align::Min,
            VAlign::Middle => Align::Center,
            VAlign::Bottom => Align::Max,
        };
        Align2([side, vy])
    };
    c.text_rich(
        pos2(x, y),
        anchor,
        text,
        ts.size as f32,
        ts.color.to_color32(),
        opts,
    );
}

/// Stats of a line-family drawing that shows them only while selected or
/// hovered (the pointer layer calls this).
pub(super) fn overlay_stats(c: &mut dyn Canvas, s: &Scene<'_>, d: &Drawing, pts: &[Pos2]) {
    if let Props::Line(lp) = &d.props
        && !lp.stats.always
        && pts.len() >= 2
    {
        line_stats(c, s, d, lp, pts);
    }
}

/// The stats box next to the second point (Right / Auto), the first point
/// (Left) or the middle (Center).
fn line_stats(c: &mut dyn Canvas, s: &Scene<'_>, d: &Drawing, lp: &LineProps, pts: &[Pos2]) {
    let (p0, p1) = (pts[0], pts[1]);
    let mut st = lp.stats;
    // The trend angle's dialog offers price range, percent, pips and bars.
    if d.tool == DrawingTool::TrendAngle {
        st.date_time = false;
        st.angle = false;
        st.volume = false;
    }
    let inp = StatInputs {
        from: d.points[0],
        to: d.points[1],
        bars: ((p1.x - p0.x) / s.vp.bar_spacing).round() as i64,
        angle_deg: stats::angle_deg(p0, p1),
        precision: s.precision,
        tick: stats::tick_or_precision(s.tick_size, s.precision),
        volume: None,
    };
    let mut lines = Lines::default();
    stats::stats_lines(&st, Axes::BOTH, &inp, &mut lines);
    if lines.is_empty() {
        return;
    }
    use super::props::StatsPosition::*;
    let at = match st.position {
        Left => p0,
        Center => p0 + (p1 - p0) * 0.5,
        Right | Auto => p1,
    };
    // Beside the point, away from the line.
    let below = match st.position {
        Left => p0.y >= p1.y,
        _ => p1.y >= p0.y,
    };
    let (at, align) = if below {
        (at + vec2(0.0, 10.0), Align2::CENTER_TOP)
    } else {
        (at - vec2(0.0, 10.0), Align2::CENTER_BOTTOM)
    };
    let size = s.style.scales.text_size;
    let bg = lp.line.color.to_color32();
    stats::draw_box(
        c,
        &lines,
        at,
        align,
        size,
        false,
        s.theme.text_on_accent,
        Some(bg),
    );
}
