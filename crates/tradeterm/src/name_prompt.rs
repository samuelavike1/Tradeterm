//! A small "enter a name" dialog (kit Sm dialog, footer Cancel / OK): one
//! text field, optional checkboxes, an error line. Used by the layout menu
//! (save, rename, make a copy, create new) and indicator templates (save).
//! Enter submits; an error keeps the dialog open.

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::v_flex;
use gpui_kit::*;
use std::rc::Rc;
use tt_ui::prelude::*;
use tt_ui::{DialogOpts, hsla};

/// What a submit decided.
pub enum Submit {
    /// Done: close the dialog.
    Close,
    /// Refused: show the message, keep the dialog open.
    Error(String),
    /// Ask again (e.g. "already exists, save again to replace it"): the
    /// next submit with the same name passes `confirmed = true`.
    Confirm(String),
}

/// `(name, checkbox states, confirmed)`.
type SubmitFn = Rc<dyn Fn(String, &[bool], bool, &mut Window, &mut App) -> Submit>;

pub struct NamePrompt {
    input: Entity<InputState>,
    label: SharedString,
    checks: Vec<(SharedString, bool)>,
    error: Option<SharedString>,
    /// Name (lowercase) the user was asked to confirm.
    confirming: Option<String>,
    submit: SubmitFn,
    _sub: Subscription,
}

/// Options of [`open`].
pub struct PromptOpts {
    pub title: &'static str,
    pub label: &'static str,
    pub initial: String,
    pub ok: &'static str,
    /// Checkbox labels and initial states.
    pub checks: Vec<(&'static str, bool)>,
}

/// Opens the dialog with the name selected.
pub fn open(
    window: &mut Window,
    cx: &mut App,
    opts: PromptOpts,
    submit: impl Fn(String, &[bool], bool, &mut Window, &mut App) -> Submit + 'static,
) {
    let submit: SubmitFn = Rc::new(submit);
    let prompt = cx.new(|cx| {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(opts.label)
                .default_value(opts.initial.clone())
        });
        let _sub = cx.subscribe_in(
            &input,
            window,
            |this: &mut NamePrompt, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::Change => {
                    if this.error.take().is_some() {
                        this.confirming = None;
                        cx.notify();
                    }
                }
                InputEvent::PressEnter { .. } => {
                    if this.run(window, cx) {
                        window.close_dialog(cx);
                    }
                }
                _ => {}
            },
        );
        NamePrompt {
            input,
            label: opts.label.into(),
            checks: opts
                .checks
                .iter()
                .map(|(l, on)| (SharedString::from(*l), *on))
                .collect(),
            error: None,
            confirming: None,
            submit,
            _sub,
        }
    });
    let input = prompt.read(cx).input.clone();
    let p = prompt.downgrade();
    let footer = DialogFooter::new()
        .cancel()
        .ok(opts.ok)
        .on_ok(move |w, cx| p.update(cx, |p, cx| p.run(w, cx)).unwrap_or(true));
    tt_ui::open_dialog(
        window,
        cx,
        DialogOpts::new(opts.title, DialogSize::Sm)
            .footer(footer)
            .on_open(move |w, cx| {
                input.update(cx, |s, cx| {
                    s.focus(w, cx);
                    s.select_all(w, cx);
                });
            }),
        prompt,
    );
}

use gpui_kit::component::WindowExt as _;

impl NamePrompt {
    /// Submits; true to close.
    fn run(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let name = self.input.read(cx).value().trim().to_string();
        let key = name.to_lowercase();
        let confirmed = self.confirming.as_deref() == Some(key.as_str());
        let checks: Vec<bool> = self.checks.iter().map(|(_, on)| *on).collect();
        match (self.submit)(name, &checks, confirmed, window, cx) {
            Submit::Close => true,
            Submit::Error(msg) => {
                self.error = Some(msg.into());
                self.confirming = None;
                cx.notify();
                false
            }
            Submit::Confirm(msg) => {
                self.error = Some(msg.into());
                self.confirming = Some(key);
                cx.notify();
                false
            }
        }
    }
}

impl Render for NamePrompt {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ui().clone();
        let label_w = DialogSize::Sm.label_width() / 2.0;
        let this = cx.entity().downgrade();
        v_flex()
            .gap(tokens::S2.px())
            .child(
                FormRow::new(self.label.clone())
                    .label_width(label_w)
                    .control(div().flex_1().child(Input::new(&self.input))),
            )
            .children(self.checks.iter().enumerate().map(|(i, (label, on))| {
                let this = this.clone();
                FormRow::check(("name-check", i), label.clone(), *on, move |v, _, cx| {
                    this.update(cx, |p, cx| {
                        p.checks[i].1 = v;
                        cx.notify();
                    })
                    .ok();
                })
            }))
            .children(self.error.clone().map(|e| {
                div()
                    .text_size(tokens::TEXT_SMALL.px())
                    .text_color(hsla(t.danger))
                    .child(e)
            }))
    }
}
