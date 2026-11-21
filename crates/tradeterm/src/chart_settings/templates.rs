//! The footer's Template ▾ menu and its small dialogs (name prompt for
//! Save as… / Rename…, delete confirmation).
//!
//! Applying a template, the user default or the factory defaults replaces
//! the dialog's draft (the chart previews it); nothing is committed until
//! Ok. Save as… / Rename / Delete write `chart-templates.json` at once.

use super::*;
use tt_session::templates::validate_name;
use tt_ui::{Menu, MenuRow};

/// The name prompt: TradingView's small comfortable dialog.
const W_PROMPT: f32 = 400.0;
const W_PROMPT_LABEL: f32 = 130.0;
const W_PROMPT_FIELD: f32 = 230.0;

/// Builds the menu each time it opens (the saved list can change).
pub(super) fn menu(panel: &Entity<ChartSettingsPanel>, menu: Menu, cx: &mut App) -> Menu {
    let app = panel.read(cx).app.clone();
    let names = app
        .upgrade()
        .map(|a| a.read(cx).templates.names())
        .unwrap_or_default();
    let p = panel.downgrade();
    let mut menu = menu
        .width(tt_ui::MenuWidth::Md)
        .row(MenuRow::new("tpl-save-as", "Save as…").on_select({
            let p = p.clone();
            move |w, cx| open_prompt(p.clone(), None, w, cx)
        }))
        .row(
            MenuRow::new("tpl-save-default", "Save as default")
                .description("New tabs, and tabs without their own settings, use these")
                .on_select({
                    let p = p.clone();
                    move |_, cx| {
                        p.update(cx, |p, cx| p.save_as_default(cx)).ok();
                    }
                }),
        )
        .separator()
        .row(
            MenuRow::new("tpl-apply-defaults", "Apply defaults")
                .description("Your default settings (Save as default)")
                .on_select({
                    let (p, app) = (p.clone(), app.clone());
                    move |w, cx| {
                        let Some(style) = app.upgrade().map(|a| a.read(cx).chart_style.clone())
                        else {
                            return;
                        };
                        p.update(cx, |p, cx| p.replace_draft(style, w, cx)).ok();
                    }
                }),
        )
        .row(MenuRow::new("tpl-factory", "Factory defaults").on_select({
            let p = p.clone();
            move |w, cx| {
                p.update(cx, |p, cx| p.replace_draft(ChartStyle::default(), w, cx))
                    .ok();
            }
        }));
    if names.is_empty() {
        return menu;
    }
    menu = menu.separator().section("Saved templates");
    for (i, name) in names.iter().enumerate() {
        let apply = {
            let (p, app, name) = (p.clone(), app.clone(), name.clone());
            move |w: &mut Window, cx: &mut App| {
                let Some(style) = app
                    .upgrade()
                    .and_then(|a| a.read(cx).templates.get(&name).map(|t| t.style.clone()))
                else {
                    return;
                };
                p.update(cx, |p, cx| p.replace_draft(style, w, cx)).ok();
            }
        };
        let delete = {
            let (app, name) = (app.clone(), name.clone());
            move |w: &mut Window, cx: &mut App| confirm_delete(app.clone(), name.clone(), w, cx)
        };
        menu = menu.row(
            MenuRow::new(("tpl-item", i), name.clone())
                .on_select(apply)
                .removable("Delete template", delete),
        );
    }
    menu.separator()
        .row(MenuRow::new("tpl-rename", "Rename template…").on_select({
            let first = names.first().cloned();
            move |w, cx| open_prompt(p.clone(), Some(first.clone().unwrap_or_default()), w, cx)
        }))
}

/// Asks before deleting a template (it can't be undone).
fn confirm_delete(app: WeakEntity<TradeTerm>, name: String, window: &mut Window, cx: &mut App) {
    let text = format!("Delete the template “{name}”? This can't be undone.");
    let body = cx.new(|_| Message(text.into()));
    let footer = DialogFooter::new()
        .comfortable()
        .cancel()
        .ok("Delete")
        .on_ok(move |_, cx| {
            app.update(cx, |a, cx| {
                a.templates.delete(&name);
                cx.notify();
            })
            .ok();
            true
        });
    tt_ui::open_dialog(
        window,
        cx,
        DialogOpts::new("Delete template", DialogSize::Sm)
            .comfortable()
            .width(W_PROMPT)
            .footer(footer),
        body,
    );
}

/// A one-paragraph dialog body.
struct Message(SharedString);

impl Render for Message {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(tokens::C_PAD_X))
            .py(px(tokens::C_PAD_Y - tokens::S2))
            .text_size(px(tokens::TEXT_BODY))
            .text_color(hsla(cx.ui().text))
            .child(self.0.clone())
    }
}

/// Name prompt: Save as… (`rename_from` = None) or Rename.
pub struct TemplatePrompt {
    panel: WeakEntity<ChartSettingsPanel>,
    app: WeakEntity<TradeTerm>,
    rename_from: Option<String>,
    names: Vec<String>,
    input: Entity<InputState>,
    error: Option<SharedString>,
    /// Lower-cased name the user was warned about; saving it again
    /// overwrites.
    confirm_overwrite: Option<String>,
    _sub: Subscription,
}

