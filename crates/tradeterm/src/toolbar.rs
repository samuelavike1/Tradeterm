//! Toolbar widgets: frameless text buttons, dividers and the chart-type
//! picker with painted icons.

use crate::icons::{self, Icon};
use egui::{
    Color32, CornerRadius, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui,
    pos2, vec2,
};
use tt_chart::ChartType;
use tt_theme::{Theme, mix, with_alpha};

pub const TOOLBAR_HEIGHT: f32 = 30.0;

/// Background of the symbol pill.
pub fn pill_fill(theme: &Theme) -> Color32 {
    mix(theme.panel_alt, theme.border, 0.6)
}

/// Frameless buttons that show a rounded background only on hover or when
/// selected (selection uses the theme's neutral `selection` token).
pub fn apply_style(ui: &mut Ui, theme: &Theme) {
    let style = ui.style_mut();
    style.spacing.item_spacing.x = 2.0;
    style.spacing.button_padding = vec2(8.0, 4.0);
    style.spacing.interact_size.y = TOOLBAR_HEIGHT - 4.0;
    let w = &mut style.visuals.widgets;
    w.inactive.weak_bg_fill = Color32::TRANSPARENT;
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.fg_stroke = Stroke::new(1.0, theme.text);
    w.hovered.weak_bg_fill = theme.border;
    w.hovered.bg_stroke = Stroke::NONE;
    w.active.bg_stroke = Stroke::NONE;
    w.open.weak_bg_fill = theme.border;
    w.open.bg_stroke = Stroke::NONE;
    for v in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        v.corner_radius = CornerRadius::same(5);
    }
    style.visuals.selection.stroke = Stroke::new(1.0, theme.text);
}

/// TradingView-style ticker pill: magnifier + symbol; opens the search.
pub fn ticker_button(ui: &mut Ui, symbol: &str, theme: &Theme) -> Response {
    let font = egui::FontId::proportional(14.0);
    let galley = ui
        .ctx()
        .fonts_mut(|f| f.layout_no_wrap(symbol.to_owned(), font, theme.text));
    let width = (galley.size().x + 50.0).max(130.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, TOOLBAR_HEIGHT), Sense::click());
    let fill = if resp.hovered() {
        mix(pill_fill(theme), theme.border, 0.6)
    } else {
        pill_fill(theme)
    };
    let p = ui.painter();
    p.rect_filled(rect, rect.height() / 2.0, fill);
    let icon_c = pos2(rect.left() + 16.0, rect.center().y);
    icons::paint(p, icon_c, Icon::Search, 15.0, theme.text_muted);
    p.galley(
        pos2(rect.left() + 30.0, rect.center().y - galley.size().y / 2.0),
        galley,
        theme.text,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A thin vertical separator between button groups.
pub fn divider(ui: &mut Ui, theme: &Theme) {
    ui.add_space(6.0);
    let (rect, _) = ui.allocate_exact_size(vec2(1.0, 20.0), Sense::hover());
    ui.painter().vline(
        rect.center().x,
        rect.y_range(),
        Stroke::new(1.0, theme.border),
    );
    ui.add_space(6.0);
}

/// A square button that paints its own content; rounded background on hover
/// or when `selected`.
pub fn icon_button(
    ui: &mut Ui,
    width: f32,
    selected: bool,
    theme: &Theme,
    paint: impl FnOnce(&Painter, Rect, Color32),
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, TOOLBAR_HEIGHT - 2.0), Sense::click());
    let bg = if selected {
        theme.selection
    } else if resp.hovered() {
        theme.border
    } else {
        Color32::TRANSPARENT
    };
    let p = ui.painter();
    p.rect_filled(rect, 5.0, bg);
    let fg = if selected || resp.hovered() {
        theme.text
    } else {
        mix(theme.text, theme.text_muted, 0.4)
    };
    paint(p, rect, fg);
    resp
}

/// A Lucide chevron-down centered at `c`.
pub fn chevron(p: &Painter, c: Pos2, color: Color32) {
    icons::paint(p, c, Icon::ChevronDown, 14.0, color);
}

