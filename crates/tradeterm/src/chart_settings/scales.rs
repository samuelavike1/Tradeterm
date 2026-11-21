//! The Scales and lines page (TradingView's order; ours adds Average close
//! price and Session breaks, TradingView's Events tab).

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

fn visibility(v: Visibility3) -> (Visibility3, &'static str) {
    (v, v.label())
}

/// The mode showing exactly `name`, `value` and `line` (there is no
/// "name, line": the name wins).
fn label_mode(name: bool, value: bool, line: bool) -> LabelMode {
    match (name, value, line) {
        (true, true, true) => LabelMode::NameValueLine,
        (true, true, false) => LabelMode::NameValue,
        (true, false, _) => LabelMode::Name,
        (false, v, l) => LabelMode::from_value_line(v, l),
    }
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
        cx: &mut Context<Self>,
    ) -> Item {
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
            .width(W_SELECT_LG);
        let line = self.line(
            format!("{id}.line"),
            move |s| get(s).color,
            move |s, c| get_mut(s).color = c,
            default,
            Some((
                v.width,
                Rc::new(move |s: &mut ChartStyle, w| get_mut(s).width = w),
            )),
            None,
            cx,
        );
        Self::row(label).control(mode).control(line).into()
    }

    /// TradingView's checkable label select (Name / Value / Line items;
    /// `line`: whether the field has a line).
    fn label_select(
        &self,
        id: &'static str,
        mode: LabelMode,
        line: bool,
        set: fn(&mut ChartStyle, LabelMode),
        cx: &mut Context<Self>,
    ) -> CheckSelect {
        let mut items = vec![("Name", mode.shows_name()), ("Value", mode.shows_value())];
        if line {
            items.push(("Line", mode.shows_line()));
        }
        let this = cx.entity().downgrade();
        CheckSelect::new(ElementId::Name(id.into()), items, mode.label())
            .width(W_SELECT_LG)
            .on_toggle(move |i, on, _, cx| {
                let (mut n, mut v, mut l) =
                    (mode.shows_name(), mode.shows_value(), mode.shows_line());
                match i {
                    0 => n = on,
                    1 => v = on,
                    _ => l = on,
                }
                let m = label_mode(n, v, l && line);
                this.update(cx, |p, cx| p.edit(cx, |s| set(s, m))).ok();
            })
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
            .width(W_SELECT_LG);
        rows.push(Self::row("Currency and Unit").control(sel).into());
        let sel = self
            .select(
                "sc.buttons",
                Visibility3::ALL.map(visibility),
                sc.scale_buttons,
                |s, v| s.scales.scale_buttons = v,
                cx,
            )
            .width(W_SELECT_LG);
        rows.push(Self::row("Scale modes (A and L)").control(sel).into());
        let lock = self.check(
            "sc.lock",
            "Lock price to bar ratio",
            sc.lock_ratio.is_some(),
            |s, v| {
                s.scales.lock_ratio = v.then(|| s.scales.lock_ratio.unwrap_or(1.0));
            },
            cx,
        );
        // TradingView shows the ratio always, disabled while unlocked.
        let lock = match sc.lock_ratio {
            Some(_) => {
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
                    W_FIELD,
                    window,
                    cx,
                );
                lock.control(n)
            }
            None => lock.control(disabled_field(fmt_number(1.0), W_FIELD, cx)),
        };
        rows.push(lock.into());
        let sel = self
            .select(
                "sc.placement",
                ScalePlacement::ALL.iter().map(|v| (*v, v.label())),
                sc.placement,
                |s, v| s.scales.placement = v,
                cx,
            )
            .width(W_SELECT);
        rows.push(Self::row("Scales placement").control(sel).into());

        rows.push(section("Price labels & lines"));
        rows.push(
            check!(
                self,
                cx,
                "sc.overlap",
                "No overlapping labels",
                scales.no_overlapping_labels
            )
            .into(),
        );
        rows.push(check!(self, cx, "sc.plus", "Plus button", scales.plus_button).into());
        rows.push(
            check!(
                self,
                cx,
                "sc.countdown",
                "Countdown to bar close",
                scales.countdown
            )
            .into(),
        );
        let mode = self.label_select(
            "sc.symbol",
            sc.symbol_label,
            true,
            |s, m| s.scales.symbol_label = m,
            cx,
        );
        let price_line = self.line(
            "sc.price_line",
            |s| s.scales.price_line_color,
            |s, v| s.scales.price_line_color = v,
            |_, t| t.up,
            Some((
                sc.price_line_width,
                Rc::new(|s: &mut ChartStyle, v| s.scales.price_line_width = v),
            )),
            None,
            cx,
        );
        rows.push(Self::row("Symbol").control(mode).control(price_line).into());
        let value_mode = self
            .select(
                "sc.symbol_value",
                LastValueMode::ALL.iter().map(|m| (*m, m.label())),
                sc.symbol_value_mode,
                |s, v| s.scales.symbol_value_mode = v,
                cx,
            )
            .width(W_SELECT_LG);
        rows.push(Item::Sub(true, value_mode.into_any_element()));
        rows.push(self.line_label(
            "sc.prev_close",
            "Previous day close",
            |s| &s.scales.prev_close,
            |s| &mut s.scales.prev_close,
            |_, _| Color32::from_rgb(0x55, 0x55, 0x55),
            cx,
        ));
        let ind = self.label_select(
            "sc.indicators",
            sc.indicators,
            false,
            |s, m| s.scales.indicators = m,
            cx,
        );
        rows.push(Self::row("Indicators and financials").control(ind).into());
        rows.push(self.line_label(
            "sc.high_low",
            "High and low",
            |s| &s.scales.high_low,
            |s| &mut s.scales.high_low,
            |_, t| t.text_muted,
            cx,
        ));
        rows.push(self.line_label(
            "sc.avg_close",
            "Average close price",
            |s| &s.scales.avg_close,
            |s| &mut s.scales.avg_close,
            |_, t| t.text_muted,
            cx,
        ));
        // Bid and ask: hidden until the feed has bid/ask data (BACKLOG.md).

        rows.push(section("Time scale"));
        rows.push(
            check!(
                self,
                cx,
                "sc.weekday",
                "Day of week on labels",
                scales.day_of_week
            )
            .into(),
        );
        // Session breaks (TradingView's Events tab): day boundaries in the
        // chart's time zone, intraday charts only.
        rows.push(
            check!(
                self,
                cx,
                "sc.session_breaks",
                "Session breaks",
                events.session_breaks.visible
            )
            .into(),
        );
        let sel = self
            .select(
                "sc.date_format",
                DateFormat::ALL.iter().map(|v| (*v, v.label())),
                sc.date_format,
                |s, v| s.scales.date_format = v,
                cx,
            )
            .width(W_SELECT);
        rows.push(Self::row("Date format").control(sel).into());
        let sel = self
            .select(
                "sc.hours",
                HoursFormat::ALL.iter().map(|v| (*v, v.label())),
                sc.hours_format,
                |s, v| s.scales.hours_format = v,
                cx,
            )
            .width(W_SELECT_SM);
        rows.push(Self::row("Time hours format").control(sel).into());
        rows.push(
            check!(
                self,
                cx,
                "sc.left_edge",
                "Save chart left edge position when changing interval",
                scales.keep_left_edge
            )
            .into(),
        );
        rows
    }
}
