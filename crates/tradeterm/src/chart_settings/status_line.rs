//! The Status line page: instrument readout and indicator legends
//! (TradingView's order; ours adds Interval / Exchange under Title).

use super::*;
use tt_chart::style::TitleFormat;

/// TradingView's legend background opacity slider.
const W_OPACITY: f32 = 148.0;

impl ChartSettingsPanel {
    pub(super) fn status_line_page(&mut self, cx: &mut Context<Self>) -> Rows {
        let sl = self.draft.status_line;
        let format = self
            .select(
                "st.title_format",
                TitleFormat::ALL.iter().map(|v| (*v, v.label())),
                sl.title_format,
                |s, v| s.status_line.title_format = v,
                cx,
            )
            .width(W_SELECT);
        let mut rows: Rows = vec![
            section("Instrument"),
            check!(self, cx, "st.logo", "Logo", status_line.logo).into(),
            check!(self, cx, "st.title", "Title", status_line.title)
                .control(format)
                .into(),
            self.sub_check(
                "st.interval",
                "Interval",
                sl.show_interval,
                |s, v| s.status_line.show_interval = v,
                cx,
            ),
            self.sub_check(
                "st.exchange",
                "Exchange",
                sl.show_exchange,
                |s, v| s.status_line.show_exchange = v,
                cx,
            ),
            check!(
                self,
                cx,
                "st.market",
                "Open market status",
                status_line.market_status
            )
            .into(),
            check!(
                self,
                cx,
                "st.values",
                "Chart values",
                status_line.chart_values
            )
            .into(),
            check!(
                self,
                cx,
                "st.change",
                "Bar change values",
                status_line.bar_change
            )
            .into(),
            check!(self, cx, "st.volume", "Volume", status_line.volume).into(),
            check!(
                self,
                cx,
                "st.day_change",
                "Last day change values",
                status_line.last_day_change
            )
            .into(),
            section("Indicators"),
            check!(
                self,
                cx,
                "lg.titles",
                "Titles",
                status_line.indicator_titles
            )
            .into(),
            self.sub_check(
                "lg.inputs",
                "Inputs",
                sl.indicator_inputs,
                |s, v| s.status_line.indicator_inputs = v,
                cx,
            ),
            check!(
                self,
                cx,
                "lg.values",
                "Values",
                status_line.indicator_values
            )
            .into(),
        ];
        // Background opacity: TradingView's slider (transparent → color).
        let t = cx.ui().clone();
        let opacity = 100 - sl.indicator_background_transparency.min(100);
        let fill = hsla(t.bg);
        let this = cx.entity().downgrade();
        let slider = RangeSlider::single("lg.bg_opacity", 0.0, 100.0, opacity as f32)
            .step(1.0)
            .width(W_OPACITY)
            .track_fill(linear_gradient(
                90.,
                linear_color_stop(tt_ui::picker::with_alpha(fill, 0.0), 0.),
                linear_color_stop(tt_ui::picker::with_alpha(hsla(t.text), 1.0), 1.),
            ))
            .on_change(move |_, v, _, cx| {
                let v = v.round().clamp(0.0, 100.0) as u8;
                this.update(cx, |p, cx| {
                    p.edit(cx, |s| {
                        s.status_line.indicator_background_transparency = 100 - v
                    })
                })
                .ok();
            });
        rows.push(
            check!(
                self,
                cx,
                "lg.bg",
                "Background",
                status_line.indicator_background
            )
            .control(slider)
            .into(),
        );
        rows
    }
}
