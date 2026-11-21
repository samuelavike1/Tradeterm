//! The TradingView-measured ("comfortable", ui-guidelines §2.10) layout of
//! the drawing settings dialog: the reference is the horizontal ray.
//!
//! A tool opts in through `tt_chart::drawings::form::dialog_layout`; its
//! Style fields then render with the comfortable kit (one `LinePicker` per
//! line, 18 px checkboxes, 34 px fields), the Text tab becomes
//! `[color][size][B][I]` / text area / "Text alignment" on one row (no
//! "Show text": text shows when there is some), Coordinates shows price
//! and bar number, and Visibility gets Ticks / Ranges and range sliders.
//! Fields this layout does not know fall back to the generic renderer.

use super::{ColorTarget, DrawingSettingsPanel, NumberTarget, from_hsla, to_hsla};
use gpui_kit::component::input::Textarea;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use serde_json::Value;
use tt_chart::drawings::form::{self, FONT_SIZES, Field, FieldKind};
use tt_chart::drawings::props::{HAlign, VAlign};
use tt_chart::{Drawing, LineStyle, Visibility};
use tt_ui::prelude::*;
use tt_ui::tokens::*;
use tt_ui::{LineDash, LinePicker, LineValue, TextStyleBar, TextStyleValue};

/// Visibility rows' label column (TradingView: label + 20).
const W_VIS_LABEL: f32 = 96.0;

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

impl DrawingSettingsPanel {
    /// One Style field in the comfortable layout (generic fallback).
    pub(super) fn field_tv(
        &self,
        f: &Field,
        d: &Drawing,
        v: &Value,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = |suffix: &str| format!("tv{}{suffix}", f.path.replace('/', "-"));
        let row = || {
            let r = FormRow::new(f.label).comfortable().label_auto();
            if f.indent { r.indent() } else { r }
        };
        match &f.kind {
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
            FieldKind::Line if f.check.is_none() => {
                let s = form::get_stroke(v, &f.path).unwrap_or_default();
                let path = f.path.clone();
                let this = cx.entity().downgrade();
                row()
                    .control(
                        LinePicker::new(
                            SharedString::from(id("-picker")),
                            LineValue {
                                color: to_hsla(s.color),
                                width: s.width,
                                dash: dash(s.style),
                            },
                        )
                        .on_change(move |lv, _, cx| {
                            let path = path.clone();
                            this.update(cx, |p, cx| {
                                p.edit(cx, move |d| {
                                    let stroke = tt_chart::drawings::props::Stroke {
                                        color: from_hsla(lv.color),
                                        width: lv.width.clamp(1, 4),
                                        style: line_style(lv.dash),
                                    };
                                    if let Ok(val) = serde_json::to_value(stroke) {
                                        form::set(&mut d.props, &path, val);
                                    }
                                })
                            })
                            .ok();
                        }),
                    )
                    .into_any_element()
            }
            FieldKind::Choice(options) => {
                let set = self.setter(cx, f.path.clone());
                let now = form::get_choice(v, &f.path);
                row()
                    .control(
                        Select::new(
                            SharedString::from(id("")),
                            options.iter().map(|(val, label)| (val.to_string(), *label)),
                            now,
                        )
                        .comfortable()
                        .on_change(move |val: String, _, cx| set(form::choice_value(&val), cx)),
                    )
                    .into_any_element()
            }
            _ => self.field(f, d, v, cx),
        }
    }

    pub(super) fn style_tv(&self, d: &Drawing, v: &Value, cx: &mut Context<Self>) -> AnyElement {
        let fields = self.style.clone();
        let mut b = v_flex();
        for f in &fields {
            b = b.child(self.field_tv(f, d, v, cx));
        }
        b.into_any_element()
    }

