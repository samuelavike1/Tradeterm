//! "Add custom interval" dialog body: a number and a unit.

use crate::app::TradeTerm;
use gpui_kit::component::button::*;
use gpui_kit::component::input::{InputEvent, InputState, NumberInput};
use gpui_kit::component::{Sizable, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use tt_core::{TimeUnit, Timeframe};

/// "Add custom interval": a number and a unit.
pub struct CustomInterval {
    app: WeakEntity<TradeTerm>,
    input: Entity<InputState>,
    unit: TimeUnit,
    error: Option<String>,
    _sub: Subscription,
}

const UNITS: [(TimeUnit, &str); 6] = [
    (TimeUnit::Second, "seconds"),
    (TimeUnit::Minute, "minutes"),
    (TimeUnit::Hour, "hours"),
    (TimeUnit::Day, "days"),
    (TimeUnit::Week, "weeks"),
    (TimeUnit::Month, "months"),
];

impl CustomInterval {
    pub fn new(app: WeakEntity<TradeTerm>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .step(1.)
                .min(1.)
                .max(1000.)
                .default_value("2")
        });
        let _sub = cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.add(window, cx);
            }
        });
        Self {
            app,
            input,
            unit: TimeUnit::Minute,
            error: None,
            _sub,
        }
    }

    pub fn focus_input(this: &Entity<Self>, window: &mut Window, cx: &mut App) {
        let input = this.read(cx).input.clone();
        let handle = input.read(cx).focus_handle(cx);
        handle.focus(window, cx);
    }

    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let count = self
            .input
            .read(cx)
            .value()
            .trim()
            .parse::<u32>()
            .unwrap_or(0);
        if !Timeframe::is_valid_custom(self.unit, count) {
            self.error = Some("Not a supported interval for this unit.".into());
            cx.notify();
            return;
        }
        let tf = Timeframe::new(self.unit, count);
        let app = self.app.clone();
        cx.defer(move |cx| {
            app.update(cx, |a, cx| a.add_custom_timeframe(tf, cx)).ok();
        });
        window.close_dialog(cx);
    }
}

impl Render for CustomInterval {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut units = h_flex().gap_1().flex_wrap();
        for (unit, label) in UNITS {
            units = units.child(
                Button::new(label)
                    .small()
                    .label(label)
                    .when(unit == self.unit, |b| b.primary())
                    .when(unit != self.unit, |b| b.outline())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.unit = unit;
                        this.error = None;
                        cx.notify();
                    })),
            );
        }
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .gap_3()
                    .child(div().w(px(120.)).child(NumberInput::new(&self.input)))
                    .child(units),
            )
            .children(self.error.clone().map(|e| {
                div()
                    .text_size(px(12.5))
                    .text_color(gpui_kit::red())
                    .child(e)
            }))
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("cancel")
                            .outline()
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("add")
                            .primary()
                            .label("Add")
                            .on_click(cx.listener(|this, _, window, cx| this.add(window, cx))),
                    ),
            )
    }
}
