//! The status line: the OHLC readout and the indicator legends.

use super::study::value_color;
use super::{LegendButton, LegendHit, Scene};
use crate::indicators::Indicator;
use crate::render::Canvas;
use crate::scale::{self, format_price, format_signed_price};
use ecolor::Color32;
use emath::{Align2, Pos2, Rect, pos2, vec2};
use lucide_icons::Icon;
use tt_core::Placement;
use tt_theme::with_alpha;

const LEGEND_ICON: f32 = 15.0;
const LEGEND_BUTTON: f32 = 20.0;

/// Legends: price-pane indicators under the OHLC readout (collapsible), and
/// each pane's indicator at its top left. Values are for `bar`.
pub(super) fn legends(c: &mut dyn Canvas, s: &Scene<'_>, bar: Option<usize>) -> Vec<LegendHit> {
    let mut hits = Vec::new();
    let row_h = s.style.scales.text_size + 10.0;
    let mut pos = s.plot.left_top() + vec2(10.0, 8.0 + row_h);
    let main: Vec<&Indicator> = s
        .indicators
        .iter()
        .filter(|i| i.placement() != Placement::Pane)
        .collect();
    if !main.is_empty() && !s.main_collapsed {
        if !s.legend_collapsed {
            for ind in &main {
                hits.push(legend_row(c, s, pos, ind, bar));
                pos.y += row_h;
            }
        }
        // Collapse / expand toggle; collapsed it shows the indicator count.
        let count = if s.legend_collapsed {
            main.len().to_string()
        } else {
            String::new()
        };
        let text_w = if count.is_empty() {
            0.0
        } else {
            c.text_size(&count, s.style.scales.text_size).x + 6.0
        };
        let rect = Rect::from_min_size(
            pos - vec2(4.0, 2.0),
            vec2(LEGEND_BUTTON + text_w, LEGEND_BUTTON),
        );
        let hovered = s.legend_hover == Some(0);
        let bg = if hovered {
            s.theme.selection
        } else {
            with_alpha(s.theme.bg, 0.85)
        };
        c.fill_rounded_rect(rect, 4.0, bg);
        c.stroke_rect(rect, 1.0, s.theme.border);
        let icon = if s.legend_collapsed {
            Icon::ChevronDown
        } else {
            Icon::ChevronUp
        };
        let ic = c.icon(
            pos2(rect.left() + LEGEND_BUTTON / 2.0, rect.center().y),
            char::from(icon),
            LEGEND_ICON,
            s.theme.text,
        );
        if !count.is_empty() {
            c.text(
                pos2(ic.right() + 2.0, rect.center().y),
                Align2::LEFT_CENTER,
                &count,
                s.style.scales.text_size,
                s.theme.text,
            );
        }
        hits.push(LegendHit {
            id: 0,
            row: rect,
            buttons: vec![(LegendButton::Collapse, rect)],
        });
    }
    for pane in &s.panes {
        let pos = pane.rect().left_top() + vec2(10.0, 6.0);
        hits.push(legend_row(c, s, pos, &s.indicators[pane.index], bar));
    }
    hits
}

/// Backing behind legend text, so plots and levels (RSI 30/50/70) don't run
/// through it: the theme background when the status line's "Background" is
/// on. Its transparency (0–100 %) maps to 0–30 % see-through, so the text
/// always stays legible.
fn backing(s: &Scene<'_>) -> Option<Color32> {
    let st = &s.style.status_line;
    st.indicator_background.then(|| {
        let see_through = st.indicator_background_transparency.min(100) as f32 / 100.0 * 0.3;
        with_alpha(s.theme.bg, 1.0 - see_through)
    })
}

