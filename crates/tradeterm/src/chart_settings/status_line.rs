//! The Status line page: instrument readout and indicator legends.

use super::*;
use tt_chart::style::TitleFormat;

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
            .width(tokens::W_FIELD_SELECT + tokens::S6 * 2.0);
        let mut rows = vec![
            section("Symbol"),
            check!(self, cx, "st.logo", "Logo", status_line.logo).into_any_element(),
            check!(self, cx, "st.title", "Title", status_line.title)
                .control(format)
                .into_any_element(),
            check!(
                self,
                cx,
                "st.interval",
                "Interval",
                status_line.show_interval
            )
            .indent()
            .into_any_element(),
            check!(
                self,
                cx,
                "st.exchange",
                "Exchange",
                status_line.show_exchange
            )
            .indent()
            .into_any_element(),
            check!(
                self,
                cx,
                "st.market",
                "Open market status",
                status_line.market_status
            )
            .into_any_element(),
            check!(
                self,
                cx,
                "st.values",
                "Chart values",
                status_line.chart_values
            )
            .into_any_element(),
            check!(
                self,
                cx,
                "st.change",
                "Bar change values",
                status_line.bar_change
            )
            .into_any_element(),
            check!(
                self,
                cx,
                "st.day_change",
                "Last day change values",
                status_line.last_day_change
            )
            .into_any_element(),
            check!(self, cx, "st.volume", "Volume", status_line.volume).into_any_element(),
            section("Indicators"),
            check!(
                self,
                cx,
                "lg.titles",
                "Titles",
                status_line.indicator_titles
            )
            .into_any_element(),
            check!(
                self,
                cx,
                "lg.inputs",
                "Inputs",
                status_line.indicator_inputs
            )
            .into_any_element(),
            check!(
                self,
                cx,
                "lg.values",
                "Values",
                status_line.indicator_values
            )
            .into_any_element(),
        ];
        // Transparency in 10 % steps, shown as opacity (TradingView's slider).
        let opacity = 100 - sl.indicator_background_transparency.min(100);
        let sel = self
            .select(
                "lg.bg_opacity",
                (0..=10u8).map(|i| (i * 10, format!("{}%", i * 10))),
                (opacity / 10) * 10,
                |s, v| s.status_line.indicator_background_transparency = 100 - v,
                cx,
            )
            .width(W_SELECT_SM);
        rows.push(
            check!(
                self,
                cx,
                "lg.bg",
                "Background",
                status_line.indicator_background
            )
            .control(sel)
            .unit("opacity")
            .into_any_element(),
        );
        rows
    }
}
