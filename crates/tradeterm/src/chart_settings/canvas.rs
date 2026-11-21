//! The Canvas page: background, grid, crosshair, scales text and margins.

use super::*;

impl ChartSettingsPanel {
    pub(super) fn canvas_page(
        &self,
        mut body: Div,
        d: &ChartStyle,
        muted: Hsla,
        cx: &mut Context<Self>,
    ) -> Div {
        let this = cx.entity().downgrade();
        let size = d.font_size;
        body = body
            .child(section("CHART BASIC STYLES", muted))
            .child(row("Background").child(self.picker(ColorKey::Background)))
            .child(
                row(self.check(
                    "grid-v",
                    "Vertical grid lines",
                    d.grid_vertical,
                    |s, v| s.grid_vertical = v,
                    cx,
                ))
                .child(self.picker(ColorKey::GridV)),
            )
            .child(
                row(self.check(
                    "grid-h",
                    "Horizontal grid lines",
                    d.grid_horizontal,
                    |s, v| s.grid_horizontal = v,
                    cx,
                ))
                .child(self.picker(ColorKey::GridH)),
            )
            .child(row("Crosshair").child(self.picker(ColorKey::Crosshair)))
            .child(section("SCALES", muted))
            .child(
                row("Text").child(self.picker(ColorKey::ScaleText)).child(
                    Button::new("font-size")
                        .outline()
                        .small()
                        .label(format!("{size}"))
                        .dropdown_caret(true)
                        .dropdown_menu(move |mut menu, _, _| {
                            for s in FONT_SIZES {
                                let this = this.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(format!("{s}"))
                                        .checked(s == size)
                                        .on_click(move |_, _, cx| {
                                            this.update(cx, |p, cx| {
                                                p.edit(cx, |st| st.font_size = s)
                                            })
                                            .ok();
                                        }),
                                );
                            }
                            menu
                        }),
                ),
            )
            .child(row("Lines").child(self.picker(ColorKey::ScaleLines)))
            .child(section("MARGINS", muted));
        for ((label, unit, ..), input) in NUMBERS.iter().zip(&self.numbers) {
            body = body.child(
                row(*label)
                    .child(div().w(px(130.)).child(NumberInput::new(input).small()))
                    .child(div().text_color(muted).child(*unit)),
            );
        }
        body
    }
}