fn open_prompt(
    panel: WeakEntity<ChartSettingsPanel>,
    rename_from: Option<String>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(app) = panel.upgrade().map(|p| p.read(cx).app.clone()) else {
        return;
    };
    let names = app
        .upgrade()
        .map(|a| a.read(cx).templates.names())
        .unwrap_or_default();
    let initial = rename_from.clone().unwrap_or_default();
    let renaming = rename_from.is_some();
    let prompt = cx.new(|cx| {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Template name")
                .default_value(initial)
        });
        let _sub = cx.subscribe(
            &input,
            |this: &mut TemplatePrompt, _, ev: &InputEvent, cx| {
                if let InputEvent::Change = ev
                    && this.error.is_some()
                {
                    this.error = None;
                    this.confirm_overwrite = None;
                    cx.notify();
                }
            },
        );
        TemplatePrompt {
            panel,
            app,
            rename_from,
            names,
            input,
            error: None,
            confirm_overwrite: None,
            _sub,
        }
    });
    let focus = prompt.read(cx).input.read(cx).focus_handle(cx);
    let p = prompt.downgrade();
    let footer = DialogFooter::new()
        .comfortable()
        .cancel()
        .ok(if renaming { "Rename" } else { "Save" })
        .on_ok(move |_, cx| p.update(cx, |p, cx| p.submit(cx)).unwrap_or(true));
    let (title, size) = if renaming {
        ("Rename template", DialogSize::Sm)
    } else {
        ("Save template", DialogSize::Sm)
    };
    tt_ui::open_dialog(
        window,
        cx,
        DialogOpts::new(title, size)
            .comfortable()
            .width(W_PROMPT)
            .footer(footer)
            .on_open(move |w, cx| focus.focus(w, cx)),
        prompt,
    );
}

impl TemplatePrompt {
    /// Saves / renames. Returns true to close the dialog.
    fn submit(&mut self, cx: &mut Context<Self>) -> bool {
        let raw = self.input.read(cx).value().to_string();
        let name = match validate_name(&raw) {
            Ok(n) => n,
            Err(e) => return self.fail(e.message(), cx),
        };
        let Some(app) = self.app.upgrade() else {
            return true;
        };
        if let Some(from) = self.rename_from.clone() {
            let r = app.update(cx, |a, cx| {
                let r = a.templates.rename(&from, &name);
                cx.notify();
                r
            });
            return match r {
                Ok(_) => true,
                Err(e) => self.fail(e.message(), cx),
            };
        }
        let key = name.to_lowercase();
        let exists = app.read(cx).templates.contains(&name);
        if exists && self.confirm_overwrite.as_deref() != Some(key.as_str()) {
            self.confirm_overwrite = Some(key);
            return self.fail(
                format!("“{name}” already exists. Save again to replace it."),
                cx,
            );
        }
        let Some(style) = self.panel.upgrade().map(|p| p.read(cx).draft().clone()) else {
            return true;
        };
        app.update(cx, |a, cx| {
            if let Err(e) = a.templates.save(&name, &style) {
                tracing::warn!("could not save template: {}", e.message());
            }
            cx.notify();
        });
        true
    }

    fn fail(&mut self, msg: impl Into<SharedString>, cx: &mut Context<Self>) -> bool {
        self.error = Some(msg.into());
        cx.notify();
        false
    }
}

impl Render for TemplatePrompt {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ui().clone();
        let this = cx.entity().downgrade();
        let mut col = v_flex()
            .px(px(tokens::C_PAD_X))
            .pt(px(tokens::C_PAD_Y - tokens::S2))
            .pb(px(tokens::S1));
        if let Some(from) = self.rename_from.clone() {
            let sel = Select::new(
                "tpl-rename-from",
                self.names.iter().map(|n| (n.clone(), n.clone())),
                from,
            )
            .comfortable()
            .width(W_PROMPT_FIELD)
            .on_change(move |v: String, w, cx| {
                this.update(cx, |p, cx| {
                    p.rename_from = Some(v.clone());
                    p.input.update(cx, |s, cx| s.set_value(v, w, cx));
                    p.error = None;
                    cx.notify();
                })
                .ok();
            });
            col = col.child(
                FormRow::new("Template")
                    .comfortable()
                    .label_width(W_PROMPT_LABEL)
                    .control(sel),
            );
        }
        col.child(
            FormRow::new(if self.rename_from.is_some() {
                "New name"
            } else {
                "Template name"
            })
            .comfortable()
            .label_width(W_PROMPT_LABEL)
            .control(Field::new(&self.input).width(W_PROMPT_FIELD)),
        )
        .children(self.error.clone().map(|e| {
            div()
                .pl(px(W_PROMPT_LABEL))
                .text_size(px(tokens::TEXT_SMALL))
                .text_color(hsla(t.danger))
                .child(e)
        }))
    }
}
