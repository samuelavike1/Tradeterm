//! The TradingView-measured ("comfortable", ui-guidelines §2.10) layout of
//! the drawing settings dialog, used by every available tool
//! (`tt_chart::drawings::form::dialog_layout`). What each tool shows and in
//! which order is in `form::style_fields` / `form::text_fields`
//! (docs/specs/drawings-tv-ref.md); this file renders each field kind once:
//!
//! - lines: one `LinePicker` (+ `LineEndSelect`s on line tools), colors:
//!   `ColorSwatchButton`, a checkbox in the label column where the field
//!   has an on / off;
//! - Extend / Stats: `CheckSelect` (checkable menu items);
//! - Fib levels: a two-column grid of `[x] [coeff] [swatch]`; channel
//!   levels: one column of `[x] [coeff] [line]`;
//! - Text tab: `[color][size][B][I]`, the text area, "Text alignment";
//! - Coordinates: price and bar number per point (channel: two points and
//!   "Price offset"); Visibility: Ticks / units with range sliders / Ranges.
//!
//! Labels share one column per tab, as wide as the widest label (like
//! TradingView's table).

use super::{DrawingSettingsPanel, NumberTarget, from_hsla, to_hsla};
use gpui_kit::component::input::Textarea;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use serde_json::Value;
use tt_chart::drawings::form::{self, FONT_SIZES, Field, FieldKind};
use tt_chart::drawings::props::{Stroke, TextStyle};
use tt_chart::{Drawing, LineStyle, Props, Visibility};
use tt_ui::prelude::*;
use tt_ui::tokens::*;
use tt_ui::{CheckSelect, LineEndSelect, TextStyleBar};

/// Visibility rows' label column (TradingView: label + 20).
const W_VIS_LABEL: f32 = 96.0;
/// Level coefficient inputs.
const W_COEFF: f32 = 72.0;
/// The font size select next to a label color.
const W_SIZE: f32 = 72.0;
/// The Fib background opacity slider.
const W_OPACITY: f32 = 110.0;

pub(super) fn dash(s: LineStyle) -> LineDash {
    match s {
        LineStyle::Solid => LineDash::Solid,
        LineStyle::Dashed => LineDash::Dashed,
        LineStyle::Dotted => LineDash::Dotted,
    }
}

pub(super) fn line_style(d: LineDash) -> LineStyle {
    match d {
        LineDash::Solid => LineStyle::Solid,
        LineDash::Dashed => LineStyle::Dashed,
        LineDash::Dotted => LineStyle::Dotted,
    }
}

fn line_value(s: Stroke) -> LineValue {
    LineValue {
        color: to_hsla(s.color),
        width: s.width,
        dash: dash(s.style),
    }
}

fn stroke(lv: LineValue) -> Stroke {
    Stroke {
        color: from_hsla(lv.color),
        width: lv.width.clamp(1, 4),
        style: line_style(lv.dash),
    }
}

/// Width of `s` in the dialog's 14 px text.
fn text_w(window: &Window, s: &str) -> f32 {
    if s.is_empty() {
        return 0.0;
    }
    let run = window.text_style().to_run(s.len());
    let line = window.text_system().shape_line(
        SharedString::from(s.to_owned()),
        px(TEXT_BODY),
        &[run],
        None,
    );
    f32::from(line.width).ceil()
}

impl DrawingSettingsPanel {
    /// The label column of a tab: its widest label (+ checkbox) and the
    /// gap to the controls. Lone checkboxes span the row and don't count.
    pub(super) fn label_column(&self, fields: &[Field], window: &Window) -> f32 {
        let check = C_CHECKBOX + C_GAP;
        fields
            .iter()
            .filter(|f| {
                !matches!(
                    f.kind,
                    FieldKind::Check
                        | FieldKind::Section
                        | FieldKind::Levels
                        | FieldKind::TextBar
                        | FieldKind::TextArea
                )
            })
            .map(|f| text_w(window, f.label) + if f.check.is_some() { check } else { 0.0 })
            .fold(0.0, f32::max)
            + C_LABEL_GAP
    }

