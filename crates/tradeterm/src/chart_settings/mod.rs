//! Chart "Settings" dialog, TradingView-style: a sidebar (Symbol, Status
//! line, Scales and lines, Canvas) and rows of options. Changes preview live
//! on every chart; Ok keeps them, Cancel / × / Esc restore the previous
//! settings, "Reset to defaults" starts over.

use crate::app::TradeTerm;
use crate::ui::lucide;
use ecolor::Color32;
use gpui_kit::component::button::*;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{InputEvent, InputState, NumberInput};
use gpui_kit::component::menu::*;
use gpui_kit::component::{Sizable, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use tt_chart::style::{SharedCandleColor, SharedCandleFlag};
use tt_chart::{ChartStyle, ColorSlot, ScaleButtons};
use tt_chart_gpui::hsla;
use tt_theme::Theme;

mod canvas;
mod scales;
mod status_line;
mod symbol;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Symbol,
    StatusLine,
    Scales,
    Canvas,
}

impl Page {
    const ALL: [(Page, &'static str, lucide_icons::Icon); 4] = [
        (Page::Symbol, "Symbol", lucide_icons::Icon::ChartCandlestick),
        (
            Page::StatusLine,
            "Status line",
            lucide_icons::Icon::AlignLeft,
        ),
        (Page::Scales, "Scales and lines", lucide_icons::Icon::Axis3d),
        (Page::Canvas, "Canvas", lucide_icons::Icon::Paintbrush),
    ];
}

/// Every color the dialog edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorKey {
    UpBody,
    DownBody,
    UpBorder,
    DownBorder,
    UpWick,
    DownWick,
    Background,
    GridV,
    GridH,
    Crosshair,
    ScaleText,
    ScaleLines,
}

impl ColorKey {
    const ALL: [ColorKey; 12] = [
        ColorKey::UpBody,
        ColorKey::DownBody,
        ColorKey::UpBorder,
        ColorKey::DownBorder,
        ColorKey::UpWick,
        ColorKey::DownWick,
        ColorKey::Background,
        ColorKey::GridV,
        ColorKey::GridH,
        ColorKey::Crosshair,
        ColorKey::ScaleText,
        ColorKey::ScaleLines,
    ];

    /// The candle colors as v1 applied them (to every chart type).
    fn shared(self) -> Option<SharedCandleColor> {
        Some(match self {
            ColorKey::UpBody => SharedCandleColor::UpBody,
            ColorKey::DownBody => SharedCandleColor::DownBody,
            ColorKey::UpBorder => SharedCandleColor::UpBorder,
            ColorKey::DownBorder => SharedCandleColor::DownBorder,
            ColorKey::UpWick => SharedCandleColor::UpWick,
            ColorKey::DownWick => SharedCandleColor::DownWick,
            _ => return None,
        })
    }

    fn set(self, s: &mut ChartStyle, v: ColorSlot) {
        if let Some(key) = self.shared() {
            s.apply_shared_color(key, v);
            return;
        }
        let c = &mut s.canvas;
        let slot = match self {
            ColorKey::Background => &mut c.background,
            ColorKey::GridV => &mut c.grid_v_color,
            ColorKey::GridH => &mut c.grid_h_color,
            ColorKey::Crosshair => &mut c.crosshair,
            ColorKey::ScaleText => &mut c.scale_text,
            ColorKey::ScaleLines => &mut c.scale_lines,
            _ => unreachable!("candle colors are shared"),
        };
        *slot = v;
    }

    /// The color in effect (override or theme) for `style` on `base`.
    fn effective(self, style: &ChartStyle, base: &Theme) -> Color32 {
        let t = style.theme(base);
        match self {
            ColorKey::UpBody => t.up,
            ColorKey::DownBody => t.down,
            ColorKey::UpBorder => ChartStyle::border_color(&style.symbol.candles, &t, true),
            ColorKey::DownBorder => ChartStyle::border_color(&style.symbol.candles, &t, false),
            ColorKey::UpWick => t.up_wick,
            ColorKey::DownWick => t.down_wick,
            ColorKey::Background => t.bg,
            ColorKey::GridV => style.grid_color(&t, true),
            ColorKey::GridH => style.grid_color(&t, false),
            ColorKey::Crosshair => t.crosshair,
            ColorKey::ScaleText => t.text_muted,
            ColorKey::ScaleLines => t.border,
        }
    }
}

