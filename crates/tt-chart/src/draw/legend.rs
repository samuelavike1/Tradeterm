//! The status line: the OHLC readout and the indicator legends.

use super::{LegendButton, LegendHit, Scene, volume_rising_at};
use crate::indicators::{Indicator, format_value, histogram_down};
use crate::render::Canvas;
use crate::scale::{self, format_price, format_signed_price};
use ecolor::Color32;
use emath::{Align2, Pos2, Rect, pos2, vec2};
use lucide_icons::Icon;
use tt_core::{Placement, Plot};
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
    let hovered = s.legend_hover == Some(ind.id);
    let full = ind.study.legend_title();
    let title = match (
        s.style.status_line.indicator_titles,
        s.style.status_line.indicator_inputs,
    ) {
        (false, _) => String::new(),
        (true, true) => full,
        (true, false) => full.split_whitespace().next().unwrap_or("").to_owned(),
    };
    let name = Rect::from_min_size(pos, c.text_size(&title, size));
    let name_color = if ind.visible {
        t.text
    } else {
        with_alpha(t.text_muted, 0.6)
    };
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
        c.fill_rounded_rect(r, 3.0, with_alpha(t.bg, 0.85));
        c.icon(r.center(), char::from(*icon), LEGEND_ICON, name_color);
        buttons.push((*button, r));
        x = r.right() + 2.0;
    }
    if !shown.is_empty() {
        x += 6.0;
    }
    if ind.visible
        && s.style.status_line.indicator_values
        && let Some(i) = bar
    {
        let outs = ind.outputs();
        for (k, (_, plot)) in ind.study.outputs().iter().enumerate() {
            if !ind.output_enabled(k) {
                continue;
            }
            let Some(v) = outs.get(k).and_then(|o| o.get(i)).copied() else {
                continue;
            };
            let color = match plot {
                Plot::VolumeColumns if volume_rising_at(s, ind, i) => t.up,
                Plot::VolumeColumns => t.down,
                Plot::Histogram if v < 0.0 => histogram_down(),
                _ => ind.color(k),
            };
            let r = c.text(
                pos2(x, pos.y),
                Align2::LEFT_TOP,
                &format_value(&ind.study, v, s.precision),
                size,
                color,
            );
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
