//! The data window tab (docs/specs/right-panel.md §4): the bar under the
//! crosshair (the last bar when there is none): date and time, the series'
//! open / high / low / close / change / volume / last day change colored by
//! the bar's direction, then each indicator's plot values in their colors.

use crate::app::TradeTerm;
use crate::object_tree::ObjectTreePanel;
use crate::ui::chart_type_icon;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::collections::HashSet;
use tt_chart::scale::{DateTime, format_price};
use tt_theme::Color32;
use tt_ui::hsla;
use tt_ui::prelude::*;
use tt_ui::tokens::*;

/// TradingView writes negative numbers with a real minus sign.
pub fn tv_minus(s: &str) -> String {
    s.replace('-', "\u{2212}")
}

/// Signed price, TradingView style: `+910.06`, `−1,226.90`.
pub fn signed(v: f64, decimals: usize) -> String {
    tv_minus(&tt_chart::scale::format_signed_price(v, decimals))
}

/// Signed percent with two decimals: `+1.55%`, `−0.06%`.
pub fn signed_pct(p: f64) -> String {
    let s = format!("{p:+.2}%");
    // No sign for a value that rounds to zero.
    if s == "-0.00%" || s == "+0.00%" {
        return "0.00%".to_owned();
    }
    tv_minus(&s)
}

/// "−33.43 (−0.06%)": the change from `base` to `value`.
pub fn change_text(value: f64, base: f64, decimals: usize) -> String {
    let ch = value - base;
    if base == 0.0 {
        return signed(ch, decimals);
    }
    format!(
        "{} ({})",
        signed(ch, decimals),
        signed_pct(ch / base * 100.0)
    )
}

/// Compact volume with a space before the unit, like TradingView's data
/// window: `172.72 M`, `18.41 B`, `950`.
pub fn volume_text(v: f64) -> String {
    let a = v.abs();
    let (x, unit) = if a >= 1e9 {
        (v / 1e9, " B")
    } else if a >= 1e6 {
        (v / 1e6, " M")
    } else if a >= 1e3 {
        (v / 1e3, " K")
    } else {
        return tv_minus(&format!("{}", (v * 100.0).round() / 100.0));
    };
    tv_minus(&format!("{x:.2}{unit}"))
}

/// Date and time rows: `Sat 21 Nov '26`, `10:30` (local time of the
/// chart's zone, given as `local_ms`).
pub fn date_text(local_ms: i64) -> String {
    let dt = DateTime::from_unix_ms(local_ms);
    format!(
        "{} {} {} '{:02}",
        DateTime::weekday_name(local_ms),
        dt.day,
        dt.month_name(),
        dt.year.rem_euclid(100)
    )
}

pub fn time_text(local_ms: i64) -> String {
    let dt = DateTime::from_unix_ms(local_ms);
    format!("{:02}:{:02}", dt.hour, dt.minute)
}

/// One label / value row.
fn value_row(label: SharedString, value: String, color: Hsla) -> impl IntoElement {
    h_flex()
        .flex_none()
        .h(H_DW_ROW.px())
        .px(S4.px())
        .justify_between()
        .gap(S2.px())
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(label),
        )
        .child(div().flex_none().text_color(color).child(value))
}

/// A section header: chevron, icon, bold title.
fn section(
    id: u64,
    icon: Glyph,
    title: String,
    open: bool,
    muted: bool,
    t: &tt_theme::Theme,
    cx: &mut Context<ObjectTreePanel>,
) -> impl IntoElement {
    h_flex()
        .id(SharedString::from(format!("dw-sec-{id}")))
        .flex_none()
        .h(H_TREE_ROW.px())
        .px(S2.px())
        .gap(S1.px())
        .cursor_pointer()
        .font_weight(FontWeight::SEMIBOLD)
        .when(muted, |d| d.text_color(hsla(t.text_disabled)))
        .hover(|s| s.bg(hsla(t.hover)))
        .on_click(cx.listener(move |this, _, _, cx| this.toggle_section(id, cx)))
        .child(glyph(
            if open {
                Glyph::CHEVRON_DOWN
            } else {
                Glyph::CHEVRON_RIGHT
            },
            ICON_XS,
        ))
        .child(glyph(icon, ICON_SVG))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(title),
        )
}

/// What the tab shows, read from the chart first (the chart is borrowed
/// from `cx`, the elements need it mutably).
enum Block {
    Row(SharedString, String, Hsla),
    Section {
        id: u64,
        icon: Glyph,
        title: String,
        open: bool,
        muted: bool,
        /// Gray band behind the series section.
        band: bool,
    },
}