    /// A row for `f`: its label, or checkbox + label when it has an on /
    /// off.
    fn row_tv(&self, f: &Field, v: &Value, lw: f32, cx: &mut Context<Self>) -> FormRow {
        let id = SharedString::from(format!("tv{}-row", f.path.replace('/', "-")));
        let r = match &f.check {
            Some(c) => {
                let set = self.setter(cx, c.clone());
                FormRow::check(id, f.label, form::get_bool(v, c), move |on, _, cx| {
                    set(Value::Bool(on), cx)
                })
            }
            None => FormRow::new(f.label),
        }
        .comfortable()
        .label_width(lw);
        if f.indent { r.indent() } else { r }
    }

    /// A `LinePicker` editing the stroke at `path`.
    fn line_picker(
        &self,
        id: String,
        path: String,
        s: Stroke,
        cx: &mut Context<Self>,
    ) -> LinePicker {
        let this = cx.entity().downgrade();
        LinePicker::new(SharedString::from(id), line_value(s)).on_change(move |lv, _, cx| {
            let path = path.clone();
            this.update(cx, |p, cx| {
                p.edit(cx, move |d| {
                    if let Ok(val) = serde_json::to_value(stroke(lv)) {
                        form::set(&mut d.props, &path, val);
                    }
                })
            })
            .ok();
        })
    }

    /// A `ColorSwatchButton` editing the color at `path`.
    fn swatch(
        &self,
        id: String,
        path: String,
        v: &Value,
        cx: &mut Context<Self>,
    ) -> ColorSwatchButton {
        let this = cx.entity().downgrade();
        let now = form::get_color(v, &path).unwrap_or(tt_chart::Rgba::rgb(0, 0, 0));
        ColorSwatchButton::new(SharedString::from(id), to_hsla(now)).on_change(move |c, _, cx| {
            let path = path.clone();
            this.update(cx, |p, cx| {
                p.edit(cx, move |d| {
                    form::set_color(&mut d.props, &path, from_hsla(c));
                })
            })
            .ok();
        })
    }

    /// A comfortable select of `options` at `path`.
    fn select(
        &self,
        id: String,
        path: &str,
        options: &[(&'static str, &'static str)],
        v: &Value,
        cx: &mut Context<Self>,
    ) -> Select<String> {
        let set = self.setter(cx, path.to_owned());
        Select::new(
            SharedString::from(id),
            options.iter().map(|(val, label)| (val.to_string(), *label)),
            form::get_choice(v, path),
        )
        .comfortable()
        .on_change(move |val: String, _, cx| set(form::choice_value(&val), cx))
    }

    /// A font size select at `path`.
    fn size_select(
        &self,
        id: String,
        path: String,
        v: &Value,
        cx: &mut Context<Self>,
    ) -> Select<u8> {
        let set = self.setter(cx, path.clone());
        Select::new(
            SharedString::from(id),
            FONT_SIZES.map(|s| (s, s.to_string())),
            form::get_f64(v, &path) as u8,
        )
        .comfortable()
        .on_change(move |s: u8, _, cx| set(Value::from(s), cx))
    }