/// A picked color, unmultiplied (as the picker shows it).
fn rgba(c: Hsla) -> tt_chart::Rgba {
    let r = Rgba::from(c);
    let b = |v: f32| (v * 255.0).round() as u8;
    tt_chart::Rgba::new(b(r.r), b(r.g), b(r.b), b(r.a))
}

/// Numeric fields: (label, unit, getter, setter, min, max).
type NumGet = fn(&ChartStyle) -> f32;
type NumSet = fn(&mut ChartStyle, f32);
const NUMBERS: [(&str, &str, NumGet, NumSet, f32, f32); 3] = [
    (
        "Top",
        "%",
        |s| s.canvas.margins.top,
        |s, v| s.canvas.margins.top = v,
        0.0,
        40.0,
    ),
    (
        "Bottom",
        "%",
        |s| s.canvas.margins.bottom,
        |s, v| s.canvas.margins.bottom = v,
        0.0,
        40.0,
    ),
    (
        "Right",
        "bars",
        |s| s.canvas.margins.right,
        |s, v| s.canvas.margins.right = v,
        0.0,
        100.0,
    ),
];

const FONT_SIZES: [f32; 8] = [10.0, 11.0, 12.0, 12.5, 13.0, 14.0, 16.0, 18.0];

pub struct ChartSettingsPanel {
    app: WeakEntity<TradeTerm>,
    base: Theme,
    original: ChartStyle,
    draft: ChartStyle,
    page: Page,
    pickers: Vec<(ColorKey, Entity<ColorPickerState>)>,
    numbers: Vec<Entity<InputState>>,
    committed: bool,
    _subs: Vec<Subscription>,
}

impl ChartSettingsPanel {
    pub fn new(
        app: WeakEntity<TradeTerm>,
        style: ChartStyle,
        base: Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut subs = Vec::new();
        let mut pickers = Vec::new();
        for key in ColorKey::ALL {
            let value = hsla(key.effective(&style, &base));
            let picker = cx.new(|cx| ColorPickerState::new(window, cx).default_value(value));
            subs.push(
                cx.subscribe(&picker, move |this, _, ev: &ColorPickerEvent, cx| {
                    if let ColorPickerEvent::Change(Some(c)) = ev {
                        let c = rgba(*c);
                        this.edit(cx, |s| key.set(s, Some(c)));
                    }
                }),
            );
            pickers.push((key, picker));
        }
        let mut numbers = Vec::new();
        for (_, _, get, set, min, max) in NUMBERS {
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .step(1.)
                    .min(min as f64)
                    .max(max as f64)
                    .default_value(format!("{}", get(&style)))
            });
            subs.push(
                cx.subscribe(&input, move |this, input, ev: &InputEvent, cx| {
                    if let InputEvent::Change = ev
                        && let Ok(v) = input.read(cx).value().trim().parse::<f32>()
                    {
                        this.edit(cx, |s| set(s, v.clamp(min, max)));
                    }
                }),
            );
            numbers.push(input);
        }
        // Closed without Ok (×, Esc, Cancel, outside click): restore.
        subs.push(cx.on_release(|this, cx| {
            if !this.committed {
                let (app, original) = (this.app.clone(), this.original.clone());
                app.update(cx, |a, cx| a.set_chart_style(original, false, cx))
                    .ok();
            }
        }));
        Self {
            app,
            base,
            original: style.clone(),
            draft: style,
            page: Page::Symbol,
            pickers,
            numbers,
            committed: false,
            _subs: subs,
        }
    }

    /// Changes the draft and previews it on every chart.
    fn edit(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut ChartStyle)) {
        f(&mut self.draft);
        let (app, style) = (self.app.clone(), self.draft.clone());
        cx.defer(move |cx| {
            app.update(cx, |a, cx| a.set_chart_style(style, false, cx))
                .ok();
        });
        cx.notify();
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let style = ChartStyle::default();
        for (key, picker) in &self.pickers {
            let v = hsla(key.effective(&style, &self.base));
            picker.update(cx, |p, cx| p.set_value(v, window, cx));
        }
        for ((_, _, get, ..), input) in NUMBERS.iter().zip(&self.numbers) {
            let v = format!("{}", get(&style));
            input.update(cx, |s, cx| s.set_value(v, window, cx));
        }
        self.edit(cx, |s| *s = style);
    }

    fn ok(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.committed = true;
        let (app, style) = (self.app.clone(), self.draft.clone());
        app.update(cx, |a, cx| a.set_chart_style(style, true, cx))
            .ok();
        window.close_dialog(cx);
    }

    fn picker(&self, key: ColorKey) -> impl IntoElement {
        let (_, p) = self
            .pickers
            .iter()
            .find(|(k, _)| *k == key)
            .expect("all keys");
        ColorPicker::new(p).small()
    }

    fn check(
        &self,
        id: &'static str,
        label: &'static str,
        value: bool,
        set: fn(&mut ChartStyle, bool),
        cx: &mut Context<Self>,
    ) -> Checkbox {
        Checkbox::new(id)
            .label(label)
            .checked(value)
            .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                let v = *checked;
                this.edit(cx, |s| set(s, v))
            }))
    }
}

