//! Drawing commands (undo / redo, clipboard, nudge, visual order, lock,
//! hide, remove) and the right-click context menu (spec 4.2), on
//! gpui-component's context menu for now (a later UI pass restyles it).

use crate::chart::{Chart, ChartEvent, ChartSource};
use crate::drawing_prefs::{DrawingPrefs, prompt_template_name};
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::*;
use std::rc::Rc;
use tt_chart::{
    ChartData, ChartView, Drawing, DrawingMenuTarget, DrawingTool, Props, VisibilityPreset,
    VisualOrder,
};

/// The context menu's "Template ▸" (spec 4.2 / 5.1); deleting named
/// templates is in the toolbar's and the dialog's template menus.
fn template_submenu<S: ChartSource>(
    mut m: PopupMenu,
    tool: DrawingTool,
    t: WeakEntity<Chart<S>>,
    cx: &App,
) -> PopupMenu {
    let current = {
        let t = t.clone();
        move |cx: &App| {
            let c = t.upgrade()?;
            let ds = &c.read(cx).state.read(cx).source.view().drawings;
            ds.selected_drawing().map(|d| d.props.clone())
        }
    };
    let apply = |t: WeakEntity<Chart<S>>, props: Props, cx: &mut App| {
        t.update(cx, |c, cx| {
            c.toolbar_edit(
                0,
                |d| {
                    d.apply_style(&props);
                },
                cx,
            )
        })
        .ok();
    };
    let cur = current.clone();
    m = m.item(
        PopupMenuItem::new("Save drawing template as…").on_click(move |_, w, cx| {
            if let Some(p) = cur(cx) {
                prompt_template_name(tool, p, w, cx);
            }
        }),
    );
    let cur = current.clone();
    m = m.item(PopupMenuItem::new("Save as default").on_click(move |_, _, cx| {
        if let Some(p) = cur(cx) {
            DrawingPrefs::update(cx, |pr| pr.templates.set_default(tool, &p));
        }
    }));
    let tt = t.clone();
    m = m.item(
        PopupMenuItem::new("Apply default drawing template").on_click(move |_, _, cx| {
            let p = DrawingPrefs::get(cx).templates.new_drawing_props(tool);
            apply(tt.clone(), p, cx);
        }),
    );
    let tt = t.clone();
    m = m.item(
        PopupMenuItem::new("Reset to factory defaults").on_click(move |_, _, cx| {
            DrawingPrefs::update(cx, |pr| pr.templates.clear_default(tool));
            apply(tt.clone(), Props::defaults(tool), cx);
        }),
    );
    let prefs = DrawingPrefs::get(cx);
    let named = prefs.templates.named(tool);
    if !named.is_empty() {
        m = m.separator();
        for n in named {
            let (tt, name) = (t.clone(), n.name.clone());
            m = m.item(PopupMenuItem::new(n.name.clone()).on_click(move |_, _, cx| {
                if let Some(p) = DrawingPrefs::get(cx).templates.named_props(tool, &name) {
                    apply(tt.clone(), p, cx);
                }
            }));
        }
    }
    m
}

/// The app-wide drawing clipboard (Ctrl + C / Ctrl + V), shared by every
/// chart, tab and symbol.
#[derive(Default)]
pub struct DrawingClipboard(pub Vec<Drawing>);

impl Global for DrawingClipboard {}

impl DrawingClipboard {
    pub fn has_drawings(cx: &App) -> bool {
        cx.try_global::<Self>().is_some_and(|c| !c.0.is_empty())
    }
}

type Command<S> = Rc<dyn Fn(&mut Chart<S>, &mut Context<Chart<S>>)>;