    /// One field in the comfortable layout (every kind; `lw`: the tab's
    /// label column).
    pub(super) fn field_tv(
        &self,
        f: &Field,
        d: &Drawing,
        v: &Value,
        lw: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = |suffix: &str| format!("tv{}{suffix}", f.path.replace('/', "-"));
        match &f.kind {
            FieldKind::Section => div()
                .pt(px(S2))
                .text_size(px(C_TEXT_SECTION))
                .text_color(tt_ui::hsla(cx.ui().text_muted))
                .child(f.label)
                .into_any_element(),
            FieldKind::Check => {
                let set = self.setter(cx, f.path.clone());
                FormRow::check(
                    SharedString::from(id("")),
                    f.label,
                    form::get_bool(v, &f.path),
                    move |on, _, cx| set(Value::Bool(on), cx),
                )
                .comfortable()
                .into_any_element()
            }
            FieldKind::Line => {
                let s = form::get_stroke(v, &f.path).unwrap_or_default();
                let mut row = self.row_tv(f, v, lw, cx).control(self.line_picker(
                    id("-picker"),
                    f.path.clone(),
                    s,
                    cx,
                ));
                if f.ends {
                    for (right, path) in [(false, "/ends/left"), (true, "/ends/right")] {
                        let set = self.setter(cx, path.into());
                        let arrow = form::get_choice(v, path) == "Arrow";
                        row = row.control(
                            LineEndSelect::new(SharedString::from(id(path)), right, arrow)
                                .on_change(move |a, _, cx| {
                                    set(
                                        Value::String(if a { "Arrow" } else { "Normal" }.into()),
                                        cx,
                                    )
                                }),
                        );
                    }
                }
                row.into_any_element()
            }
            FieldKind::Color => self
                .row_tv(f, v, lw, cx)
                .control(self.swatch(id("-swatch"), f.path.clone(), v, cx))
                .into_any_element(),
            FieldKind::WidthStyle => {
                let w = form::get_f64(v, &format!("{}/width", f.path)) as u8;
                let st: LineStyle = form::get(v, &format!("{}/style", f.path))
                    .and_then(|x| serde_json::from_value(x.clone()).ok())
                    .unwrap_or_default();
                let (set_w, set_s) = (
                    self.setter(cx, format!("{}/width", f.path)),
                    self.setter(cx, format!("{}/style", f.path)),
                );
                self.row_tv(f, v, lw, cx)
                    .control(
                        LineWidthButton::new(SharedString::from(id("-w")), w)
                            .boxed()
                            .on_change(move |w, _, cx| set_w(Value::from(w), cx)),
                    )
                    .control(
                        LineStyleButton::new(SharedString::from(id("-s")), dash(st))
                            .boxed()
                            .on_change(move |d, _, cx| {
                                if let Ok(v) = serde_json::to_value(line_style(d)) {
                                    set_s(v, cx)
                                }
                            }),
                    )
                    .into_any_element()
            }
            FieldKind::Choice(options) => {
                let select = self.select(id(""), &f.path, options, v, cx);
                // Stats position lines up with the Stats menu above it.
                let select = if f.path.starts_with("/stats") {
                    select.width(tt_ui::form::W_CHECK_SELECT)
                } else {
                    select
                };
                self.row_tv(f, v, lw, cx).control(select).into_any_element()
            }
            FieldKind::Pair {
                first,
                second_path,
                second,
            } => self
                .row_tv(f, v, lw, cx)
                .control(self.select(id("-a"), &f.path, first, v, cx))
                .control(self.select(id("-b"), second_path, second, v, cx))
                .into_any_element(),
            FieldKind::CheckMenu { items, .. } => {
                let this = cx.entity().downgrade();
                let paths: Vec<&'static str> = items.iter().map(|(p, _)| *p).collect();
                self.row_tv(f, v, lw, cx)
                    .control(
                        CheckSelect::new(
                            SharedString::from(id("-menu")),
                            items.iter().map(|(p, l)| (*l, form::get_bool(v, p))),
                            f.menu_summary(v),
                        )
                        .on_toggle(move |i, on, _, cx| {
                            let path = paths[i];
                            this.update(cx, |p, cx| {
                                p.edit(cx, move |d| {
                                    form::set(&mut d.props, path, Value::Bool(on));
                                })
                            })
                            .ok();
                        }),
                    )
                    .into_any_element()
            }
            FieldKind::Number { .. } => {
                let row = self.row_tv(f, v, lw, cx);
                match self.input(&NumberTarget::Path(f.path.clone())) {
                    Some(i) => row.control(tt_ui::Field::new(&i)),
                    None => row,
                }
                .into_any_element()
            }
            FieldKind::Percent => {
                let set = self.setter(cx, f.path.clone());
                let now = form::get_f64(v, &f.path) as f32 * 100.0;
                self.row_tv(f, v, lw, cx)
                    .control(
                        RangeSlider::single(SharedString::from(id("-slider")), 0.0, 100.0, now)
                            .step(1.0)
                            .width(W_OPACITY)
                            .on_change(move |_, x, _, cx| {
                                set(Value::from((x / 100.0).clamp(0.0, 1.0) as f64), cx)
                            }),
                    )
                    .into_any_element()
            }
            FieldKind::FontSize => self
                .row_tv(f, v, lw, cx)
                .control(self.size_select(id(""), f.path.clone(), v, cx))
                .into_any_element(),
            FieldKind::TextStyle => self
                .row_tv(f, v, lw, cx)
                .control(self.swatch(id("-color"), format!("{}/color", f.path), v, cx))
                .control(
                    self.size_select(id("-size"), format!("{}/size", f.path), v, cx)
                        .width(W_SIZE),
                )
                .into_any_element(),
            FieldKind::TextBar => self.text_bar(&f.path, v, cx),
            FieldKind::TextArea => match &self.text {
                Some(area) => div()
                    .py(px(S2))
                    .child(
                        Textarea::new(area)
                            .h(px(C_H_TEXTAREA))
                            .rounded(px(C_R_CONTROL)),
                    )
                    .into_any_element(),
                None => div().into_any_element(),
            },
            FieldKind::Levels => match &d.props {
                Props::Channel(_) => self.channel_levels(d, cx),
                _ => self.fib_levels(d, lw, cx),
            },
        }
    }

