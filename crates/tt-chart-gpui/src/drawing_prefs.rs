//! App-wide drawing preferences shared by every chart (drawings spec 4.1,
//! 5): the templates per tool and the floating toolbar's position, plus the
//! "Template ▾" menu used by the floating toolbar and the settings dialog.
//!
//! The app loads [`DrawingPrefs`] from its settings into a gpui global and
//! observes it: every change is saved and the tools' defaults are pushed
//! into the charts (new drawings start from them).

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{WindowExt as _, v_flex};
use gpui_kit::*;
use std::rc::Rc;
use tt_chart::drawings::DrawingTemplates;
use tt_chart::{DrawingTool, Props};
use tt_ui::prelude::*;
use tt_ui::tokens::*;
use tt_ui::{DialogFooter, DialogOpts, DialogSize, open_dialog};

/// Drawing preferences shared by every chart.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DrawingPrefs {
    pub templates: DrawingTemplates,
    /// The floating toolbar's top-left as a fraction of the chart size.
    pub toolbar_pos: Option<[f32; 2]>,
}

impl Global for DrawingPrefs {}

impl DrawingPrefs {
    /// The current preferences (defaults before the app set them).
    pub fn get(cx: &App) -> DrawingPrefs {
        cx.try_global::<Self>().cloned().unwrap_or_default()
    }

    pub fn templates(cx: &App) -> Option<&DrawingTemplates> {
        cx.try_global::<Self>().map(|p| &p.templates)
    }

    /// Changes the preferences (observers save them).
    pub fn update(cx: &mut App, f: impl FnOnce(&mut DrawingPrefs)) {
        cx.update_default_global::<Self, _>(|p, _| f(p));
    }

    /// The last-used style of `tool` (every committed style edit).
    pub fn remember_style(cx: &mut App, tool: DrawingTool, props: &Props) {
        let same = Self::templates(cx)
            .and_then(|t| t.default_props(tool))
            .is_some_and(|p| &p == props);
        if !same {
            Self::update(cx, |p| p.templates.set_default(tool, props));
        }
    }
}

/// What a template menu works on: the tool, the current style and how to
/// apply a style (to the selection or to the dialog's drawings).
#[derive(Clone)]
pub struct TemplateTarget {
    pub tool: DrawingTool,
    pub current: Rc<dyn Fn(&App) -> Option<Props>>,
    pub apply: Rc<dyn Fn(Props, &mut Window, &mut App)>,
}

/// The "Template ▾" menu (spec 5.1): save as…, save as default, apply
/// default, reset to factory defaults, then the named templates (click
/// applies, × deletes after confirmation).
pub fn template_menu(menu: Menu, target: TemplateTarget, cx: &App) -> Menu {
    let tool = target.tool;
    let prefs = DrawingPrefs::get(cx);
    let t = target.clone();
    let mut menu = menu
        .width(MenuWidth::Lg)
        .row(
            MenuRow::new("tpl-save-as", "Save drawing template as…").on_select(move |w, cx| {
                if let Some(p) = (t.current)(cx) {
                    prompt_template_name(tool, p, w, cx);
                }
            }),
        );
    let t = target.clone();
    menu = menu.row(
        MenuRow::new("tpl-save-default", "Save as default").on_select(move |_, cx| {
            if let Some(p) = (t.current)(cx) {
                DrawingPrefs::update(cx, |pr| pr.templates.set_default(tool, &p));
            }
        }),
    );
    let t = target.clone();
    menu = menu.row(
        MenuRow::new("tpl-apply-default", "Apply default drawing template")
            .description("The tool's saved default style (factory defaults if none)")
            .on_select(move |w, cx| {
                let p = DrawingPrefs::get(cx).templates.new_drawing_props(tool);
                (t.apply)(p, w, cx);
            }),
    );
    let t = target.clone();
    menu = menu.row(
        MenuRow::new("tpl-reset", "Reset to factory defaults")
            .description("TradingView's defaults; new drawings use them too")
            .on_select(move |w, cx| {
                DrawingPrefs::update(cx, |pr| pr.templates.clear_default(tool));
                (t.apply)(Props::defaults(tool), w, cx);
            }),
    );
    let named = prefs.templates.named(tool);
    if !named.is_empty() {
        menu = menu.separator().section("Templates");
        for (i, n) in named.iter().enumerate() {
            let name = n.name.clone();
            let (t, apply_name, del_name) = (target.clone(), name.clone(), name.clone());
            menu = menu.row(
                MenuRow::new(("tpl-named", i), name)
                    .removable("Delete template", move |w, cx| {
                        confirm_delete(tool, del_name.clone(), w, cx);
                    })
                    .on_select(move |w, cx| {
                        let p = DrawingPrefs::get(cx)
                            .templates
                            .named_props(tool, &apply_name);
                        if let Some(p) = p {
                            (t.apply)(p, w, cx);
                        }
                    }),
            );
        }
    }
    menu
}

