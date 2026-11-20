//! "Chart colors" dialog: rising / falling bar colors, applied live.

use crate::app::TradeTerm;
use gpui_kit::component::button::*;
use gpui_kit::component::color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use tt_chart_gpui::hsla;
use tt_theme::Theme;

pub struct ChartColors {
    app: WeakEntity<TradeTerm>,
    up: Entity<ColorPickerState>,
    down: Entity<ColorPickerState>,
    _subs: Vec<Subscription>,
}

/// Picked color as an egui color (the chart's color type).
pub fn color32(c: Hsla) -> egui::Color32 {
    let r = Rgba::from(c);
    egui::Color32::from_rgba_unmultiplied(
        (r.r * 255.0).round() as u8,
        (r.g * 255.0).round() as u8,
        (r.b * 255.0).round() as u8,
        255,
    )
}

impl ChartColors {
    pub fn new(
        app: WeakEntity<TradeTerm>,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let up = cx.new(|cx| ColorPickerState::new(window, cx).default_value(hsla(theme.up)));
        let down = cx.new(|cx| ColorPickerState::new(window, cx).default_value(hsla(theme.down)));
        let subs = vec![
            cx.subscribe(&up, |this, _, ev: &ColorPickerEvent, cx| {
                if let ColorPickerEvent::Change(Some(c)) = ev {
                    let c = color32(*c);
                    this.set(cx, move |o| o.up = Some(c));
                }
            }),
            cx.subscribe(&down, |this, _, ev: &ColorPickerEvent, cx| {
                if let ColorPickerEvent::Change(Some(c)) = ev {
                    let c = color32(*c);
                    this.set(cx, move |o| o.down = Some(c));
                }
            }),
        ];
        Self {
            app,
            up,
            down,
            _subs: subs,
        }
    }

    fn set(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut tt_theme::ThemeOverrides) + 'static,
    ) {
        let app = self.app.clone();
        cx.defer(move |cx| {
            app.update(cx, |a, cx| a.edit_overrides(f, cx)).ok();
        });
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app = self.app.clone();
        let (up, down) = (self.up.clone(), self.down.clone());
        app.update(cx, |a, cx| {
            a.edit_overrides(|o| *o = tt_theme::ThemeOverrides::default(), cx);
            let t = a.theme().clone();
            up.update(cx, |s, cx| s.set_value(hsla(t.up), window, cx));
            down.update(cx, |s, cx| s.set_value(hsla(t.down), window, cx));
        })
        .ok();
    }
}

impl Render for ChartColors {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let row = |label: &'static str, picker: &Entity<ColorPickerState>| {
            h_flex()
                .h(px(40.))
                .gap_4()
                .child(div().w(px(120.)).child(label))
                .child(ColorPicker::new(picker))
        };
        v_flex()
            .gap_2()
            .child(row("Rising bars", &self.up))
            .child(row("Falling bars", &self.down))
            .child(
                h_flex()
                    .pt_2()
                    .gap_2()
                    .child(
                        Button::new("reset")
                            .outline()
                            .label("Reset to theme")
                            .on_click(cx.listener(|this, _, window, cx| this.reset(window, cx))),
                    )
                    .child(div().flex_1())
                    .child(Button::new("done").primary().label("Done").on_click(
                        |_, window, cx| {
                            use gpui_kit::component::WindowExt as _;
                            window.close_dialog(cx)
                        },
                    )),
            )
    }
}
