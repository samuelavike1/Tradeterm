//! "Add custom interval" dialog, laid out like TradingView's
//! (docs/specs/chart-settings-tv-ref.md, comfortable density): Type (unit
//! select) and Interval (number), footer Cancel / Add. Enter adds; an
//! unsupported interval shows an error and keeps the dialog open.

use crate::app::TradeTerm;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::v_flex;
use gpui_kit::*;
use tt_core::{TimeUnit, Timeframe};
use tt_ui::prelude::*;
use tt_ui::tokens::{C_PAD_X, C_PAD_Y, S1};
use tt_ui::{DialogOpts, hsla};

pub struct CustomInterval {
    app: WeakEntity<TradeTerm>,
    input: Entity<InputState>,
    unit: TimeUnit,
    error: Option<SharedString>,
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

/// TradingView's dialog width, label column and control width.
const W_DIALOG: f32 = 400.0;
const W_LABEL: f32 = 107.0;
const W_CONTROL: f32 = 180.0;

impl CustomInterval {
    pub fn new(app: WeakEntity<TradeTerm>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).default_value("2"));
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

    /// Opens the dialog (footer Cancel / Add) with the number focused.
    pub fn open(panel: Entity<Self>, window: &mut Window, cx: &mut App) {
        let focus = panel.read(cx).input.read(cx).focus_handle(cx);
        let p = panel.downgrade();
        let footer = DialogFooter::new()
            .comfortable()
            .cancel()
            .ok("Add")
            .on_ok(move |_, cx| p.update(cx, |p, cx| p.add(cx)).unwrap_or(true));
        tt_ui::open_dialog(
            window,
            cx,
            DialogOpts::new("Add custom interval", DialogSize::Sm)
                .comfortable()
                .width(W_DIALOG)
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
        let this = cx.entity().downgrade();
        let unit = Select::new("interval-unit", UNITS, self.unit)
            .comfortable()
            .width(W_CONTROL)
            .on_change(move |u, _, cx| {
                this.update(cx, |p, cx| {
                    p.unit = u;
                    p.error = None;
                    cx.notify();
                })
                .ok();
            });
        v_flex()
            .px(px(C_PAD_X))
            .pt(px(C_PAD_Y - S1 * 2.0))
            .pb(px(S1))
            .child(
                FormRow::new("Type")
                    .comfortable()
                    .label_width(W_LABEL)
                    .control(unit),
            )
            .child(
                FormRow::new("Interval")
                    .comfortable()
                    .label_width(W_LABEL)
                    .control(Field::new(&self.input).width(W_CONTROL)),
            )
            .children(self.error.clone().map(|e| {
                div()
                    .pl(px(W_LABEL))
                    .text_size(px(tokens::TEXT_SMALL))
                    .text_color(hsla(t.danger))
                    .child(e)
            }))
    }
}
