//! The Status line page: instrument readout and indicator legends.

use super::*;

impl ChartSettingsPanel {
    pub(super) fn status_line_page(
        &self,
        body: Div,
        d: &ChartStyle,
        muted: Hsla,
        cx: &mut Context<Self>,
    ) -> Div {
        body.child(section("INSTRUMENT", muted))
            .child(row(self.check(
                "st-title",
                "Title",
                d.status_line.title,
                |s, v| s.status_line.title = v,
                cx,
            )))
            .child(row(self.check(
                "st-values",
                "Chart values",
                d.status_line.chart_values,
                |s, v| s.status_line.chart_values = v,
                cx,
            )))
            .child(row(self.check(
                "st-change",
                "Bar change values",
                d.status_line.bar_change,
                |s, v| s.status_line.bar_change = v,
                cx,
            )))
            .child(row(self.check(
                "st-volume",
                "Volume",
                d.status_line.volume,
                |s, v| s.status_line.volume = v,
                cx,
            )))
            .child(section("INDICATORS", muted))
            .child(row(self.check(
                "lg-titles",
                "Titles",
                d.status_line.indicator_titles,
                |s, v| s.status_line.indicator_titles = v,
                cx,
            )))
            .child(row(self.check(
                "lg-inputs",
                "Inputs",
                d.status_line.indicator_inputs,
                |s, v| s.status_line.indicator_inputs = v,
                cx,
            )))
            .child(row(self.check(
                "lg-values",
                "Values",
                d.status_line.indicator_values,
                |s, v| s.status_line.indicator_values = v,
                cx,
            )))
    }
}
