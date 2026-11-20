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

/// A Lucide magnifier centered at `c`.
pub fn search_icon(p: &Painter, c: Pos2, color: Color32) {
    icons::paint(p, c, Icon::Search, 15.0, color);
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
    }
}

/// Icon button showing the current chart type plus a chevron; opens a menu
/// with every type.
pub fn chart_type_picker(ui: &mut Ui, current: &mut ChartType, theme: &Theme) {
    let ty = *current;
    let resp = icon_button(ui, 44.0, false, theme, |p, rect, fg| {
        let icon = Rect::from_center_size(rect.center() - vec2(7.0, 0.0), vec2(20.0, 20.0));
        chart_type_icon(p, icon, ty, fg);
        chevron(p, pos2(rect.right() - 9.0, rect.center().y), fg);
    })
    .on_hover_text(format!("Chart type: {}", ty.label()));

    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(170.0);
        for t in ChartType::ALL {
            let (rect, row) = ui.allocate_exact_size(vec2(170.0, 28.0), Sense::click());
            let selected = t == *current;
            let bg = if selected {
                theme.selection
            } else if row.hovered() {
                theme.border
            } else {
                Color32::TRANSPARENT
            };
            let p = ui.painter();
            p.rect_filled(rect, 4.0, bg);
            let icon =
                Rect::from_center_size(pos2(rect.left() + 16.0, rect.center().y), vec2(20.0, 20.0));
            chart_type_icon(p, icon, t, theme.text);
            p.text(
                pos2(rect.left() + 34.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                t.label(),
                egui::FontId::proportional(13.0),
                theme.text,
            );
            if row.clicked() {
                *current = t;
                ui.close();
            }
        }
    });
}
