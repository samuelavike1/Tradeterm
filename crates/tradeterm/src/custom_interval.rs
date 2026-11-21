//! "Add custom interval" dialog: a number and a unit (segmented control),
//! footer Cancel / Add. Enter adds; an unsupported interval shows an error
//! and keeps the dialog open.

use crate::app::TradeTerm;
use gpui_kit::component::button::{Toggle, ToggleGroup};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{Sizable as _, Size, v_flex};
use gpui_kit::*;
use tt_core::{TimeUnit, Timeframe};
use tt_ui::prelude::*;
use tt_ui::{DialogOpts, hsla};

pub struct CustomInterval {
    app: WeakEntity<TradeTerm>,
    input: Entity<InputState>,
    unit: TimeUnit,
    error: Option<SharedString>,
    _sub: Subscription,
}

const UNITS: [(TimeUnit, &str); 6] = [
    (TimeUnit::Second, "Seconds"),
    (TimeUnit::Minute, "Minutes"),
    (TimeUnit::Hour, "Hours"),
    (TimeUnit::Day, "Days"),
    (TimeUnit::Week, "Weeks"),
    (TimeUnit::Month, "Months"),
];

const SIZE: DialogSize = DialogSize::Sm;

impl CustomInterval {
    pub fn new(app: WeakEntity<TradeTerm>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .step(1.)
                .min(1.)
                .max(1000.)
                .default_value("2")
        });
        // Editing clears a previous error.
        let _sub = cx.subscribe(&input, |this, _, ev: &InputEvent, cx| {
            if let InputEvent::Change = ev
                && this.error.take().is_some()
            {
                cx.notify();
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

    /// Opens the dialog (Sm, footer Cancel / Add) with the number focused.
    pub fn open(panel: Entity<Self>, window: &mut Window, cx: &mut App) {
        let focus = panel.read(cx).input.read(cx).focus_handle(cx);
        let p = panel.downgrade();
        let footer = DialogFooter::new()
            .cancel()
            .ok("Add")
            .on_ok(move |_, cx| p.update(cx, |p, cx| p.add(cx)).unwrap_or(true));
        tt_ui::open_dialog(
            window,
            cx,
            DialogOpts::new("Add custom interval", SIZE)
                .footer(footer)
                .on_open(move |w, cx| focus.focus(w, cx)),
            panel,
        );
    }

    /// Validates and adds. Returns true to close the dialog.
    fn add(&mut self, cx: &mut Context<Self>) -> bool {
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
            return false;
        }
        let tf = Timeframe::new(self.unit, count);
        let app = self.app.clone();
        cx.defer(move |cx| {
            app.update(cx, |a, cx| a.add_custom_timeframe(tf, cx)).ok();
        });
        true
    }
}

impl Render for CustomInterval {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ui().clone();
        let units = ToggleGroup::new("interval-unit")
            .segmented()
            .single()
            .with_size(Size::Small)
            .children(UNITS.iter().map(|(unit, label)| {
                Toggle::new(*label)
                    .label(*label)
                    .checked(*unit == self.unit)
            }))
            .on_click(cx.listener(|this, checked: &Vec<bool>, _, cx| {
                // A click on the selected unit clears the group: keep it.
                if let Some(i) = checked.iter().position(|c| *c) {
                    this.unit = UNITS[i].0;
                    this.error = None;
                }
                cx.notify();
            }));
        v_flex()
            .gap(px(tokens::S2))
            .child(
                FormRow::new("Interval")
                    .label_width(SIZE.label_width() / 2.0)
                    .control(NumberField::new(&self.input)),
            )
            .child(
                FormRow::new("Unit")
                    .label_width(SIZE.label_width() / 2.0)
                    .control(units),
            )
            .children(self.error.clone().map(|e| {
                div()
                    .text_size(px(tokens::TEXT_SMALL))
                    .text_color(hsla(t.danger))
                    .child(e)
            }))
    }
}