/// A toolbar toggle showing a Lucide icon; highlighted while `on`.
pub fn icon_toggle(ui: &mut Ui, on: &mut bool, icon: Icon, theme: &Theme) -> Response {
    let mut resp = icon_button(ui, 32.0, *on, theme, |p, rect, fg| {
        icons::paint(p, rect.center(), icon, 17.0, fg);
    });
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    resp
}

/// Draws a 18×18-ish pictogram of `ty` inside `rect`.
pub fn chart_type_icon(p: &Painter, rect: Rect, ty: ChartType, color: Color32) {
    let c = rect.center();
    let stroke = Stroke::new(1.3, color);
    // Two candles: (x offset, wick top, body top, body bottom, wick bottom).
    let candles = [(-3.5, -8.0, -4.0, 3.0, 6.0), (3.5, -5.0, -1.0, 5.0, 8.0)];
    match ty {
        ChartType::Candles | ChartType::HollowCandles | ChartType::HeikinAshi => {
            for (i, (dx, wt, bt, bb, wb)) in candles.into_iter().enumerate() {
                let x = c.x + dx;
                p.vline(x, (c.y + wt)..=(c.y + bt), stroke);
                p.vline(x, (c.y + bb)..=(c.y + wb), stroke);
                let body = Rect::from_min_max(pos2(x - 2.5, c.y + bt), pos2(x + 2.5, c.y + bb));
                let hollow = ty == ChartType::HollowCandles && i == 1;
                if hollow {
                    p.rect_stroke(body, 0.5, stroke, StrokeKind::Inside);
                } else {
                    p.rect_filled(body, 0.5, color);
                }
            }
            if ty == ChartType::HeikinAshi {
                // A smoothing arc over the candles.
                p.add(Shape::line(
                    vec![
                        c + vec2(-8.0, 0.0),
                        c + vec2(-2.0, -6.0),
                        c + vec2(8.0, -9.0),
                    ],
                    Stroke::new(1.0, with_alpha(color, 0.7)),
                ));
            }
        }
        ChartType::Bars => {
            for (dx, wt, bt, bb, wb) in candles {
                let x = c.x + dx;
                p.vline(x, (c.y + wt)..=(c.y + wb), stroke);
                p.hline((x - 3.0)..=x, c.y + bt, stroke);
                p.hline(x..=(x + 3.0), c.y + bb, stroke);
            }
        }
        ChartType::Line | ChartType::Area => {
            let pts = vec![
                c + vec2(-8.0, 5.0),
                c + vec2(-3.0, -1.0),
                c + vec2(1.0, 2.0),
                c + vec2(8.0, -6.0),
            ];
            if ty == ChartType::Area {
                // Hatch under the line to suggest the fill.
                for (a, b) in pts.iter().zip(pts.iter().skip(1)) {
                    let steps = ((b.x - a.x) / 2.0).max(1.0) as i32;
                    for k in 0..steps {
                        let t = k as f32 / steps as f32;
                        let x = a.x + (b.x - a.x) * t;
                        let y = a.y + (b.y - a.y) * t;
                        p.vline(
                            x,
                            (y + 1.5)..=(c.y + 8.0),
                            Stroke::new(1.0, with_alpha(color, 0.35)),
                        );
                    }
                }
            }
            p.add(Shape::line(pts, stroke));
        }
        ChartType::VolumeCandles => {
            // A thin and a wide candle.
            for (dx, half_w, top, bot) in [(-4.5, 1.5, -6.0, 3.0), (3.0, 3.5, -2.0, 6.0)] {
                let x = c.x + dx;
                p.vline(x, (c.y + top - 2.5)..=(c.y + bot + 2.5), stroke);
                p.rect_filled(
                    Rect::from_min_max(pos2(x - half_w, c.y + top), pos2(x + half_w, c.y + bot)),
                    0.5,
                    color,
                );
            }
        }
        ChartType::LineWithMarkers => {
            let pts = [
                c + vec2(-8.0, 4.0),
                c + vec2(-3.0, -2.0),
                c + vec2(2.0, 2.0),
                c + vec2(8.0, -5.0),
            ];
            p.add(Shape::line(pts.to_vec(), stroke));
            for q in pts {
                p.circle_filled(q, 1.8, color);
            }
        }
        ChartType::StepLine => {
            let pts = vec![
                c + vec2(-9.0, 5.0),
                c + vec2(-4.0, 5.0),
                c + vec2(-4.0, -1.0),
                c + vec2(2.0, -1.0),
                c + vec2(2.0, -6.0),
                c + vec2(9.0, -6.0),
            ];
            p.add(Shape::line(pts, stroke));
        }
        ChartType::HlcArea => {
            let hi = vec![
                c + vec2(-9.0, -3.0),
                c + vec2(-2.0, -7.0),
                c + vec2(9.0, -4.0),
            ];
            let lo = vec![c + vec2(-9.0, 5.0), c + vec2(-2.0, 2.0), c + vec2(9.0, 6.0)];
            let mid = vec![
                c + vec2(-9.0, 1.0),
                c + vec2(-2.0, -2.5),
                c + vec2(9.0, 1.0),
            ];
            p.add(Shape::line(hi, Stroke::new(1.1, color)));
            p.add(Shape::line(lo, Stroke::new(1.1, color)));
            p.add(Shape::line(mid, Stroke::new(1.0, with_alpha(color, 0.6))));
        }
        ChartType::Baseline => {
            p.add(Shape::dashed_line(
                &[c + vec2(-9.0, 0.0), c + vec2(9.0, 0.0)],
                Stroke::new(1.0, with_alpha(color, 0.6)),
                2.0,
                2.0,
            ));
            let pts = vec![
                c + vec2(-9.0, 5.0),
                c + vec2(-3.0, 3.0),
                c + vec2(2.0, -4.0),
                c + vec2(9.0, -6.0),
            ];
            p.add(Shape::line(pts, stroke));
        }
        ChartType::Columns => {
            for (dx, h) in [(-6.0, 6.0), (0.0, 11.0), (6.0, 8.0)] {
                p.rect_filled(
                    Rect::from_min_max(
                        pos2(c.x + dx - 2.0, c.y + 7.0 - h),
                        pos2(c.x + dx + 2.0, c.y + 7.0),
                    ),
                    0.5,
                    color,
                );
            }
        }
        ChartType::HighLow => {
            for (dx, top, bot) in [(-4.0, -7.0, 3.0), (4.0, -3.0, 7.0)] {
                p.rect_stroke(
                    Rect::from_min_max(
                        pos2(c.x + dx - 2.5, c.y + top),
                        pos2(c.x + dx + 2.5, c.y + bot),
                    ),
                    0.5,
                    stroke,
                    StrokeKind::Inside,
                );
            }
        }
        ChartType::Renko => {
            // A staircase of equal bricks.
            for k in 0..3 {
                let x = c.x - 8.0 + k as f32 * 5.5;
                let y = c.y + 3.0 - k as f32 * 4.5;
                p.rect_stroke(
                    Rect::from_min_size(pos2(x, y), vec2(5.0, 4.5)),
                    0.0,
                    stroke,
                    StrokeKind::Inside,
                );
            }
        }
        ChartType::LineBreak => {
            for (k, (top, bot)) in [(-1.0, 6.0), (-7.0, -1.0), (-4.0, 3.0)].iter().enumerate() {
                let x = c.x - 8.0 + k as f32 * 5.5;
                let r = Rect::from_min_max(pos2(x, c.y + top), pos2(x + 5.0, c.y + bot));
                if k == 2 {
                    p.rect_filled(r, 0.0, color);
                } else {
                    p.rect_stroke(r, 0.0, stroke, StrokeKind::Inside);
                }
            }
        }
    }
}