/// One legend row: title, hover buttons (eye / settings / remove), values.
fn legend_row(
    c: &mut dyn Canvas,
    s: &Scene<'_>,
    pos: Pos2,
    ind: &Indicator,
    bar: Option<usize>,
) -> LegendHit {
    let t = s.theme;
    let size = s.style.scales.text_size;
    let st = &s.style.status_line;
    let hovered = s.legend_hover == Some(ind.id);
    let title = if !st.indicator_titles {
        String::new()
    } else if st.indicator_inputs && ind.style.inputs_in_status_line {
        ind.study.legend_title()
    } else {
        ind.study.meta().short.to_owned()
    };
    let name = Rect::from_min_size(pos, c.text_size(&title, size));
    let name_color = if ind.visible {
        t.text
    } else {
        with_alpha(t.text_muted, 0.6)
    };
    // The backing grows left to right as items are drawn (one strip, no
    // overlaps, no allocation).
    let bg = backing(s);
    let (top, bottom) = (pos.y - 3.0, name.bottom() + 3.0);
    let mut covered = pos.x - 5.0;
    let mut cover = |c: &mut dyn Canvas, right: f32| {
        if let Some(bg) = bg
            && right > covered
        {
            c.fill_rect(
                Rect::from_min_max(pos2(covered, top), pos2(right, bottom)),
                bg,
            );
            covered = right;
        }
    };
    if !title.is_empty() {
        cover(c, name.right() + 5.0);
    }
    if hovered {
        let r = name.expand2(vec2(5.0, 3.0));
        c.fill_rounded_rect(r, 3.0, with_alpha(t.bg, 0.85));
        c.stroke_rect(r, 1.0, t.border);
    }
    c.text(pos, Align2::LEFT_TOP, &title, size, name_color);
    let mut x = name.right() + 8.0;
    let eye = if ind.visible { Icon::Eye } else { Icon::EyeOff };
    let shown: &[(LegendButton, Icon)] = if hovered {
        &[
            (LegendButton::ToggleVisible, eye),
            (LegendButton::Settings, Icon::Settings),
            (LegendButton::Remove, Icon::Trash2),
        ]
    } else if !ind.visible {
        &[(LegendButton::ToggleVisible, eye)]
    } else {
        &[]
    };
    let mut buttons = Vec::new();
    for (button, icon) in shown {
        let r = Rect::from_min_size(
            pos2(x, name.center().y - LEGEND_BUTTON / 2.0),
            vec2(LEGEND_BUTTON, LEGEND_BUTTON),
        );
        cover(c, r.right() + 2.0);
        c.fill_rounded_rect(r, 3.0, with_alpha(t.bg, 0.85));
        c.icon(r.center(), char::from(*icon), LEGEND_ICON, name_color);
        buttons.push((*button, r));
        x = r.right() + 2.0;
    }
    if !shown.is_empty() {
        x += 6.0;
    }
    if ind.visible
        && st.indicator_values
        && ind.style.values_in_status_line
        && let Some(i) = bar
    {
        let off = ind.offset();
        for k in 0..ind.outputs().len() {
            if !ind.output_enabled(k) {
                continue;
            }
            let Some(v) = ind.value_at(k, i) else {
                continue;
            };
            let color = value_color(s, ind, k, (i as i64 - off) as usize);
            let text = ind.format(v, s.precision);
            if bg.is_some() {
                let w = c.text_size(&text, size).x;
                cover(c, x + w + 4.0);
            }
            let r = c.text(pos2(x, pos.y), Align2::LEFT_TOP, &text, size, color);
            x = r.right() + 8.0;
        }
    }
    // Hover area covers where the buttons appear, so they don't flicker.
    let right = x.max(name.right() + 3.0 * LEGEND_BUTTON + 16.0);
    LegendHit {
        id: ind.id,
        row: Rect::from_min_max(pos - vec2(6.0, 3.0), pos2(right, name.bottom() + 3.0)),
        buttons,
    }
}

/// Top-left readout: title, OHLC, change vs previous close and volume of the
/// hovered (or last) bar.
pub(super) fn readout(c: &mut dyn Canvas, s: &Scene<'_>, bar: Option<usize>) {
    let t = s.theme;
    let size = s.style.scales.text_size;
    let mut pos = s.plot.left_top() + vec2(10.0, 8.0);
    let mut put = |c: &mut dyn Canvas, text: &str, color: Color32, gap: f32| {
        let r = c.text(pos, Align2::LEFT_TOP, text, size, color);
        pos.x = r.right() + gap;
    };

    let st = &s.style.status_line;
    if st.title && !s.title.is_empty() {
        put(c, s.title, t.text, 14.0);
    }
    let Some(i) = bar else { return };
    let Some(k) = s.display.get(i) else { return };
    let prev_close = i
        .checked_sub(1)
        .and_then(|j| s.display.get(j))
        .map_or(k.open, |p| p.close);
    let change = k.close - prev_close;
    let (color, _) = s.colors(change >= 0.0);
    let d = s.precision;
    if st.chart_values {
        for (name, v) in [("O", k.open), ("H", k.high), ("L", k.low), ("C", k.close)] {
            put(c, name, t.text_muted, 3.0);
            put(c, &format_price(v, d), color, 10.0);
        }
    }
    if st.bar_change {
        let pct = if prev_close != 0.0 {
            change / prev_close * 100.0
        } else {
            0.0
        };
        put(
            c,
            &format!("{} ({pct:+.2}%)", format_signed_price(change, d)),
            color,
            14.0,
        );
    }
    if st.volume {
        put(c, "Vol", t.text_muted, 3.0);
        put(c, &scale::format_volume(k.volume), color, 10.0);
    }
}