    /// `[color][size][B][I]` for the `TextStyle` at `path`.
    fn text_bar(&self, path: &str, v: &Value, cx: &mut Context<Self>) -> AnyElement {
        let Some(ts) =
            form::get(v, path).and_then(|x| serde_json::from_value::<TextStyle>(x.clone()).ok())
        else {
            return div().into_any_element();
        };
        let this = cx.entity().downgrade();
        let path = path.to_owned();
        h_flex()
            .h(px(C_ROW))
            .items_center()
            .child(
                TextStyleBar::new(
                    "tv-text-style",
                    TextStyleValue {
                        color: to_hsla(ts.color),
                        size: ts.size,
                        bold: ts.bold,
                        italic: ts.italic,
                    },
                    &FONT_SIZES,
                )
                .on_change(move |nv, _, cx| {
                    let path = path.clone();
                    this.update(cx, |p, cx| {
                        p.edit(cx, move |d| {
                            let ts = TextStyle {
                                color: from_hsla(nv.color),
                                size: nv.size,
                                bold: nv.bold,
                                italic: nv.italic,
                            };
                            if let Ok(val) = serde_json::to_value(ts) {
                                form::set(&mut d.props, &path, val);
                            }
                        })
                    })
                    .ok();
                }),
            )
            .into_any_element()
    }

    /// The level's `[x]` (or the same space for the channel's own lines)
    /// and coefficient input.
    fn level_head(&self, d: &Drawing, i: usize, cx: &mut Context<Self>) -> Div {
        let on = form::levels(&d.props).get(i).is_some_and(|l| l.on);
        let this = cx.entity().downgrade();
        let check: AnyElement = if form::level::is_main(&d.props, i) {
            div().size(px(C_CHECKBOX)).into_any_element()
        } else {
            CheckBox::new(("tv-level-on", i), "", on)
                .on_change(move |v, _, cx| {
                    this.update(cx, |p, cx| {
                        p.edit(cx, move |d| {
                            form::level::set_on(&mut d.props, i, v);
                        })
                    })
                    .ok();
                })
                .into_any_element()
        };
        h_flex()
            .gap(px(C_GAP))
            .items_center()
            .child(check)
            .children(
                self.input(&NumberTarget::LevelCoeff(i))
                    .map(|inp| tt_ui::Field::new(&inp).width(W_COEFF)),
            )
    }