/// Chart types like TradingView: one quick icon button per favorite type,
/// then a button showing the current type that opens the grouped menu. In the
/// menu a star toggles favorites; hovering a row explains the type.
pub fn chart_type_picker(
    ui: &mut Ui,
    current: &mut ChartType,
    favorites: &mut Vec<ChartType>,
    theme: &Theme,
) {
    for fav in favorites.clone() {
        if fav == *current {
            continue;
        }
        let r = icon_button(ui, 32.0, false, theme, |p, rect, fg| {
            chart_type_icon(
                p,
                Rect::from_center_size(rect.center(), vec2(20.0, 20.0)),
                fav,
                fg,
            );
        })
        .on_hover_text(fav.label());
        if r.clicked() {
            *current = fav;
        }
    }

    let ty = *current;
    let resp = icon_button(ui, 44.0, false, theme, |p, rect, fg| {
        let icon = Rect::from_center_size(rect.center() - vec2(7.0, 0.0), vec2(20.0, 20.0));
        chart_type_icon(p, icon, ty, fg);
        chevron(p, pos2(rect.right() - 9.0, rect.center().y), fg);
    })
    .on_hover_text(format!("Chart type: {}", ty.label()));

    egui::Popup::menu(&resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            const W: f32 = 230.0;
            ui.set_width(W);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            for (g, group) in ChartType::GROUPS.iter().enumerate() {
                if g > 0 {
                    ui.add_space(4.0);
                    let y = ui.cursor().top();
                    ui.painter()
                        .hline(ui.max_rect().x_range(), y, Stroke::new(1.0, theme.border));
                    ui.add_space(4.0);
                }
                for &t in group.iter() {
                    match chart_type_row(ui, t, t == *current, favorites.contains(&t), theme, W) {
                        RowClick::Select => {
                            *current = t;
                            ui.close();
                        }
                        RowClick::Star => {
                            if let Some(i) = favorites.iter().position(|f| *f == t) {
                                favorites.remove(i);
                            } else {
                                favorites.push(t);
                                // Keep TradingView's menu order on the toolbar.
                                favorites
                                    .sort_by_key(|f| ChartType::ALL.iter().position(|a| a == f));
                            }
                        }
                        RowClick::None => {}
                    }
                }
            }
        });
}