fn blocks(app: &Entity<TradeTerm>, collapsed: &HashSet<u64>, cx: &App) -> Option<Vec<Block>> {
    let a = app.read(cx);
    let chart = a.tab_chart()?;
    let t = cx.ui();
    let state = chart.read(cx).state.read(cx);
    let tab = &state.source.tab;
    let view = &tab.chart;
    let series = &tab.series;
    let last = series.len().checked_sub(1)?;
    let bar = state
        .crosshair()
        .and_then(|p| view.bar_at_x(p, series.len()))
        .unwrap_or(last);
    let k = series.get(bar)?;
    let d = tab.price_precision();
    let prev_close = bar
        .checked_sub(1)
        .and_then(|i| series.get(i))
        .map_or(k.open, |p| p.close);
    let (up, down) = (hsla(t.up), hsla(t.down));
    let dir = if k.close >= prev_close { up } else { down };
    let text = hsla(t.text);
    let local = view.time_zone().local_ms(k.open_time_ms);
    let mut out = vec![Block::Row("Date".into(), date_text(local), text)];
    if tab.timeframe.duration_ms() < 86_400_000 {
        out.push(Block::Row("Time".into(), time_text(local), text));
    }
    let open = !collapsed.contains(&0);
    out.push(Block::Section {
        id: 0,
        icon: chart_type_icon(view.chart_type),
        title: format!(
            "{} · {} · {}",
            tab.display_symbol(),
            tab.timeframe,
            tab.source_name(&a.shared)
        ),
        open,
        muted: false,
        band: true,
    });
    if open {
        for (label, v) in [
            ("Open", k.open),
            ("High", k.high),
            ("Low", k.low),
            ("Close", k.close),
        ] {
            out.push(Block::Row(label.into(), format_price(v, d), dir));
        }
        out.push(Block::Row(
            "Change".into(),
            change_text(k.close, prev_close, d),
            dir,
        ));
        if k.volume > 0.0 {
            out.push(Block::Row("Vol".into(), volume_text(k.volume), dir));
        }
        if let Some(pct) = tab.change_pct() {
            // The last close vs. 24 h earlier (the tab strip's %).
            let close = series.get(last).map_or(0.0, |c| c.close);
            let base = close / (1.0 + pct / 100.0);
            out.push(Block::Row(
                "Last day change".into(),
                change_text(close, base, d),
                if pct >= 0.0 { up } else { down },
            ));
        }
    }
    // Indicators: price pane first, then panes (the object tree's order).
    let mut inds: Vec<&tt_chart::Indicator> = view.indicators.iter().collect();
    inds.sort_by_key(|i| i.placement() == tt_core::Placement::Pane);
    for ind in inds {
        let open = !collapsed.contains(&ind.id);
        out.push(Block::Section {
            id: ind.id,
            icon: Glyph::TREE_INDICATOR,
            title: ind.study.legend_title(),
            open,
            muted: !ind.shown(),
            band: false,
        });
        if !open {
            continue;
        }
        for k in 0..ind.outputs().len() {
            if !ind.output_enabled(k) {
                continue;
            }
            let Some(def) = ind.plot_def(k) else {
                continue;
            };
            let value = ind
                .value_at(k, bar)
                .filter(|v| v.is_finite())
                .map_or_else(|| "\u{2205}".to_owned(), |v| tv_minus(&ind.format(v, d)));
            let color: Color32 = if ind.shown() {
                ind.color(k)
            } else {
                t.text_disabled
            };
            out.push(Block::Row(def.title.into(), value, hsla(color)));
        }
    }
    Some(out)
}

/// The tab's body.
pub fn render(
    app: &WeakEntity<TradeTerm>,
    collapsed: &HashSet<u64>,
    _window: &mut Window,
    cx: &mut Context<ObjectTreePanel>,
) -> AnyElement {
    let Some(app) = app.upgrade() else {
        return Empty.into_any_element();
    };
    let Some(blocks) = blocks(&app, collapsed, cx) else {
        return EmptyState::new("No data").into_any_element();
    };
    let t = cx.ui().clone();
    let mut body = v_flex()
        .id("dw-body")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .text_size(TEXT_BODY.px())
        .text_color(hsla(t.text))
        .pb(S2.px());
    // Rows after the series header sit on its band.
    let mut band = false;
    for b in blocks {
        match b {
            Block::Row(label, value, color) => {
                body = body.child(
                    div()
                        .when(band, |d| d.bg(hsla(t.panel_alt)))
                        .child(value_row(label, value, color)),
                );
            }
            Block::Section {
                id,
                icon,
                title,
                open,
                muted,
                band: b,
            } => {
                band = b;
                body = body.child(
                    div()
                        .mt(S2.px())
                        .when(b, |d| d.bg(hsla(t.panel_alt)))
                        .child(section(id, icon, title, open, muted, &t, cx)),
                );
            }
        }
    }
    body.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn change_and_percent_like_tradingview() {
        assert_eq!(
            change_text(59_819.55, 59_852.98, 2),
            "\u{2212}33.43 (\u{2212}0.06%)"
        );
        assert_eq!(change_text(110.0, 100.0, 2), "+10.00 (+10.00%)");
        assert_eq!(change_text(5.0, 0.0, 2), "+5.00");
        assert_eq!(signed_pct(-0.001), "0.00%");
        assert_eq!(signed(-1226.9, 2), "\u{2212}1,226.90");
    }

    #[::core::prelude::v1::test]
    fn volume_units() {
        assert_eq!(volume_text(172_720_000.0), "172.72 M");
        assert_eq!(volume_text(18_410_000_000.0), "18.41 B");
        assert_eq!(volume_text(12_345.0), "12.35 K");
        assert_eq!(volume_text(950.0), "950");
        assert_eq!(volume_text(0.125), "0.13");
    }

    #[::core::prelude::v1::test]
    fn date_and_time() {
        // 2026-11-21 10:30 UTC was a Saturday.
        let ms = 1_795_257_000_000;
        assert_eq!(date_text(ms), "Sat 21 Nov '26");
        assert_eq!(time_text(ms), "10:30");
    }
}