impl<S: ChartSource> Chart<S> {
    /// Runs a drawing command with the view and the data; repaints and
    /// reports the change when it returns true.
    fn drawing_command(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut ChartView, ChartData<'_>) -> bool,
    ) -> bool {
        let done = self.state.update(cx, |s, _| {
            let (v, d) = s.source.parts();
            f(v, d)
        });
        if done {
            self.invalidate(cx);
            cx.emit(ChartEvent::DrawingsChanged);
            cx.notify();
        }
        done
    }

    /// Ctrl + Z.
    pub fn undo_drawing(&mut self, cx: &mut Context<Self>) -> bool {
        self.drawing_command(cx, |v, _| v.drawings.undo())
    }

    /// Ctrl + Y / Ctrl + Shift + Z.
    pub fn redo_drawing(&mut self, cx: &mut Context<Self>) -> bool {
        self.drawing_command(cx, |v, _| v.drawings.redo())
    }

    /// Ctrl + C: the selected drawings to the app clipboard.
    pub fn copy_drawings(&mut self, cx: &mut Context<Self>) -> bool {
        let items = self.state.read(cx).source.view().drawings.copy_selected();
        if items.is_empty() {
            return false;
        }
        cx.set_global(DrawingClipboard(items));
        true
    }

    /// Ctrl + V: pastes the app clipboard at the same anchors (offset when
    /// that would cover a copy exactly) and selects the copies.
    pub fn paste_drawings(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(items) = cx
            .try_global::<DrawingClipboard>()
            .filter(|c| !c.0.is_empty())
            .map(|c| c.0.clone())
        else {
            return false;
        };
        self.drawing_command(cx, |v, d| v.paste_drawings(&items, d))
    }

    /// Arrow keys: `bars` bars and `ticks` price ticks.
    pub fn nudge_drawings(&mut self, bars: i32, ticks: i32, cx: &mut Context<Self>) -> bool {
        self.drawing_command(cx, |v, d| v.nudge_drawings(bars, ticks, d))
    }

    pub fn clone_drawings(&mut self, cx: &mut Context<Self>) -> bool {
        self.drawing_command(cx, |v, d| v.clone_selected_drawings(d))
    }

    pub fn reorder_drawings(&mut self, op: VisualOrder, cx: &mut Context<Self>) -> bool {
        self.drawing_command(cx, |v, _| v.drawings.reorder_selected(op))
    }

    /// Locks the selection, or unlocks it when all of it is locked.
    pub fn toggle_drawings_locked(&mut self, cx: &mut Context<Self>) -> bool {
        self.drawing_command(cx, |v, _| {
            let lock = !v.drawings.selected_drawings().all(|d| d.locked);
            v.drawings.edit_selected(|d| d.locked = lock)
        })
    }

    /// Hides the selected drawings (undo shows them again).
    pub fn hide_drawings(&mut self, cx: &mut Context<Self>) -> bool {
        self.drawing_command(cx, |v, _| {
            let done = v.drawings.edit_selected(|d| d.hidden = true);
            v.drawings.clear_selection();
            done
        })
    }

    pub fn remove_all_drawings(&mut self, cx: &mut Context<Self>) -> bool {
        self.drawing_command(cx, |v, _| v.drawings.remove_all())
    }

    /// Right button: picks what the context menu is for (selecting the
    /// drawing under the pointer), or finishes a path / polyline.
    pub(crate) fn right_mouse_down(&mut self, e: &MouseDownEvent, cx: &mut Context<Self>) {
        let p = crate::chart::pos(e.position);
        let target = self
            .state
            .update(cx, |s, _| s.source.parts().0.drawing_context_press(p));
        self.menu_target = target;
        match target {
            DrawingMenuTarget::Finished => {
                self.invalidate(cx);
                cx.emit(ChartEvent::DrawingsChanged);
                cx.notify();
            }
            DrawingMenuTarget::Drawing(_) => {
                self.invalidate(cx);
                cx.notify();
            }
            DrawingMenuTarget::None | DrawingMenuTarget::Empty => {}
        }
    }