    /// Channel: one row per level, `[x] [coeff] [line]` (TradingView).
    fn channel_levels(&self, d: &Drawing, cx: &mut Context<Self>) -> AnyElement {
        let mut col = v_flex();
        for i in 0..form::levels(&d.props).len() {
            let s = form::level::stroke(&d.props, i).unwrap_or_default();
            let this = cx.entity().downgrade();
            col = col.child(self.level_head(d, i, cx).h(px(C_ROW)).child(
                LinePicker::new(("tv-level-line", i), line_value(s)).on_change(move |lv, _, cx| {
                    this.update(cx, |p, cx| {
                        p.edit(cx, move |d| {
                            form::level::set_stroke(&mut d.props, i, stroke(lv));
                        })
                    })
                    .ok();
                }),
            ));
        }
        col.into_any_element()
    }

    /// Fib: two columns of `[x] [coeff] [swatch]` filled row by row, then
    /// "Use one color".
    fn fib_levels(&self, d: &Drawing, lw: f32, cx: &mut Context<Self>) -> AnyElement {
        let n = form::levels(&d.props).len();
        let mut grid = v_flex();
        for r in 0..n.div_ceil(2) {
            let mut row = h_flex().h(px(C_ROW)).gap(px(S6)).items_center();
            for i in [2 * r, 2 * r + 1].into_iter().filter(|i| *i < n) {
                let this = cx.entity().downgrade();
                let c = form::level::color(&d.props, i);
                row = row.child(self.level_head(d, i, cx).child(
                    ColorSwatchButton::new(("tv-level-color", i), to_hsla(c)).on_change(
                        move |c, _, cx| {
                            this.update(cx, |p, cx| {
                                p.edit(cx, move |d| {
                                    form::level::set_color(&mut d.props, i, from_hsla(c));
                                })
                            })
                            .ok();
                        },
                    ),
                ));
            }
            grid = grid.child(row);
        }
        let this = cx.entity().downgrade();
        let first = form::level::color(&d.props, 0);
        grid.child(
            FormRow::new("Use one color")
                .comfortable()
                .label_width(lw)
                .control(
                    ColorSwatchButton::new("tv-levels-one-color", to_hsla(first))
                        .tooltip("Use one color")
                        .on_change(move |c, _, cx| {
                            this.update(cx, |p, cx| {
                                p.edit(cx, move |d| {
                                    form::level::set_all_colors(&mut d.props, from_hsla(c));
                                })
                            })
                            .ok();
                        }),
                ),
        )
        .into_any_element()
    }

    /// The Style tab.
    pub(super) fn style_tv(
        &self,
        d: &Drawing,
        v: &Value,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let fields = self.style.clone();
        let mut lw = self.label_column(&fields, window);
        if fields.iter().any(|f| f.kind == FieldKind::Levels) && matches!(d.props, Props::Fib(_)) {
            lw = lw.max(text_w(window, "Use one color") + C_LABEL_GAP);
        }
        let mut b = v_flex();
        for f in &fields {
            b = b.child(self.field_tv(f, d, v, lw, cx));
        }
        b.into_any_element()
    }

    /// The Text tab: `[color][size][B][I]`, the text, "Text alignment" and
    /// the tool's other text options.
    pub(super) fn text_tv(
        &self,
        d: &Drawing,
        v: &Value,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let fields = self.text_fields.clone();
        let lw = self.label_column(&fields, window);
        let mut b = v_flex();
        for f in &fields {
            b = b.child(self.field_tv(f, d, v, lw, cx));
        }
        b.into_any_element()
    }

