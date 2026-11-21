//! The Scales and lines page.

use super::*;

impl ChartSettingsPanel {
    pub(super) fn scales_page(
        &self,
        body: Div,
        d: &ChartStyle,
        muted: Hsla,
        cx: &mut Context<Self>,
    ) -> Div {
        let this = cx.entity().downgrade();
        let current = d.scale_buttons;
        body.child(section("PRICE SCALE", muted))
            .child(
                row("Scale modes (A, L and %)").child(
                    Button::new("scale-buttons")
                        .outline()
                        .small()
                        .label(current.label())
                        .dropdown_caret(true)
                        .dropdown_menu(move |mut menu, _, _| {
                            for option in ScaleButtons::ALL {
                                let this = this.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(option.label())
                                        .checked(option == current)
                                        .on_click(move |_, _, cx| {
                                            this.update(cx, |p, cx| {
                                                p.edit(cx, |s| s.scale_buttons = option)
                                            })
                                            .ok();
                                        }),
                                );
                            }
                            menu
                        }),
                ),
            )
            .child(section("PRICE LABELS & LINES", muted))
            .child(row(self.check(
                "no-overlap",
                "No overlapping labels",
                d.no_overlapping_labels,
                |s, v| s.no_overlapping_labels = v,
                cx,
            )))
            .child(row(self.check(
                "countdown",
                "Countdown to bar close",
                d.countdown,
                |s, v| s.countdown = v,
                cx,
            )))
            .child(row(self.check(
                "last-label",
                "Symbol last price label",
                d.last_price_label,
                |s, v| s.last_price_label = v,
                cx,
            )))
            .child(row(self.check(
                "last-line",
                "Symbol price line",
                d.last_price_line,
                |s, v| s.last_price_line = v,
                cx,
            )))
            .child(row(self.check(
                "ind-labels",
                "Indicator value labels",
                d.indicator_value_labels,
                |s, v| s.indicator_value_labels = v,
                cx,
            )))
            .child(section("TIME SCALE", muted))
            .child(row(self.check(
                "weekday",
                "Day of week on labels",
                d.day_of_week,
                |s, v| s.day_of_week = v,
                cx,
            )))
    }
}
