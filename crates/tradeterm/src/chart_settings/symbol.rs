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
                d.shared_flag(SharedCandleFlag::ColorByPrevClose),
                |s, v| s.apply_shared_flag(SharedCandleFlag::ColorByPrevClose, v),
                cx,
            )))
            .child(
                row(self.check(
                    "body",
                    "Body",
                    d.shared_flag(SharedCandleFlag::Body),
                    |s, v| s.apply_shared_flag(SharedCandleFlag::Body, v),
                    cx,
                ))
                .child(self.picker(ColorKey::UpBody))
                .child(self.picker(ColorKey::DownBody)),
            )
            .child(
                row(self.check(
                    "borders",
                    "Borders",
                    d.shared_flag(SharedCandleFlag::Borders),
                    |s, v| s.apply_shared_flag(SharedCandleFlag::Borders, v),
                    cx,
                ))
                .child(self.picker(ColorKey::UpBorder))
                .child(self.picker(ColorKey::DownBorder)),
            )
            .child(
                row(self.check(
                    "wick",
                    "Wick",
                    d.shared_flag(SharedCandleFlag::Wick),
                    |s, v| s.apply_shared_flag(SharedCandleFlag::Wick, v),
                    cx,
                ))
                .child(self.picker(ColorKey::UpWick))
                .child(self.picker(ColorKey::DownWick)),
            )
    }
}
