//! The Scales and lines page.

use super::*;
use tt_chart::style::{
    DateFormat, HoursFormat, LabelMode, LastValueMode, LineLabel, ScalePlacement, Visibility3,
};

/// Label / line combinations of a [`LineLabel`] (previous close, high and
/// low, average close, bid and ask).
const LINE_LABEL_MODES: [LabelMode; 4] = [
    LabelMode::ValueLine,
    LabelMode::Value,
    LabelMode::Line,
    LabelMode::Hidden,
];

/// TradingView's "Indicators and financials" options.
const INDICATOR_MODES: [LabelMode; 4] = [
    LabelMode::Value,
    LabelMode::NameValue,
    LabelMode::Name,
    LabelMode::Hidden,
];

fn visibility(v: Visibility3) -> (Visibility3, &'static str) {
    (v, v.label())
}

impl ChartSettingsPanel {
    /// Mode, color and width of a [`LineLabel`] field.
    #[allow(clippy::too_many_arguments)]
    fn line_label(
        &mut self,
        id: &'static str,
        label: &'static str,
        get: fn(&ChartStyle) -> &LineLabel,
        get_mut: fn(&mut ChartStyle) -> &mut LineLabel,
        default: fn(&ChartStyle, &Theme) -> Color32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let v = *get(&self.draft);
        let mode = self
            .select(
                format!("{id}.mode"),
                LINE_LABEL_MODES.iter().map(|m| (*m, m.label())),
                LabelMode::from_value_line(v.label, v.line),
                move |s, m| {
                    let l = get_mut(s);
                    l.label = m.shows_value();
                    l.line = m.shows_line();
                },
                cx,
            )
            .width(W_SELECT_SM + tokens::S4);
        let color = self.color(
            format!("{id}.color"),
            move |s| get(s).color,
            move |s, c| get_mut(s).color = c,
            default,
            window,
            cx,
        );
        let width = self.width(
            format!("{id}.width"),
            v.width,
            move |s, w| get_mut(s).width = w,
            cx,
        );
        Self::row(label)
            .control(mode)
            .control(color)
            .control(width)
            .into_any_element()
    }

