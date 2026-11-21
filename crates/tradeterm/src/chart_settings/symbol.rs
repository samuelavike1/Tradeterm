//! The Symbol page: the candle options (one block for every chart type).

use super::*;

impl ChartSettingsPanel {
    pub(super) fn symbol_page(
        &self,
        body: Div,
        d: &ChartStyle,
        muted: Hsla,
        cx: &mut Context<Self>,
    ) -> Div {
        body.child(section("CANDLES", muted))
            .child(row(self.check(
                "prev-close",
                "Color bars based on previous close",
                d.color_by_prev_close,
                |s, v| s.color_by_prev_close = v,
                cx,
            )))
            .child(
                row(self.check("body", "Body", d.body, |s, v| s.body = v, cx))
                    .child(self.picker(ColorKey::UpBody))
                    .child(self.picker(ColorKey::DownBody)),
            )
            .child(
                row(self.check("borders", "Borders", d.borders, |s, v| s.borders = v, cx))
                    .child(self.picker(ColorKey::UpBorder))
                    .child(self.picker(ColorKey::DownBorder)),
            )
            .child(
                row(self.check("wick", "Wick", d.wick, |s, v| s.wick = v, cx))
                    .child(self.picker(ColorKey::UpWick))
                    .child(self.picker(ColorKey::DownWick)),
            )
    }
}
