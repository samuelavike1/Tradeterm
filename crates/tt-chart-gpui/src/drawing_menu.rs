//! Drawing commands (undo / redo, clipboard, nudge, visual order, lock,
//! hide, remove) and the right-click context menu (spec 4.2), on
//! gpui-component's context menu for now (a later UI pass restyles it).

use crate::chart::{Chart, ChartEvent, ChartSource};
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::*;
use std::rc::Rc;
use tt_chart::{ChartData, ChartView, Drawing, DrawingMenuTarget, VisualOrder};

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
            let (target, locked, has_items) = {
                let c = chart.read(cx);
                let ds = &c.state.read(cx).source.view().drawings;
                (
                    c.menu_target,
                    ds.selected_drawings().all(|d| d.locked),
                    !ds.items.is_empty(),
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
                    menu.min_w(px(220.))
                        .item(item(
                            "Settings…",
                            Rc::new(move |_, cx| cx.emit(ChartEvent::OpenDrawingSettings(id))),
                        ))
                        .separator()
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