/// The "Save drawing template as…" name prompt. An existing name asks to
/// replace it (the first Ok shows the question, the second replaces).
pub fn prompt_template_name(tool: DrawingTool, props: Props, window: &mut Window, cx: &mut App) {
    let input = cx.new(|cx| InputState::new(window, cx).placeholder("Template name"));
    let panel = cx.new(|_| NamePrompt {
        input: input.clone(),
        tool,
        confirm_replace: None,
    });
    let (ok_panel, focus_input) = (panel.clone(), input.clone());
    let footer = DialogFooter::new()
        .cancel()
        .ok("Save")
        .on_ok(move |_, cx| {
            let name = ok_panel.read(cx).input.read(cx).value().trim().to_owned();
            if name.is_empty() {
                return false;
            }
            let exists = DrawingPrefs::get(cx).templates.has_named(tool, &name);
            let confirmed = ok_panel.read(cx).confirm_replace.as_deref() == Some(name.as_str());
            if exists && !confirmed {
                ok_panel.update(cx, |p, cx| {
                    p.confirm_replace = Some(name);
                    cx.notify();
                });
                return false;
            }
            DrawingPrefs::update(cx, |pr| {
                pr.templates.save_named(tool, &name, &props);
            });
            true
        });
    open_dialog(
        window,
        cx,
        DialogOpts::new("Save drawing template", DialogSize::Sm)
            .footer(footer)
            .on_open(move |w, cx| {
                focus_input.update(cx, |s, cx| s.focus(w, cx));
            }),
        panel,
    );
}

struct NamePrompt {
    input: Entity<InputState>,
    tool: DrawingTool,
    /// The name the user was asked to replace.
    confirm_replace: Option<String>,
}

impl Render for NamePrompt {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ui().clone();
        let name = self.input.read(cx).value().trim().to_owned();
        let replacing = self.confirm_replace.as_deref() == Some(name.as_str());
        v_flex()
            .gap(px(S2))
            .py(px(S2))
            .child(
                div()
                    .text_size(px(TEXT_SMALL))
                    .text_color(hsla(t.text_muted))
                    .child(format!("{} template (style only)", self.tool.label())),
            )
            .child(Input::new(&self.input))
            .when(replacing, |d| {
                d.child(
                    div()
                        .text_size(px(TEXT_SMALL))
                        .text_color(hsla(t.warning))
                        .child(format!("\"{name}\" exists. Save again to replace it.")),
                )
            })
    }
}

/// Asks before deleting a named template.
fn confirm_delete(tool: DrawingTool, name: String, window: &mut Window, cx: &mut App) {
    let label = SharedString::from(format!("Delete the template \"{name}\"?"));
    let panel = cx.new(|_| Message(label));
    let footer = DialogFooter::new()
        .cancel()
        .ok("Delete")
        .on_ok(move |_, cx| {
            DrawingPrefs::update(cx, |p| {
                p.templates.remove_named(tool, &name);
            });
            true
        });
    open_dialog(
        window,
        cx,
        DialogOpts::new("Delete template", DialogSize::Sm).footer(footer),
        panel,
    );
}

struct Message(SharedString);

impl Render for Message {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().py(px(S2)).text_size(px(TEXT_BODY)).child(self.0.clone())
    }
}