    /// "#1 (price, bar)" `[price] [bar]` per point; a vertical line shows
    /// the bar only, horizontal lines the price only; a channel its first
    /// two points and "Price offset".
    pub(super) fn coordinates_tv(&self, d: &Drawing, window: &Window) -> AnyElement {
        let rows = form::coordinate_rows(d.tool, d.points.len());
        let labels: Vec<String> = (0..rows)
            .map(|k| match form::coordinate_parts(d.tool, k) {
                (true, true) => form::point_label_price_bar(k),
                (false, true) => format!("#{} (price)", k + 1),
                _ => format!("#{} (bar)", k + 1),
            })
            .collect();
        let offset = self.input(&NumberTarget::PriceOffset);
        let lw = labels
            .iter()
            .map(|l| text_w(window, l))
            .chain(offset.as_ref().map(|_| text_w(window, "Price offset")))
            .fold(0.0, f32::max)
            + C_LABEL_GAP;
        let mut body = v_flex();
        for (k, label) in labels.into_iter().enumerate() {
            let mut row = FormRow::new(SharedString::from(label))
                .comfortable()
                .label_width(lw);
            if let Some(i) = self.input(&NumberTarget::PointPrice(k)) {
                row = row.control(tt_ui::Field::new(&i));
            }
            if let Some(i) = self.input(&NumberTarget::PointBar(k)) {
                row = row.control(tt_ui::Field::new(&i));
            }
            body = body.child(row);
        }
        if let Some(i) = offset {
            body = body.child(
                FormRow::new("Price offset")
                    .comfortable()
                    .label_width(lw)
                    .control(tt_ui::Field::new(&i)),
            );
        }
        body.into_any_element()
    }

    /// Ticks, then each unit `[x] Label [min] ——o==o—— [max]`, then Ranges.
    pub(super) fn visibility_tv(&self, d: &Drawing, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity().downgrade();
        let check =
            |id: &'static str, label: &'static str, on: bool, f: fn(&mut Visibility, bool)| {
                let t = this.clone();
                FormRow::check(id, label, on, move |on, _, cx| {
                    t.update(cx, |p, cx| p.edit(cx, move |d| f(&mut d.visibility, on)))
                        .ok();
                })
                .comfortable()
            };
        let mut body = v_flex().child(check(
            "tv-vis-ticks",
            "Ticks",
            d.visibility.ticks,
            |v, on| v.ticks = on,
        ));
        for u in Visibility::UNITS {
            let span = d.visibility.span(u);
            let t = this.clone();
            let mut row = FormRow::check(
                SharedString::from(format!("tv-vis-{}", form::unit_label(u))),
                form::unit_label(u),
                span.on,
                move |on, _, cx| {
                    t.update(cx, |p, cx| {
                        p.edit(cx, move |d| d.visibility.span_mut(u).on = on)
                    })
                    .ok();
                },
            )
            .comfortable()
            .label_width(W_VIS_LABEL);
            if let (Some(lo), Some(hi)) = (
                self.input(&NumberTarget::VisMin(u)),
                self.input(&NumberTarget::VisMax(u)),
            ) {
                let t = this.clone();
                row = row.control(
                    RangeField::new(
                        SharedString::from(format!("tv-vis-range-{}", form::unit_label(u))),
                        &lo,
                        &hi,
                        (1.0, Visibility::row_max(u) as f32),
                        (span.min as f32, span.max as f32),
                    )
                    .on_change(move |a, b, _, cx| {
                        let (a, b) = (a.round() as u16, b.round() as u16);
                        t.update(cx, |p, cx| {
                            p.edit(cx, move |d| {
                                let s = d.visibility.span_mut(u);
                                s.min = a.min(b).max(1);
                                s.max = b.max(a);
                            })
                        })
                        .ok();
                    }),
                );
            }
            body = body.child(row);
        }
        body.child(check(
            "tv-vis-ranges",
            "Ranges",
            d.visibility.ranges,
            |v, on| v.ranges = on,
        ))
        .into_any_element()
    }
}