    /// `[color][size][B][I]`, the text, "Text alignment" `[vertical]
    /// [horizontal]`.
    pub(super) fn text_tv(&self, v: &Value, cx: &mut Context<Self>) -> AnyElement {
        let mut b = v_flex();
        let ts = form::get(v, "/text_style").and_then(|x| {
            serde_json::from_value::<tt_chart::drawings::props::TextStyle>(x.clone()).ok()
        });
        if let Some(ts) = ts {
            let this = cx.entity().downgrade();
            b = b.child(
                h_flex().h(px(C_ROW)).items_center().child(
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
                        this.update(cx, |p, cx| {
                            p.edit(cx, move |d| {
                                let ts = tt_chart::drawings::props::TextStyle {
                                    color: from_hsla(nv.color),
                                    size: nv.size,
                                    bold: nv.bold,
                                    italic: nv.italic,
                                };
                                if let Ok(val) = serde_json::to_value(ts) {
                                    form::set(&mut d.props, "/text_style", val);
                                }
                            })
                        })
                        .ok();
                    }),
                ),
            );
        }
        if let Some(area) = &self.text {
            b = b.child(
                div().py(px(S2)).child(
                    Textarea::new(area)
                        .h(px(C_H_TEXTAREA))
                        .rounded(px(C_R_CONTROL)),
                ),
            );
        }
        if form::get(v, "/text/h").is_some() {
            let (set_v, set_h) = (
                self.setter(cx, "/text/v".into()),
                self.setter(cx, "/text/h".into()),
            );
            let now_v = form::get_choice(v, "/text/v");
            let now_h = form::get_choice(v, "/text/h");
            let vs = [VAlign::Middle, VAlign::Top, VAlign::Bottom].map(|a| {
                let s = format!("{a:?}");
                (s.clone(), s)
            });
            let hs = [HAlign::Center, HAlign::Left, HAlign::Right].map(|a| {
                let s = format!("{a:?}");
                (s.clone(), s)
            });
            b = b.child(
                FormRow::new("Text alignment")
                    .comfortable()
                    .label_auto()
                    .control(
                        Select::new("tv-text-v", vs, now_v)
                            .comfortable()
                            .on_change(move |s: String, _, cx| set_v(Value::String(s), cx)),
                    )
                    .control(
                        Select::new("tv-text-h", hs, now_h)
                            .comfortable()
                            .on_change(move |s: String, _, cx| set_h(Value::String(s), cx)),
                    ),
            );
        }
        // Fields beyond color / size / B / I / text / alignment (vertical
        // line orientation, …) keep their generic rows.
        let extra: Vec<Field> = self
            .text_fields
            .iter()
            .filter(|f| {
                !matches!(
                    f.path.as_str(),
                    "/text/on" | "/text_style" | "/text/h" | "/text/v"
                )
            })
            .cloned()
            .collect();
        let d = self.current(cx);
        if let Some(d) = d {
            for f in &extra {
                b = b.child(self.field_tv(f, &d, v, cx));
            }
        }
        b.into_any_element()
    }

    /// "#1 (price, bar)" `[price] [bar]` per point.
    pub(super) fn coordinates_tv(&self, d: &Drawing) -> AnyElement {
        let mut body = v_flex();
        for k in 0..d.points.len() {
            let (time, price) = form::coordinate_parts(d.tool, k);
            let label = match (price, time) {
                (true, true) => form::point_label_price_bar(k),
                (true, false) => format!("#{} (price)", k + 1),
                _ => format!("#{} (bar)", k + 1),
            };
            let mut row = FormRow::new(SharedString::from(label))
                .comfortable()
                .label_auto();
            if let Some(i) = self.input(&NumberTarget::PointPrice(k)) {
                row = row.control(tt_ui::Field::new(&i));
            }
            if let Some(i) = self.input(&NumberTarget::PointBar(k)) {
                row = row.control(tt_ui::Field::new(&i));
            }
            body = body.child(row);
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

    /// The color pickers this layout does not use (it edits colors through
    /// the line picker / text bar), so `build_controls` can skip them.
    pub(super) fn tv_skips_color(t: &ColorTarget) -> bool {
        matches!(t, ColorTarget::Path(p) if p.ends_with("/color") && (p.starts_with("/line") || p.starts_with("/text_style")))
    }
}
