//! Every tt-ui component, for screenshot review.
//!
//! ```text
//! cargo run -p tt-ui --example gallery            # dark
//! cargo run -p tt-ui --example gallery -- --light # light
//! cargo run -p tt-ui --example gallery -- --measure
//! cargo run -p tt-ui --example gallery -- --icons  # icon contact sheet
//! ```
//!
//! `--measure` prints the laid-out bounds of the sized components (checked
//! against the token table) and exits with status 1 on a mismatch.

use gpui_kit::component::color_picker::ColorPickerState;
use gpui_kit::component::input::InputState;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use tt_theme::Theme;
use tt_ui::dialog::{DialogFooter, DialogFrame, DialogOpts, DialogSize, NavTabs, TemplateMenu};
use tt_ui::prelude::*;
use tt_ui::tokens as t;
use tt_ui::{
    ColorBarButton, LineDash, LinePicker, LineStyleButton, LineValue, LineWidthButton, Menu,
    TextStyleBar, TextStyleValue, hsla,
};

actions!(gallery, [PickTrendLine, OpenIndicators]);

type Measured = Rc<RefCell<Vec<(&'static str, Bounds<Pixels>)>>>;

struct Gallery {
    dark: bool,
    /// `--icons`: only the SVG icon contact sheet and the large menu.
    icons_only: bool,
    toggled: bool,
    magnet: bool,
    tf: &'static str,
    favorite: bool,
    collapsed: bool,
    select: u32,
    check: bool,
    tab: &'static str,
    nav: &'static str,
    number: Entity<InputState>,
    color: Entity<ColorPickerState>,
    measured: Measured,
    // Comfortable density (section 2.10).
    line: LineValue,
    text_style: TextStyleValue,
    fill: Hsla,
    range: (f32, f32),
    range_lo: Entity<InputState>,
    range_hi: Entity<InputState>,
    c_tab: &'static str,
    c_check: bool,
    c_vertical: &'static str,
}

impl Gallery {
    fn new(
        dark: bool,
        icons_only: bool,
        measured: Measured,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let number = cx.new(|cx| InputState::new(window, cx).default_value("14"));
        let color = cx.new(|cx| {
            ColorPickerState::new(window, cx).default_value(hsla(tt_theme::Theme::dark().active))
        });
        let range_lo = cx.new(|cx| InputState::new(window, cx).default_value("1"));
        let range_hi = cx.new(|cx| InputState::new(window, cx).default_value("59"));
        let blue = hsla(tt_theme::Theme::dark().active);
        Self {
            dark,
            icons_only,
            toggled: true,
            magnet: false,
            tf: "1h",
            favorite: true,
            collapsed: false,
            select: 1,
            check: true,
            tab: "inputs",
            nav: "symbol",
            number,
            color,
            measured,
            line: LineValue {
                color: blue,
                width: 1,
                dash: LineDash::Solid,
            },
            text_style: TextStyleValue {
                color: blue,
                size: 12,
                bold: false,
                italic: false,
            },
            fill: tt_ui::picker::with_alpha(blue, 0.2),
            range: (1.0, 59.0),
            range_lo,
            range_hi,
            c_tab: "style",
            c_check: true,
            c_vertical: "Middle",
        }
    }

    fn theme(&self) -> Theme {
        Theme::for_dark(self.dark)
    }
}

/// Records the bounds of `el` under `name` (for `--measure`).
fn measure(name: &'static str, m: &Measured, el: impl IntoElement) -> impl IntoElement {
    let m = m.clone();
    div().relative().flex_none().child(el).child(
        canvas(
            move |b, _, _| m.borrow_mut().push((name, b)),
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0(),
    )
}

fn section(title: &'static str, cx: &App) -> impl IntoElement {
    div()
        .pt(px(t::S4))
        .pb(px(t::S1))
        .text_size(px(t::TEXT_CAPTION))
        .font_weight(FontWeight::MEDIUM)
        .text_color(hsla(cx.ui().text_muted))
        .child(title.to_uppercase())
}

fn caption(s: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    div()
        .text_size(px(t::TEXT_SMALL))
        .text_color(hsla(cx.ui().text_muted))
        .child(s.into())
}

/// Our SVG icons: a grid of 28 px icons with names, then one row at the
/// three sizes (20 / 24 / 28).
fn icon_sheet(cols: usize, cx: &App) -> impl IntoElement {
    let muted = hsla(cx.ui().text_muted);
    let mut grid = v_flex().gap(px(t::S2));
    for chunk in Glyph::SVG_SET.chunks(cols) {
        let mut row = h_flex().gap(px(t::S1)).items_start();
        for (name, g) in chunk {
            row = row.child(
                v_flex()
                    .w(px(76.))
                    .items_center()
                    .gap(px(t::S1))
                    .child(glyph(*g, t::ICON_SVG))
                    .child(
                        div()
                            .text_size(px(t::TEXT_CAPTION))
                            .text_color(muted)
                            .text_center()
                            .child(*name),
                    ),
            );
        }
        grid = grid.child(row);
    }
    let mut sizes = h_flex().gap(px(t::S3)).items_end();
    for s in [20.0, 24.0, t::ICON_SVG] {
        let mut set = h_flex().gap(px(t::S1)).items_end();
        for g in [
            Glyph::CHART_CANDLES,
            Glyph::CHART_LINE,
            Glyph::DRAW_TREND_LINE,
            Glyph::DRAW_FIB_RETRACEMENT,
        ] {
            set = set.child(glyph(g, s));
        }
        sizes = sizes.child(set.child(div().text_color(muted).child(format!("{s}"))));
    }
    v_flex().gap(px(t::S3)).child(grid).child(sizes)
}

/// The chart-type menu as the app builds it: large rows, 28 px icons,
/// group separators, the current type (Candles) inverted, stars.
fn chart_type_menu_preview() -> Menu {
    let mut menu = Menu::new().id("ct-large-menu").width(MenuWidth::Lg).large();
    for (gi, group) in [0..4, 4..7, 7..10, 10..12, 12..15].into_iter().enumerate() {
        if gi > 0 {
            menu = menu.separator();
        }
        for (name, g) in &Glyph::SVG_SET[group] {
            menu = menu.row(
                MenuRow::new(SharedString::from(format!("ctl-{name}")), *name)
                    .icon(*g)
                    .current(*name == "Candles")
                    .favorite(*name == "Line" || *name == "Candles", |_, _| {}),
            );
        }
    }
    menu
}

/// The left drawing toolbar's buttons at their real size (34, icon 28).
fn drawing_toolbar_preview(cx: &App) -> impl IntoElement {
    let mut bar = v_flex()
        .w(px(t::W_DRAWING_TOOLBAR))
        .items_center()
        .gap(px(t::S1))
        .py(px(t::S1))
        .border_r_1()
        .border_color(hsla(cx.ui().border));
    for (i, g) in [
        Glyph::DRAW_CURSOR,
        Glyph::DRAW_TREND_LINE,
        Glyph::DRAW_FIB_RETRACEMENT,
        Glyph::DRAW_RECTANGLE,
        Glyph::DRAW_DATE_PRICE_RANGE,
        Glyph::DRAW_MAGNET,
        Glyph::DRAW_KEEP_DRAWING,
        Glyph::DRAW_UNLOCK,
        Glyph::DRAW_EYE,
        Glyph::DRAW_TRASH,
    ]
    .into_iter()
    .enumerate()
    {
        bar = bar.child(
            IconButton::new(SharedString::from(format!("dtp-{i}")), g)
                .size(ControlSize::Drawing)
                .toggled(i == 1)
                .tooltip("Drawing tool"),
        );
    }
    bar
}

impl Gallery {
    /// `--icons`: the contact sheet for docs/specs/img/icons-*.png.
    fn render_icons(&self, cx: &App) -> Div {
        let ui = cx.ui().clone();
        div()
            .size_full()
            .bg(hsla(ui.bg))
            .text_color(hsla(ui.text))
            .font_family(if cfg!(target_os = "windows") {
                "Segoe UI"
            } else {
                ".SystemUIFont"
            })
            .text_size(px(t::TEXT_BODY))
            .child(
                h_flex()
                    .p(px(t::S5))
                    .gap(px(t::S6))
                    .items_start()
                    .child(
                        v_flex()
                            .child(section("tradeterm icons · 1-stroke · 28 box", cx))
                            .child(icon_sheet(6, cx)),
                    )
                    .child(
                        v_flex()
                            .child(section("Drawing toolbar", cx))
                            .child(drawing_toolbar_preview(cx)),
                    )
                    .child(
                        v_flex()
                            .child(section("Chart type menu", cx))
                            .child(chart_type_menu_preview()),
                    ),
            )
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.icons_only {
            return self.render_icons(cx).into_any_element();
        }
        let theme = self.theme();
        if cx.ui() != &theme {
            tt_ui::theme::sync(&theme, cx);
        }
        let ui = cx.ui().clone();
        let m = self.measured.clone();
        m.borrow_mut().clear();
        let this = cx.entity().downgrade();

        // ---- colors ----------------------------------------------------
        let swatch = |name: &'static str, c: tt_theme::Color32| {
            v_flex()
                .gap(px(t::S0_5))
                .items_center()
                .child(
                    div()
                        .size(px(28.))
                        .rounded(px(t::R_SM))
                        .border_1()
                        .border_color(hsla(ui.border))
                        .bg(hsla(c)),
                )
                .child(
                    div()
                        .text_size(px(t::TEXT_CAPTION))
                        .text_color(hsla(ui.text_muted))
                        .child(name),
                )
        };
        let colors = h_flex()
            .gap(px(t::S3))
            .child(swatch("hover", ui.hover))
            .child(swatch("selection", ui.selection))
            .child(swatch("active", ui.active))
            .child(swatch("active_bg", ui.active_bg))
            .child(swatch("focus", ui.focus_ring))
            .child(swatch("favorite", ui.favorite))
            .child(swatch("warning", ui.warning))
            .child(swatch("danger", ui.danger))
            .child(swatch("overlay", ui.overlay_bg))
            .child(swatch("disabled", ui.text_disabled));

        // ---- icon buttons ----------------------------------------------
        let mut sizes = h_flex().gap(px(t::S3)).items_end();
        for (name, size) in [
            ("icon-xs", ControlSize::Xs),
            ("icon-sm", ControlSize::Sm),
            ("icon-md", ControlSize::Md),
            ("icon-lg", ControlSize::Lg),
        ] {
            sizes = sizes.child(measure(
                name,
                &m,
                IconButton::new(name, Glyph::SETTINGS)
                    .size(size)
                    .tooltip(format!("{name}: {} px", size.px())),
            ));
        }
        let states = h_flex()
            .gap(px(t::S1))
            .items_center()
            .child(
                IconButton::new("st-toggled", Glyph::MAGNET)
                    .size(ControlSize::Lg)
                    .toggled(self.magnet)
                    .tooltip("Magnet (click: toggles; tooltip must not go stale)")
                    .on_click(cx.listener(|g, _, _, cx| {
                        g.magnet = !g.magnet;
                        cx.notify()
                    })),
            )
            .child(
                IconButton::new("st-active", Glyph::CROSSHAIR)
                    .size(ControlSize::Lg)
                    .toggled(self.toggled)
                    .tooltip_action("Trend line", &PickTrendLine, None)
                    .on_click(cx.listener(|g, _, _, cx| {
                        g.toggled = !g.toggled;
                        cx.notify()
                    })),
            )
            .child(measure(
                "icon-flyout",
                &m,
                IconButton::new("st-flyout", Glyph::Lucide(lucide_icons::Icon::Slash))
                    .size(ControlSize::Lg)
                    .tooltip("Trend line (right-click or the corner: tools)")
                    .with_flyout(|menu, _, _| {
                        menu.section("Lines")
                            .row(
                                MenuRow::new("fl-trend", "Trend line")
                                    .icon(Glyph::Lucide(lucide_icons::Icon::Slash))
                                    .shortcut("Alt + T")
                                    .checked(true),
                            )
                            .row(
                                MenuRow::new("fl-ray", "Ray")
                                    .icon(Glyph::Lucide(lucide_icons::Icon::MoveUpRight)),
                            )
                            .row(
                                MenuRow::new("fl-h", "Horizontal line")
                                    .icon(Glyph::Lucide(lucide_icons::Icon::Minus))
                                    .shortcut("alt-h"),
                            )
                            .width(MenuWidth::Lg)
                    }),
            ))
            .child(
                IconButton::new("st-selected", Glyph::CHART_CANDLESTICK)
                    .selected(true)
                    .tooltip("Selected"),
            )
            .child(
                IconButton::new("st-danger", Glyph::TRASH)
                    .danger()
                    .tooltip("Danger"),
            )
            .child(
                IconButton::new("st-disabled", Glyph::LOCK)
                    .disabled(true)
                    .tooltip("Disabled"),
            )
            .child(Divider::vertical())
            .child(measure(
                "text-xs",
                &m,
                IconButton::text("st-a", "A")
                    .size(ControlSize::Xs)
                    .toggled(true)
                    .tooltip("Auto scale"),
            ))
            .child(
                IconButton::text("st-l", "L")
                    .size(ControlSize::Xs)
                    .tooltip("Log scale"),
            )
            .child(
                IconButton::text("st-p", "%")
                    .size(ControlSize::Xs)
                    .tooltip("Percent"),
            )
            .child(
                IconButton::new("st-theme", if self.dark { Glyph::SUN } else { Glyph::MOON })
                    .size(ControlSize::Sm)
                    .tooltip("Toggle theme")
                    .on_click(cx.listener(|g, _, _, cx| {
                        g.dark = !g.dark;
                        cx.notify()
                    })),
            );

        // ---- toolbar buttons -------------------------------------------
        let mut tfs = h_flex().gap(px(t::S0_5));
        for tf in ["1m", "15m", "1h", "4h", "1D"] {
            tfs = tfs.child(
                ToolbarButton::text(tf, tf)
                    .selected(self.tf == tf)
                    .on_click(cx.listener(move |g, _, _, cx| {
                        g.tf = tf;
                        cx.notify()
                    })),
            );
        }
        let collapsed = self.collapsed;
        let favorite = self.favorite;
        let tf_now = self.tf;
        let (w1, w2, w3) = (this.clone(), this.clone(), this.clone());
        let toolbar = h_flex()
            .h(px(t::H_TOOLBAR))
            .px(px(t::S2))
            .gap(px(t::S1))
            .items_center()
            .border_1()
            .border_color(hsla(ui.border))
            .rounded(px(t::R_MD))
            .bg(hsla(ui.panel))
            .child(measure(
                "field",
                &m,
                ToolbarButton::field("tb-ticker", "BTCUSDT").tooltip("Symbol search"),
            ))
            .child(Divider::vertical())
            .child(tfs)
            .child(measure(
                "tb-caret",
                &m,
                ToolbarButton::icon("tb-tf-menu", Glyph::CHEVRON_DOWN)
                    .tooltip("Intervals")
                    .menu(move |menu, _, _| {
                        let (w1, w2, w3) = (w1.clone(), w2.clone(), w3.clone());
                        menu.max_h(px(360.))
                            .row(MenuRow::new("tf-add", "Add custom interval…").icon(Glyph::PLUS))
                            .separator()
                            .collapsible_section("Minutes", !collapsed, move |_, cx| {
                                w1.update(cx, |g, cx| {
                                    g.collapsed = !g.collapsed;
                                    cx.notify()
                                })
                                .ok();
                            })
                            .when(!collapsed, |menu| {
                                menu.rows(["1m", "15m"].map(|tf| {
                                    MenuRow::new(tf, format!("{tf} minutes")).checked(tf_now == tf)
                                }))
                                .row(
                                    MenuRow::new("tf-7m", "7 minutes")
                                        .removable("Remove interval", |_, _| {}),
                                )
                            })
                            .section("Hours")
                            .row(
                                MenuRow::new("1h", "1 hour")
                                    .checked(tf_now == "1h")
                                    .favorite(favorite, move |_, cx| {
                                        w2.update(cx, |g, cx| {
                                            g.favorite = !g.favorite;
                                            cx.notify()
                                        })
                                        .ok();
                                    })
                                    .on_select(move |_, cx| {
                                        w3.update(cx, |g, cx| {
                                            g.tf = "1h";
                                            cx.notify()
                                        })
                                        .ok();
                                    }),
                            )
                            .row(MenuRow::new("4h", "4 hours").checked(tf_now == "4h"))
                    }),
            ))
            .child(Divider::vertical())
            .child(
                ToolbarButton::icon("tb-ct", Glyph::CHART_CANDLESTICK)
                    .caret()
                    .tooltip("Candles")
                    .menu(|menu, _, _| {
                        menu.row(
                            MenuRow::new("ct-candles", "Candles")
                                .icon(Glyph::CHART_CANDLESTICK)
                                .checked(true)
                                .description("Open, high, low and close as candle bodies"),
                        )
                        .row(
                            MenuRow::new("ct-line", "Line")
                                .icon(Glyph::Lucide(lucide_icons::Icon::ChartLine)),
                        )
                    }),
            )
            .child(Divider::vertical())
            .child(measure(
                "tb-text",
                &m,
                ToolbarButton::text("tb-ind", "Indicators")
                    .leading(Glyph::SQUARE_FUNCTION)
                    .tooltip_action("Indicators, metrics and strategies", &OpenIndicators, None),
            ));

        let chips = h_flex()
            .gap(px(t::S1))
            .items_center()
            .child(measure(
                "chip",
                &m,
                Chip::new("c-all", "All").selected(true),
            ))
            .child(Chip::new("c-usdt", "USDT"))
            .child(Chip::new("c-btc", "BTC"))
            .child(Divider::vertical())
            .child(Tag::new("SPOT"))
            .child(caption(
                format!(
                    "format_keys: {} · {} · {}",
                    tt_ui::format_keys("Alt + Shift + R"),
                    tt_ui::format_keys("ctrl-t"),
                    tt_ui::format_keys("/")
                ),
                cx,
            ));

        // ---- static menu -------------------------------------------------
        let menu = Menu::new()
            .id("static-menu")
            .width(MenuWidth::Lg)
            .section("Theme")
            .row(MenuRow::new("m-dark", "Dark").checked(self.dark))
            .row(MenuRow::new("m-light", "Light").checked(!self.dark))
            .row(MenuRow::new("m-system", "System").checked(false))
            .separator()
            .row(
                MenuRow::new("m-settings", "Chart settings…")
                    .icon(Glyph::SETTINGS)
                    .shortcut("ctrl-comma"),
            )
            .row(
                MenuRow::new("m-fav", "1 hour")
                    .favorite(true, |_, _| {})
                    .checked(false),
            )
            .row(MenuRow::new("m-disabled", "Disabled row").disabled(true))
            .section("Developer")
            .row(MenuRow::new("m-synth", "Load 1M synthetic bars").shortcut("ctrl-shift-d"))
            .row(MenuRow::danger("m-remove", "Remove 3 drawings").icon(Glyph::TRASH));

        let list = v_flex()
            .w(px(280.))
            .child(SectionHeader::new("Recent"))
            .child(measure(
                "list-row",
                &m,
                ListRow::new("lr-1")
                    .leading(
                        div()
                            .size(px(28.))
                            .rounded_full()
                            .bg(hsla(ui.series_palette[1]))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(t::TEXT_SMALL))
                            .text_color(hsla(tt_theme::badge_fg(ui.series_palette[1])))
                            .child("B"),
                    )
                    .primary("BTCUSDT")
                    .secondary("Bitcoin / TetherUS")
                    .trailing(Tag::new("SPOT"))
                    .tall()
                    .selected(true),
            ))
            .child(
                ListRow::new("lr-2")
                    .primary("ETHUSDT")
                    .secondary("Ethereum / TetherUS"),
            )
            .child(EmptyState::new("Nothing matches"));

        // ---- floating toolbar --------------------------------------------
        let floating = measure(
            "floating",
            &m,
            Overlay::new()
                .floating()
                .child(ColorField::new(&self.color).compact())
                .child(
                    ToolbarButton::text("ft-width", "2px")
                        .size(ControlSize::Sm)
                        .caret()
                        .menu(|menu, _, _| {
                            menu.width(MenuWidth::Sm).rows(
                                [1usize, 2, 3, 4].map(|w| MenuRow::new(("w", w), format!("{w}px"))),
                            )
                        }),
                )
                .child(
                    IconButton::new("ft-lock", Glyph::LOCK_OPEN)
                        .size(ControlSize::Sm)
                        .tooltip("Lock"),
                )
                .child(
                    IconButton::new("ft-trash", Glyph::TRASH)
                        .size(ControlSize::Sm)
                        .tooltip("Remove"),
                ),
        );
        // ---- comfortable density (section 2.10) ---------------------------
        let wc = cx.entity().downgrade();
        let upd = move |f: Box<dyn FnOnce(&mut Gallery)>, cx: &mut App| {
            wc.update(cx, |g, cx| {
                f(g);
                cx.notify();
            })
            .ok();
        };
        let (u1, u2, u3, u4, u5, u6, u7, u8, u9) = (
            upd.clone(),
            upd.clone(),
            upd.clone(),
            upd.clone(),
            upd.clone(),
            upd.clone(),
            upd.clone(),
            upd.clone(),
            upd,
        );
        let floating_tv = measure(
            "floating-tv",
            &m,
            Overlay::new()
                .floating()
                .child(IconButton::new("ftv-template", Glyph::GRID_PLUS).size(ControlSize::Xl))
                .child(measure(
                    "ft-color-bar",
                    &m,
                    ColorBarButton::new("ftv-line", Glyph::PENCIL, self.line.color)
                        .tooltip("Line color")
                        .on_change(move |c, _, cx| u1(Box::new(move |g| g.line.color = c), cx)),
                ))
                .child(
                    ColorBarButton::new("ftv-fill", Glyph::PAINT_BUCKET, self.fill)
                        .tooltip("Background color")
                        .on_change(move |c, _, cx| u9(Box::new(move |g| g.fill = c), cx)),
                )
                .child(
                    ColorBarButton::new("ftv-text", Glyph::TYPE, self.text_style.color)
                        .tooltip("Text color")
                        .on_change(move |c, _, cx| {
                            u2(Box::new(move |g| g.text_style.color = c), cx)
                        }),
                )
                .child(
                    LineWidthButton::new("ftv-width", self.line.width)
                        .on_change(move |w, _, cx| u3(Box::new(move |g| g.line.width = w), cx)),
                )
                .child(
                    LineStyleButton::new("ftv-style", self.line.dash)
                        .on_change(move |d, _, cx| u4(Box::new(move |g| g.line.dash = d), cx)),
                )
                .child(IconButton::new("ftv-settings", Glyph::SETTINGS).size(ControlSize::Xl))
                .child(IconButton::new("ftv-lock", Glyph::LOCK_OPEN).size(ControlSize::Xl))
                .child(IconButton::new("ftv-trash", Glyph::TRASH).size(ControlSize::Xl))
                .child(IconButton::new("ftv-more", Glyph::ELLIPSIS).size(ControlSize::Xl)),
        );
        let c_tab = self.c_tab;
        let wt = cx.entity().downgrade();
        let tabs = NavTabs::horizontal()
            .item("style", "Style", None)
            .item("text", "Text", None)
            .item("coords", "Coordinates", None)
            .item("visibility", "Visibility", None)
            .selected(c_tab)
            .on_change(move |id, _, cx| {
                wt.update(cx, |g, cx| {
                    g.c_tab = id;
                    cx.notify()
                })
                .ok();
            });
        let c_body = v_flex()
            .child(
                FormRow::new("Line")
                    .comfortable()
                    .label_auto()
                    .control(measure(
                        "line-picker",
                        &m,
                        LinePicker::new("c-line", self.line)
                            .on_change(move |v, _, cx| u5(Box::new(move |g| g.line = v), cx)),
                    )),
            )
            .child(
                FormRow::check(
                    "c-price-label",
                    "Price label",
                    self.c_check,
                    move |on, _, cx| u6(Box::new(move |g| g.c_check = on), cx),
                )
                .comfortable(),
            )
            .child(
                h_flex().h(px(t::C_ROW)).items_center().child(
                    TextStyleBar::new("c-text", self.text_style, &[10, 11, 12, 14, 16, 20, 24])
                        .on_change(move |v, _, cx| u7(Box::new(move |g| g.text_style = v), cx)),
                ),
            )
            .child(
                FormRow::new("Text alignment")
                    .comfortable()
                    .label_auto()
                    .control(measure(
                        "select-c",
                        &m,
                        Select::new(
                            "c-valign",
                            ["Middle", "Top", "Bottom"].map(|s| (s, s)),
                            self.c_vertical,
                        )
                        .comfortable()
                        .on_change(move |s, _, cx| u8(Box::new(move |g| g.c_vertical = s), cx)),
                    ))
                    .control(measure("field-c", &m, Field::new(&self.number))),
            )
            .child({
                let wr = cx.entity().downgrade();
                FormRow::check("c-minutes", "Minutes", true, |_, _, _| {})
                    .comfortable()
                    .label_width(96.0)
                    .control(measure(
                        "range-field",
                        &m,
                        RangeField::new(
                            "c-range",
                            &self.range_lo,
                            &self.range_hi,
                            (1.0, 59.0),
                            self.range,
                        )
                        .on_change(move |a, b, _, cx| {
                            wr.update(cx, |g, cx| {
                                g.range = (a, b);
                                cx.notify()
                            })
                            .ok();
                        }),
                    ))
            });
        let comfortable_preview = v_flex()
            .w(px(454.))
            .bg(hsla(ui.panel))
            .border_1()
            .border_color(hsla(ui.border))
            .rounded(px(t::C_R_DIALOG))
            .shadow_lg()
            .child(
                h_flex()
                    .h(px(t::C_H_HEADER))
                    .px(px(t::C_PAD_X))
                    .items_center()
                    .gap(px(5.))
                    .text_size(px(t::C_TEXT_TITLE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Horizontal ray")
                    .child(glyph(Glyph::PENCIL, t::ICON_MD + 2.0)),
            )
            .child(
                DialogFrame::new(DialogSize::Sm)
                    .id("c-frame")
                    .comfortable()
                    .nav(tabs)
                    .body(c_body),
            )
            .child(
                DialogFooter::new()
                    .comfortable()
                    .template(TemplateMenu::defaults(|_, _| {}))
                    .cancel()
                    .ok("Ok"),
            );
        let pane_controls = caption(
            format!(
                "PaneControls::width(3) = {}",
                tt_ui::PaneControls::width_f32(3)
            ),
            cx,
        );

        // ---- dialog preview ----------------------------------------------
        let size = DialogSize::Sm;
        let w = this.clone();
        let w_check = this.clone();
        let w_sel = this.clone();
        let body = v_flex()
            .child(
                FormRow::new("Length")
                    .label_width(size.label_width())
                    .control(NumberField::new(&self.number)),
            )
            .child(
                FormRow::new("Source")
                    .label_width(size.label_width())
                    .control(
                        Select::new(
                            "src",
                            [(0u32, "Open"), (1, "Close"), (2, "High"), (3, "Low")],
                            self.select,
                        )
                        .on_change(move |v, _, cx| {
                            w_sel
                                .update(cx, |g, cx| {
                                    g.select = v;
                                    cx.notify()
                                })
                                .ok();
                        }),
                    ),
            )
            .child(
                FormRow::new("Color")
                    .label_width(size.label_width())
                    .control(ColorField::new(&self.color)),
            )
            .child(
                FormRow::check(
                    "chk",
                    "Color bars based on previous close",
                    self.check,
                    move |v, _, cx| {
                        w_check
                            .update(cx, |g, cx| {
                                g.check = v;
                                cx.notify()
                            })
                            .ok();
                    },
                )
                .label_width(size.label_width() + 120.)
                .control(ColorField::new(&self.color)),
            )
            .child(
                FormRow::new("Offset")
                    .label_width(size.label_width())
                    .indent()
                    .control(NumberField::new(&self.number))
                    .unit("bars"),
            );
        let w_tab = this.clone();
        let frame = DialogFrame::new(size)
            .id("preview-frame")
            .nav(
                NavTabs::horizontal()
                    .item("inputs", "Inputs", None)
                    .item("style", "Style", None)
                    .item("visibility", "Visibility", None)
                    .selected(self.tab)
                    .on_change(move |id, _, cx| {
                        w_tab
                            .update(cx, |g, cx| {
                                g.tab = id;
                                cx.notify()
                            })
                            .ok();
                    }),
            )
            .body(body);
        let footer = DialogFooter::new()
            .template(TemplateMenu::defaults(|_, _| {}))
            .cancel()
            .ok("Ok");
        let dialog_preview = v_flex()
            .w(px(size.width()))
            .bg(hsla(ui.panel))
            .border_1()
            .border_color(hsla(ui.border))
            .rounded(px(t::R_XL))
            .shadow_lg()
            .child(
                div()
                    .h(px(t::H_DIALOG_HEADER))
                    .px(px(t::S5))
                    .flex()
                    .items_center()
                    .text_size(px(t::TEXT_TITLE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Moving Average Simple"),
            )
            .child(div().px(px(t::S5)).child(frame))
            .child(
                div()
                    .h(px(t::H_DIALOG_FOOTER))
                    .px(px(t::S5))
                    .flex()
                    .items_center()
                    .border_t_1()
                    .border_color(hsla(ui.border))
                    .child(footer),
            );
        let w_nav = this.clone();
        let nav_preview = h_flex()
            .w(px(420.))
            .p(px(t::S3))
            .bg(hsla(ui.panel))
            .border_1()
            .border_color(hsla(ui.border))
            .rounded(px(t::R_XL))
            .child(
                NavTabs::vertical()
                    .item("symbol", "Symbol", Some(Glyph::CHART_CANDLESTICK))
                    .item(
                        "scales",
                        "Scales and lines",
                        Some(Glyph::Lucide(lucide_icons::Icon::Ruler)),
                    )
                    .item("canvas", "Canvas", Some(Glyph::PALETTE))
                    .item(
                        "trading",
                        "Trading",
                        Some(Glyph::Lucide(lucide_icons::Icon::ArrowLeftRight)),
                    )
                    .selected(self.nav)
                    .on_change(move |id, _, cx| {
                        w_nav
                            .update(cx, |g, cx| {
                                g.nav = id;
                                cx.notify()
                            })
                            .ok();
                    }),
            );
        let open = ToolbarButton::text("open-dialog", "Open a real dialog…").on_click(
            move |_, window, cx| {
                let Some(g) = w.upgrade() else { return };
                let panel = cx.new(|_| DialogBody(g.downgrade()));
                tt_ui::open_dialog(
                    window,
                    cx,
                    DialogOpts::new("Add custom interval", DialogSize::Sm)
                        .footer(DialogFooter::new().cancel().ok("Add").on_ok(|_, _| true)),
                    panel,
                );
            },
        );

        let _ = window;
        div()
            .id("gallery")
            .size_full()
            .overflow_y_scroll()
            .bg(hsla(ui.bg))
            .text_color(hsla(ui.text))
            .font_family(if cfg!(target_os = "windows") {
                "Segoe UI"
            } else {
                ".SystemUIFont"
            })
            .text_size(px(t::TEXT_BODY))
            .child(
                h_flex()
                    .p(px(t::S5))
                    .gap(px(t::S6))
                    .items_start()
                    .child(
                        v_flex()
                            .w(px(640.))
                            .child(section(
                                if self.dark {
                                    "tt-ui gallery · dark"
                                } else {
                                    "tt-ui gallery · light"
                                },
                                cx,
                            ))
                            .child(colors)
                            .child(section("Icon buttons: Xs 20 · Sm 24 · Md 32 · Lg 36", cx))
                            .child(sizes)
                            .child(section(
                                "States: toggled · flyout · selected · danger · disabled · A/L/%",
                                cx,
                            ))
                            .child(states)
                            .child(section("Toolbar", cx))
                            .child(toolbar)
                            .child(section("Chips, tags, keys", cx))
                            .child(chips)
                            .child(section("Floating toolbar", cx))
                            .child(
                                h_flex()
                                    .gap(px(t::S3))
                                    .items_center()
                                    .child(floating)
                                    .child(pane_controls),
                            )
                            .child(section(
                                "Comfortable (TradingView-measured): floating toolbar 38 · dialog",
                                cx,
                            ))
                            .child(floating_tv)
                            .child(div().h(px(t::S3)))
                            .child(comfortable_preview)
                            .child(section("Dialog (Sm, top tabs)", cx))
                            .child(dialog_preview)
                            .child(div().pt(px(t::S3)).child(open))
                            .child(section("SVG icons (28 box, 1 px stroke)", cx))
                            .child(icon_sheet(8, cx)),
                    )
                    .child(
                        v_flex()
                            .gap(px(t::S1))
                            .child(section("Menu", cx))
                            .child(menu)
                            .child(section("List", cx))
                            .child(list)
                            .child(section("Dialog left nav", cx))
                            .child(nav_preview)
                            .child(section("Large menu: chart type, current inverted", cx))
                            .child(measure("menu-large", &m, chart_type_menu_preview()))
                            .child(section("Drawing toolbar 52 · buttons 34", cx))
                            .child(measure("drawing-toolbar", &m, drawing_toolbar_preview(cx))),
                    ),
            )
            .into_any_element()
    }
}

/// Body of the real dialog opened from the gallery.
struct DialogBody(WeakEntity<Gallery>);

impl Render for DialogBody {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(g) = self.0.upgrade() else {
            return div();
        };
        let number = g.read(cx).number.clone();
        div().child(
            FormRow::new("Interval")
                .control(NumberField::new(&number))
                .unit("minutes"),
        )
    }
}

/// Expected sizes (token table, §2.4).
const EXPECTED: [(&str, f32, f32); 17] = [
    ("icon-xs", t::CONTROL_XS, t::CONTROL_XS),
    ("icon-sm", t::CONTROL_SM, t::CONTROL_SM),
    ("icon-md", t::CONTROL_MD, t::CONTROL_MD),
    ("icon-lg", t::CONTROL_LG, t::CONTROL_LG),
    ("icon-flyout", t::CONTROL_LG, t::CONTROL_LG),
    ("text-xs", t::CONTROL_XS, t::CONTROL_XS),
    ("tb-caret", t::CONTROL_MD, t::CONTROL_MD),
    ("chip", f32::NAN, t::CONTROL_SM),
    ("tb-text", f32::NAN, t::CONTROL_MD),
    // Comfortable density (section 2.10).
    ("floating-tv", f32::NAN, t::FT_HEIGHT),
    ("ft-color-bar", t::FT_BUTTON, t::FT_BUTTON),
    ("line-picker", tt_ui::picker::W_LINE_PICKER, t::C_CONTROL),
    ("select-c", t::C_W_FIELD, t::C_CONTROL),
    ("field-c", t::C_W_FIELD, t::C_CONTROL),
    ("range-field", f32::NAN, t::C_CONTROL),
    // TradingView-measured: large menu, drawing toolbar.
    ("menu-large", t::W_MENU_LG, f32::NAN),
    ("drawing-toolbar", t::W_DRAWING_TOOLBAR, f32::NAN),
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dark = !args.iter().any(|a| a == "--light");
    let measure_only = args.iter().any(|a| a == "--measure");
    let icons_only = args.iter().any(|a| a == "--icons");
    let measured: Measured = Rc::new(RefCell::new(Vec::new()));

    gpui_kit::application()
        .with_assets(tt_ui::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            tt_ui::init(cx);
            tt_ui::theme::sync(&Theme::for_dark(dark), cx);
            cx.bind_keys([
                KeyBinding::new("alt-t", PickTrendLine, None),
                KeyBinding::new("/", OpenIndicators, None),
            ]);
            let opts = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1180.), px(1000.)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("tt-ui gallery".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let m = measured.clone();
            gpui_kit::open_window(opts, cx, move |window, cx| {
                cx.new(|cx| Gallery::new(dark, icons_only, m.clone(), window, cx))
            })
            .expect("open window");

            if measure_only {
                let m = measured.clone();
                cx.spawn(async move |cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(1500))
                        .await;
                    let mut ok = true;
                    let got = m.borrow().clone();
                    for (name, w, h) in EXPECTED {
                        match got.iter().find(|(n, _)| *n == name) {
                            Some((_, b)) => {
                                let (bw, bh) = (f32::from(b.size.width), f32::from(b.size.height));
                                let good = (w.is_nan() || (bw - w).abs() < 0.5)
                                    && (h.is_nan() || (bh - h).abs() < 0.5);
                                ok &= good;
                                println!(
                                    "{} {name}: {bw}x{bh} (expected {}x{h})",
                                    if good { "ok  " } else { "FAIL" },
                                    if w.is_nan() {
                                        "*".to_owned()
                                    } else {
                                        w.to_string()
                                    }
                                );
                            }
                            None => {
                                ok = false;
                                println!("FAIL {name}: not rendered");
                            }
                        }
                    }
                    for (name, b) in got
                        .iter()
                        .filter(|(n, _)| !EXPECTED.iter().any(|e| e.0 == *n))
                    {
                        println!(
                            "info {name}: {}x{}",
                            f32::from(b.size.width),
                            f32::from(b.size.height)
                        );
                    }
                    println!("MEASURE_DONE {}", if ok { "ok" } else { "mismatch" });
                    cx.update(|cx| cx.quit());
                    if !ok {
                        std::process::exit(1);
                    }
                })
                .detach();
            }
        });
}
