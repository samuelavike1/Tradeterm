//! TradingView's floating toolbar for the selected drawing (top right of
//! the price pane): color, line width, line style, lock and delete.

use crate::chart::{Chart, ChartSource};
use gpui_kit::*;
use tt_chart::LineStyle;

/// A small Lucide glyph.
fn glyph(icon: lucide_icons::Icon, size: f32) -> Div {
    div()
        .font_family(crate::ICON_FONT)
        .text_size(px(size))
        .line_height(px(size))
        .child(char::from(icon).to_string())
}

impl<S: ChartSource> Chart<S> {
    /// TradingView's floating toolbar for the selected drawing: color,
    /// line width, line style, lock and delete.
    pub(crate) fn drawing_toolbar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        use gpui_kit::component::Sizable;
        use gpui_kit::component::button::*;
        use gpui_kit::component::menu::*;
        use gpui_kit::component::popover::Popover;
        use lucide_icons::Icon;

        let state = self.state.read(cx);
        let view = state.source.view();
        let ds = &view.drawings;
        if ds.busy() || ds.hidden {
            return None;
        }
        let d = ds.selected_drawing()?.clone();
        let pane = *view.last_layout().panes.first()?;
        let origin = view.last_layout().plot.min;
        let theme = state.theme.clone();
        let this = cx.entity().downgrade();

        let swatch = |c: ecolor::Color32| {
            div()
                .size(px(14.))
                .rounded(px(3.))
                .border_1()
                .border_color(crate::hsla(theme.border))
                .bg(crate::hsla(c))
        };
        let t_color = this.clone();
        let color = Popover::new("drawing-color")
            .trigger(
                Button::new("drawing-color-btn")
                    .ghost()
                    .xsmall()
                    .child(swatch(d.color()))
                    .tooltip("Color"),
            )
            .content(move |_, _, cx| {
                let pop = cx.entity();
                let mut grid = div().flex().flex_wrap().gap_1().w(px(6. * 26.));
                for (k, c) in tt_chart::drawings::PALETTE.iter().enumerate() {
                    let (t, pop, c) = (t_color.clone(), pop.clone(), *c);
                    grid = grid.child(
                        div()
                            .id(("drawing-swatch", k))
                            .size(px(22.))
                            .rounded(px(4.))
                            .cursor_pointer()
                            .bg(crate::hsla(c))
                            .hover(|d| d.opacity(0.8))
                            .on_click(move |_, window, cx| {
                                t.update(cx, |chart, cx| {
                                    chart.edit_drawing(cx, |d| d.set_color(c))
                                })
                                .ok();
                                pop.update(cx, |s, cx| s.dismiss(window, cx));
                            }),
                    );
                }
                grid.into_any_element()
            });

        let t_width = this.clone();
        let width_now = d.width();
        let width = Button::new("drawing-width")
            .ghost()
            .xsmall()
            .child(format!("{}px", width_now as u32))
            .tooltip("Line width")
            .dropdown_menu(move |mut menu, _, _| {
                for w in [1.0f32, 2.0, 3.0, 4.0] {
                    let t = t_width.clone();
                    menu = menu.item(
                        PopupMenuItem::new(format!("{}px", w as u32))
                            .checked(w == width_now)
                            .on_click(move |_, _, cx| {
                                t.update(cx, |chart, cx| {
                                    chart.edit_drawing(cx, |d| d.set_width(w))
                                })
                                .ok();
                            }),
                    );
                }
                menu
            });

        let t_style = this.clone();
        let style_now = d.style();
        let style = Button::new("drawing-style")
            .ghost()
            .xsmall()
            .child(style_now.label())
            .tooltip("Line style")
            .dropdown_menu(move |mut menu, _, _| {
                for st in LineStyle::ALL {
                    let t = t_style.clone();
                    menu = menu.item(
                        PopupMenuItem::new(st.label())
                            .checked(st == style_now)
                            .on_click(move |_, _, cx| {
                                t.update(cx, |chart, cx| {
                                    chart.edit_drawing(cx, |d| d.set_style(st))
                                })
                                .ok();
                            }),
                    );
                }
                menu
            });

        let locked = d.locked;
        let lock = Button::new("drawing-lock")
            .ghost()
            .xsmall()
            .child(glyph(if locked { Icon::Lock } else { Icon::LockOpen }, 14.))
            .tooltip(if locked { "Unlock" } else { "Lock" })
            .on_click(
                cx.listener(move |this, _, _, cx| this.edit_drawing(cx, |d| d.locked = !locked)),
            );
        let delete = Button::new("drawing-delete")
            .ghost()
            .xsmall()
            .child(glyph(Icon::Trash2, 14.))
            .tooltip("Remove")
            .on_click(cx.listener(|this, _, _, cx| {
                this.delete_selected_drawing(cx);
            }));

        // Top right of the price pane, left of the pane buttons (clear of
        // the OHLC readout on the left).
        let axis_w = view.last_layout().price_axis.width();
        Some(
            div()
                .absolute()
                .right(px(axis_w + 70.0))
                .top(px(pane.top() - origin.y + 6.0))
                .flex()
                .items_center()
                .gap_0p5()
                .px_1()
                .py_0p5()
                .rounded(px(6.))
                .border_1()
                .border_color(crate::hsla(theme.border))
                .bg(crate::hsla(theme.bg))
                .shadow_md()
                .text_size(px(12.))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .px_1()
                        .text_color(crate::hsla(theme.text_muted))
                        .child(d.tool.label()),
                )
                .child(color)
                .child(width)
                .child(style)
                .child(lock)
                .child(delete)
                .into_any_element(),
        )
    }
}