    /// The context menu builder: built when the menu opens, after
    /// [`Self::right_mouse_down`] picked the target (an empty menu does not
    /// open).
    pub(crate) fn drawing_menu(
        &self,
        cx: &mut Context<Self>,
    ) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
        let this = cx.entity().downgrade();
        move |menu, window, cx| {
            let Some(chart) = this.upgrade() else {
                return menu;
            };
            let (target, locked, has_items, same_tool) = {
                let c = chart.read(cx);
                let ds = &c.state.read(cx).source.view().drawings;
                let first = ds.selected_drawing().map(|d| d.tool);
                (
                    c.menu_target,
                    ds.selected_drawings().all(|d| d.locked),
                    !ds.items.is_empty(),
                    first.filter(|t| ds.selected_drawings().all(|d| d.tool == *t)),
                )
            };
            let item = |label: &'static str, f: Command<S>| {
                let t = this.clone();
                PopupMenuItem::new(label).on_click(move |_, _, cx| {
                    let f = f.clone();
                    t.update(cx, |c, cx| f(c, cx)).ok();
                })
            };
            match target {
                DrawingMenuTarget::Drawing(id) => {
                    let t = this.clone();
                    let mut menu = menu
                        .min_w(px(220.))
                        .item(item(
                            "Settings…",
                            Rc::new(move |_, cx| cx.emit(ChartEvent::OpenDrawingSettings(id))),
                        ))
                        .separator();
                    if let Some(tool) = same_tool {
                        let t = this.clone();
                        menu = menu.submenu("Template", window, cx, move |m, _, cx| {
                            template_submenu(m, tool, t.clone(), cx)
                        });
                    }
                    let tv = this.clone();
                    menu.submenu("Visibility on intervals", window, cx, move |mut m, _, _| {
                        for p in VisibilityPreset::ALL {
                            let t = tv.clone();
                            m = m.item(PopupMenuItem::new(p.label()).on_click(move |_, _, cx| {
                                t.update(cx, |c, cx| c.set_drawings_visibility(p, cx)).ok();
                            }));
                        }
                        m
                    })
                    .submenu("Visual order", window, cx, move |mut m, _, _| {
                            for op in VisualOrder::ALL {
                                let t = t.clone();
                                m = m.item(PopupMenuItem::new(op.label()).on_click(
                                    move |_, _, cx| {
                                        t.update(cx, |c, cx| c.reorder_drawings(op, cx)).ok();
                                    },
                                ));
                            }
                            m
                        })
                        .separator()
                        .item(item(
                            "Clone",
                            Rc::new(|c, cx| {
                                c.clone_drawings(cx);
                            }),
                        ))
                        .item(item(
                            "Copy",
                            Rc::new(|c, cx| {
                                c.copy_drawings(cx);
                            }),
                        ))
                        .separator()
                        .item(item(
                            if locked { "Unlock" } else { "Lock" },
                            Rc::new(|c, cx| {
                                c.toggle_drawings_locked(cx);
                            }),
                        ))
                        .item(item(
                            "Hide",
                            Rc::new(|c, cx| {
                                c.hide_drawings(cx);
                            }),
                        ))
                        .item(item(
                            "Remove",
                            Rc::new(|c, cx| {
                                c.delete_selected_drawing(cx);
                            }),
                        ))
                }
                DrawingMenuTarget::Empty => {
                    let mut menu = menu.min_w(px(220.));
                    if DrawingClipboard::has_drawings(cx) {
                        menu = menu.item(item(
                            "Paste",
                            Rc::new(|c, cx| {
                                c.paste_drawings(cx);
                            }),
                        ));
                    }
                    if has_items {
                        menu = menu.item(item(
                            "Remove drawings",
                            Rc::new(|c, cx| {
                                c.remove_all_drawings(cx);
                            }),
                        ));
                    }
                    menu
                }
                DrawingMenuTarget::None | DrawingMenuTarget::Finished => menu,
            }
        }
    }
}
