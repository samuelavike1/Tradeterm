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
        let current = d.scales.scale_buttons;
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
                                                p.edit(cx, |s| s.scales.scale_buttons = option)
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
                d.scales.no_overlapping_labels,
                |s, v| s.scales.no_overlapping_labels = v,
                cx,
            )))
            .child(row(self.check(
                "countdown",
                "Countdown to bar close",
                d.scales.countdown,
                |s, v| s.scales.countdown = v,
                cx,
            )))
            .child(row(self.check(
                "last-label",
                "Symbol last price label",
                d.scales.symbol_label.shows_value(),
                |s, v| s.scales.symbol_label = s.scales.symbol_label.with_value(v),
                cx,
            )))
            .child(row(self.check(
                "last-line",
                "Symbol price line",
                d.scales.symbol_label.shows_line(),
                |s, v| s.scales.symbol_label = s.scales.symbol_label.with_line(v),
                cx,
            )))
            .child(row(self.check(
                "ind-labels",
                "Indicator value labels",
                d.scales.indicators.shows_value(),
                |s, v| s.scales.indicators = s.scales.indicators.with_value(v),
                cx,
            )))
            .child(section("TIME SCALE", muted))
            .child(row(self.check(
                "weekday",
                "Day of week on labels",
                d.scales.day_of_week,
                |s, v| s.scales.day_of_week = v,
                cx,
            )))
    }
}
