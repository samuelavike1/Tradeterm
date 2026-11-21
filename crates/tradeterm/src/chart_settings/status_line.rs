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
                d.status_title,
                |s, v| s.status_title = v,
                cx,
            )))
            .child(row(self.check(
                "st-values",
                "Chart values",
                d.status_values,
                |s, v| s.status_values = v,
                cx,
            )))
            .child(row(self.check(
                "st-change",
                "Bar change values",
                d.status_change,
                |s, v| s.status_change = v,
                cx,
            )))
            .child(row(self.check(
                "st-volume",
                "Volume",
                d.status_volume,
                |s, v| s.status_volume = v,
                cx,
            )))
            .child(section("INDICATORS", muted))
            .child(row(self.check(
                "lg-titles",
                "Titles",
                d.legend_titles,
                |s, v| s.legend_titles = v,
                cx,
            )))
            .child(row(self.check(
                "lg-inputs",
                "Inputs",
                d.legend_inputs,
                |s, v| s.legend_inputs = v,
                cx,
            )))
            .child(row(self.check(
                "lg-values",
                "Values",
                d.legend_values,
                |s, v| s.legend_values = v,
                cx,
            )))
    }
}