    pub(super) fn scales_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Rows {
        let sc = self.draft.scales;
        let mut rows = vec![section("Price scale")];
        let sel = self
            .select(
                "sc.currency",
                Visibility3::ALL.map(visibility),
                sc.currency_label,
                |s, v| s.scales.currency_label = v,
                cx,
            )
            .width(W_SELECT_WIDE);
        rows.push(
            Self::row("Currency and unit")
                .control(sel)
                .into_any_element(),
        );
        let sel = self
            .select(
                "sc.buttons",
                Visibility3::ALL.map(visibility),
                sc.scale_buttons,
                |s, v| s.scales.scale_buttons = v,
                cx,
            )
            .width(W_SELECT_WIDE);
        rows.push(
            Self::row("Scale modes (A and L)")
                .control(sel)
                .into_any_element(),
        );
        let lock = self.check(
            "sc.lock",
            "Lock price to bar ratio",
            sc.lock_ratio.is_some(),
            |s, v| {
                s.scales.lock_ratio = v.then(|| s.scales.lock_ratio.unwrap_or(1.0));
            },
            cx,
        );
        let lock = if sc.lock_ratio.is_some() {
            let n = self.number(
                "sc.lock_ratio",
                |s| s.scales.lock_ratio.unwrap_or(1.0),
                |s, v| {
                    if s.scales.lock_ratio.is_some() && v > 0.0 {
                        s.scales.lock_ratio = Some(v);
                    }
                },
                0.000_000_01,
                1e12,
                0.1,
                window,
                cx,
            );
            lock.control(n)
        } else {
            lock
        };
        rows.push(lock.into_any_element());
        let sel = self.select(
            "sc.placement",
            ScalePlacement::ALL.iter().map(|v| (*v, v.label())),
            sc.placement,
            |s, v| s.scales.placement = v,
            cx,
        );
        rows.push(
            Self::row("Scales placement")
                .control(sel)
                .into_any_element(),
        );

        rows.push(section("Price labels & lines"));
        rows.push(
            check!(
                self,
                cx,
                "sc.overlap",
                "No overlapping labels",
                scales.no_overlapping_labels
            )
            .into_any_element(),
        );
        rows.push(
            check!(self, cx, "sc.plus", "Plus button", scales.plus_button).into_any_element(),
        );
        rows.push(
            check!(
                self,
                cx,
                "sc.countdown",
                "Countdown to bar close",
                scales.countdown
            )
            .into_any_element(),
        );
        let mode = self
            .select(
                "sc.symbol",
                LabelMode::ALL.iter().map(|m| (*m, m.label())),
                sc.symbol_label,
                |s, v| s.scales.symbol_label = v,
                cx,
            )
            .width(W_SELECT_SM + tokens::S4);
        let value_mode = self
            .select(
                "sc.symbol_value",
                LastValueMode::ALL.iter().map(|m| (*m, m.label())),
                sc.symbol_value_mode,
                |s, v| s.scales.symbol_value_mode = v,
                cx,
            )
            .width(W_SELECT_WIDE);
        rows.push(Self::row("Symbol").control(mode).into_any_element());
        rows.push(
            Self::row("Symbol last value")
                .indent()
                .control(value_mode)
                .into_any_element(),
        );
        let color = color!(
            self,
            window,
            cx,
            "sc.price_line",
            scales.price_line_color,
            |_, t| t.up
        );
        let width = self.width(
            "sc.price_line_w",
            sc.price_line_width,
            |s, v| s.scales.price_line_width = v,
            cx,
        );
        rows.push(
            Self::row("Symbol price line")
                .indent()
                .control(color)
                .control(width)
                .into_any_element(),
        );
        rows.push(self.line_label(
            "sc.prev_close",
            "Previous day close",
            |s| &s.scales.prev_close,
            |s| &mut s.scales.prev_close,
            |_, _| Color32::from_rgb(0x55, 0x55, 0x55),
            window,
            cx,
        ));
        let ind = self
            .select(
                "sc.indicators",
                INDICATOR_MODES.iter().map(|m| (*m, m.label())),
                sc.indicators,
                |s, v| s.scales.indicators = v,
                cx,
            )
            .width(W_SELECT_SM + tokens::S4);
        rows.push(
            Self::row("Indicators and financials")
                .control(ind)
                .into_any_element(),
        );
        rows.push(self.line_label(
            "sc.high_low",
            "High and low",
            |s| &s.scales.high_low,
            |s| &mut s.scales.high_low,
            |_, t| t.text_muted,
            window,
            cx,
        ));
        rows.push(self.line_label(
            "sc.avg_close",
            "Average close price",
            |s| &s.scales.avg_close,
            |s| &mut s.scales.avg_close,
            |_, t| t.text_muted,
            window,
            cx,
        ));
        // Bid and ask: mode + both colors, then the line style.
        let ba = sc.bid_ask;
        let mode = self
            .select(
                "sc.bid_ask.mode",
                LINE_LABEL_MODES.iter().map(|m| (*m, m.label())),
                LabelMode::from_value_line(ba.label, ba.line),
                |s, m| {
                    s.scales.bid_ask.label = m.shows_value();
                    s.scales.bid_ask.line = m.shows_line();
                },
                cx,
            )
            .width(W_SELECT_SM + tokens::S4);
        let bid = color!(self, window, cx, "sc.bid", scales.bid_ask.bid, |_, t| {
            t.series_palette[0]
        });
        let ask = color!(self, window, cx, "sc.ask", scales.bid_ask.ask, |_, _| {
            Color32::from_rgb(0xf7, 0x52, 0x5f)
        });
        rows.push(
            Self::row("Bid and ask")
                .control(mode)
                .control(bid)
                .control(ask)
                .into_any_element(),
        );
        let kind = self.line_kind(
            "sc.bid_ask.kind",
            ba.kind,
            |s, v| s.scales.bid_ask.kind = v,
            cx,
        );
        let width = self.width(
            "sc.bid_ask.width",
            ba.width,
            |s, v| s.scales.bid_ask.width = v,
            cx,
        );
        rows.push(
            Self::row("Bid and ask lines")
                .indent()
                .control(kind)
                .control(width)
                .into_any_element(),
        );

        rows.push(section("Time scale"));
        rows.push(
            check!(
                self,
                cx,
                "sc.weekday",
                "Day of week on labels",
                scales.day_of_week
            )
            .into_any_element(),
        );
        let sel = self.select(
            "sc.date_format",
            DateFormat::ALL.iter().map(|v| (*v, v.label())),
            sc.date_format,
            |s, v| s.scales.date_format = v,
            cx,
        );
        rows.push(Self::row("Date format").control(sel).into_any_element());
        let sel = self.select(
            "sc.hours",
            HoursFormat::ALL.iter().map(|v| (*v, v.label())),
            sc.hours_format,
            |s, v| s.scales.hours_format = v,
            cx,
        );
        rows.push(
            Self::row("Time hours format")
                .control(sel)
                .into_any_element(),
        );
        rows.push(
            check!(
                self,
                cx,
                "sc.left_edge",
                "Save chart left edge position when changing interval",
                scales.keep_left_edge
            )
            .into_any_element(),
        );
        rows
    }
}