fn section(title: &'static str, muted: Hsla) -> Div {
    div()
        .pt_4()
        .pb_1()
        .text_size(px(11.))
        .text_color(muted)
        .child(title)
}

fn row(label: impl IntoElement) -> Div {
    h_flex()
        .h(px(38.))
        .gap_3()
        .child(div().w(px(200.)).child(label))
}

impl Render for ChartSettingsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.draft.theme(&self.base);
        let muted = hsla(t.text_muted);
        let d = self.draft.clone();

        let mut sidebar = v_flex().w(px(200.)).gap_0p5();
        for (page, label, icon) in Page::ALL {
            let selected = page == self.page;
            sidebar = sidebar.child(
                h_flex()
                    .id(label)
                    .h(px(38.))
                    .px_3()
                    .gap_2()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(selected, |e| e.bg(hsla(t.selection)))
                    .hover(|e| e.bg(hsla(t.selection)))
                    .child(lucide(icon, 16.))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.page = page;
                        cx.notify();
                    })),
            );
        }

        let mut body = v_flex();
        match self.page {
            Page::Symbol => body = self.symbol_page(body, &d, muted, cx),
            Page::StatusLine => body = self.status_line_page(body, &d, muted, cx),
            Page::Scales => body = self.scales_page(body, &d, muted, cx),
            Page::Canvas => body = self.canvas_page(body, &d, muted, cx),
        }

        let this = cx.entity().downgrade();
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .items_start()
                    .gap_5()
                    .h(px(470.))
                    .child(sidebar)
                    .child(
                        div()
                            .id("settings-body")
                            .flex_1()
                            .h_full()
                            .overflow_y_scroll()
                            .child(body),
                    ),
            )
            .child(
                h_flex()
                    .pt_3()
                    .gap_2()
                    .border_t_1()
                    .border_color(hsla(t.border))
                    .child(
                        Button::new("reset")
                            .outline()
                            .label("Reset to defaults")
                            .on_click(move |_, window, cx| {
                                this.update(cx, |p, cx| p.reset(window, cx)).ok();
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("cancel")
                            .outline()
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("ok")
                            .primary()
                            .label("Ok")
                            .on_click(cx.listener(|this, _, window, cx| this.ok(window, cx))),
                    ),
            )
    }
}