enum RowClick {
    None,
    Select,
    Star,
}

fn chart_type_row(
    ui: &mut Ui,
    t: ChartType,
    selected: bool,
    favorite: bool,
    theme: &Theme,
    w: f32,
) -> RowClick {
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
    let resp = resp.on_hover_text(t.description());
    let star_rect =
        Rect::from_center_size(pos2(rect.right() - 18.0, rect.center().y), vec2(22.0, 22.0));
    let star = ui.interact(
        star_rect,
        ui.make_persistent_id(("ct_star", t.label())),
        Sense::click(),
    );
    let hovered = resp.hovered() || star.hovered();
    let (bg, fg) = if selected {
        (theme.text, theme.bg)
    } else if hovered {
        (theme.selection, theme.text)
    } else {
        (Color32::TRANSPARENT, theme.text)
    };
    let p = ui.painter();
    p.rect_filled(rect.shrink2(vec2(4.0, 1.0)), 5.0, bg);
    chart_type_icon(
        p,
        Rect::from_center_size(pos2(rect.left() + 22.0, rect.center().y), vec2(20.0, 20.0)),
        t,
        fg,
    );
    p.text(
        pos2(rect.left() + 42.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        t.label(),
        egui::FontId::proportional(13.5),
        fg,
    );
    if favorite {
        crate::tfmenu::filled_star(p, star_rect.center(), 6.5, fg);
    } else if hovered {
        let c = if star.hovered() {
            fg
        } else {
            with_alpha(fg, 0.6)
        };
        icons::paint(p, star_rect.center(), Icon::Star, 14.0, c);
    }
    if star.clicked() {
        RowClick::Star
    } else if resp.clicked() {
        RowClick::Select
    } else {
        RowClick::None
    }
}
