//! Custom window chrome: the window is created without OS decorations, so
//! this module draws the tab strip's window buttons, makes empty title-bar
//! space draggable and adds resize handles along the window edges.

use egui::{
    Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Rect, ResizeDirection, Response, Sense,
    Stroke, StrokeKind, Ui, ViewportCommand, pos2, vec2,
};
use tt_theme::{Theme, mix};

pub const TITLE_BAR_HEIGHT: f32 = 38.0;
/// Width of the invisible resize band along each window edge.
const RESIZE_BAND: f32 = 5.0;

pub fn is_maximized(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.viewport().maximized.unwrap_or(false))
}

/// How far a maximized undecorated window extends past the monitor on each
/// side (Windows places it ~8 px off-screen, the size of the hidden resize
/// frame). Content must be inset by this much or its edges are cut off.
pub fn maximized_overhang(ctx: &egui::Context) -> f32 {
    ctx.input(|i| {
        let v = i.viewport();
        match (v.maximized, v.inner_rect, v.monitor_size) {
            (Some(true), Some(inner), Some(monitor)) if inner.width() > monitor.x => {
                ((inner.width() - monitor.x) / 2.0).ceil()
            }
            _ => 0.0,
        }
    })
}

pub fn toggle_maximize(ctx: &egui::Context) {
    ctx.send_viewport_cmd(ViewportCommand::Maximized(!is_maximized(ctx)));
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WindowButton {
    Minimize,
    Maximize,
    Restore,
    Close,
}

/// A 46-px wide caption button, drawn like the Windows 11 ones.
pub fn window_button(ui: &mut Ui, kind: WindowButton, theme: &Theme) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(46.0, TITLE_BAR_HEIGHT), Sense::click());
    let hovered = resp.hovered();
    let (bg, fg) = match kind {
        WindowButton::Close if hovered => (theme.down, theme.text_on_accent),
        _ if hovered => (theme.border, theme.text),
        _ => (Color32::TRANSPARENT, theme.text_muted),
    };
    let p = ui.painter();
    p.rect_filled(rect, 0.0, bg);
    let c = rect.center();
    let s = 5.0;
    let stroke = Stroke::new(1.0, fg);
    match kind {
        WindowButton::Minimize => {
            p.hline((c.x - s)..=(c.x + s), c.y, stroke);
        }
        WindowButton::Maximize => {
            p.rect_stroke(
                Rect::from_center_size(c, vec2(2.0 * s, 2.0 * s)),
                1.0,
                stroke,
                StrokeKind::Inside,
            );
        }
        WindowButton::Restore => {
            let back = Rect::from_center_size(c + vec2(2.0, -2.0), vec2(8.0, 8.0));
            let front = Rect::from_center_size(c + vec2(-1.0, 1.0), vec2(8.0, 8.0));
            p.rect_stroke(back, 1.0, stroke, StrokeKind::Inside);
            p.rect_filled(front, 1.0, if hovered { bg } else { theme.chrome });
            p.rect_stroke(front, 1.0, stroke, StrokeKind::Inside);
        }
        WindowButton::Close => {
            p.line_segment([c + vec2(-s, -s), c + vec2(s, s)], stroke);
            p.line_segment([c + vec2(-s, s), c + vec2(s, -s)], stroke);
        }
    }
    resp
}

/// A painted gear button (the ⚙ character comes from a color-emoji font and
/// would ignore the theme's text color).
pub fn settings_button(ui: &mut Ui, theme: &Theme) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(36.0, TITLE_BAR_HEIGHT), Sense::click());
    let p = ui.painter();
    let bg_rect = Rect::from_center_size(rect.center(), vec2(30.0, 28.0));
    if resp.hovered() || egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp))
    {
        p.rect_filled(bg_rect, 5.0, theme.border);
    }
    let color = if resp.hovered() {
        theme.text
    } else {
        theme.text_muted
    };
    let c = rect.center();
    let stroke = Stroke::new(1.5, color);
    // Eight teeth around a ring, with a hole in the middle.
    for k in 0..8 {
        let a = k as f32 * std::f32::consts::TAU / 8.0;
        let dir = vec2(a.cos(), a.sin());
        p.line_segment([c + dir * 5.0, c + dir * 7.5], Stroke::new(2.4, color));
    }
    p.circle_stroke(c, 5.0, stroke);
    p.circle_stroke(c, 2.0, stroke);
    resp
}

/// Minimize / maximize-restore / close, right to left. Call inside a
/// right-to-left layout.
pub fn window_controls(ui: &mut Ui, theme: &Theme) {
    let ctx = ui.ctx().clone();
    if window_button(ui, WindowButton::Close, theme)
        .on_hover_text("Close")
        .clicked()
    {
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }
    let (kind, tip) = if is_maximized(&ctx) {
        (WindowButton::Restore, "Restore")
    } else {
        (WindowButton::Maximize, "Maximize")
    };
    if window_button(ui, kind, theme).on_hover_text(tip).clicked() {
        toggle_maximize(&ctx);
    }
    if window_button(ui, WindowButton::Minimize, theme)
        .on_hover_text("Minimize")
        .clicked()
    {
        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
    }
}

/// Makes `rect` behave like a native title bar: drag to move (with OS snap),
/// double-click to maximize/restore.
pub fn drag_region(ui: &mut Ui, rect: Rect) {
    if rect.width() <= 0.0 {
        return;
    }
    let resp = ui.interact(rect, Id::unique("title_bar_drag"), Sense::click_and_drag());
    if resp.double_clicked() {
        toggle_maximize(ui.ctx());
    } else if resp.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

/// What the user did with a tab.
#[derive(Default)]
pub struct TabAction {
    pub select: bool,
    pub close: bool,
}

pub struct TabView<'a> {
    pub label: &'a str,
    pub badge: &'a str,
    pub badge_color: Color32,
    /// Change in percent, if known.
    pub change: Option<f64>,
    pub active: bool,
    /// Whether the close button may be shown (not for the last tab).
    pub closable: bool,
}

/// A browser-style tab: badge, symbol, change % and a close button.
pub fn tab(ui: &mut Ui, id: Id, v: &TabView<'_>, theme: &Theme) -> TabAction {
    let label_font = FontId::proportional(13.0);
    let change_font = FontId::proportional(12.0);
    let change_text = v.change.map(|c| {
        let arrow = if c >= 0.0 { "▲" } else { "▼" };
        format!("{arrow} {c:+.2}%")
    });
    let (label_w, change_w) = ui.ctx().fonts_mut(|f| {
        let lw = f
            .layout_no_wrap(v.label.to_owned(), label_font.clone(), theme.text)
            .size()
            .x;
        let cw = change_text.as_ref().map_or(0.0, |t| {
            f.layout_no_wrap(t.clone(), change_font.clone(), theme.text)
                .size()
                .x
        });
        (lw, cw)
    });
    let width =
        (12.0 + 18.0 + 8.0 + label_w + 8.0 + change_w + 8.0 + 16.0 + 10.0).clamp(120.0, 260.0);

    let (outer, resp) = ui.allocate_exact_size(vec2(width, TITLE_BAR_HEIGHT), Sense::click());
    let rect = Rect::from_min_max(outer.min + vec2(0.0, 6.0), outer.max);
    let close_rect =
        Rect::from_center_size(pos2(rect.right() - 16.0, rect.center().y), vec2(18.0, 18.0));
    let close_resp = ui.interact(close_rect, id.with("close"), Sense::click());

    let hovered = resp.hovered() || close_resp.hovered();
    let bg = if v.active {
        theme.tab_active
    } else if hovered {
        mix(theme.chrome, theme.tab_active, 0.5)
    } else {
        Color32::TRANSPARENT
    };
    let p = ui.painter();
    p.rect_filled(
        rect,
        CornerRadius {
            nw: 7,
            ne: 7,
            sw: 0,
            se: 0,
        },
        bg,
    );

    // Badge with the base asset's initial.
    let badge_c = pos2(rect.left() + 12.0 + 9.0, rect.center().y);
    p.circle_filled(badge_c, 9.0, v.badge_color);
    p.text(
        badge_c,
        Align2::CENTER_CENTER,
        v.badge,
        FontId::proportional(10.5),
        theme.text_on_accent,
    );

    let mut x = badge_c.x + 9.0 + 8.0;
    let text_color = if v.active {
        theme.text
    } else {
        theme.text_muted
    };
    let r = p.text(
        pos2(x, rect.center().y),
        Align2::LEFT_CENTER,
        v.label,
        label_font,
        text_color,
    );
    x = r.right() + 8.0;
    if let (Some(c), Some(t)) = (v.change, &change_text) {
        let color = if c >= 0.0 { theme.up } else { theme.down };
        p.text(
            pos2(x, rect.center().y),
            Align2::LEFT_CENTER,
            t,
            change_font,
            color,
        );
    }

    let show_close = v.closable && (hovered || v.active);
    if show_close {
        if close_resp.hovered() {
            p.rect_filled(close_rect, 4.0, theme.border);
        }
        let c = close_rect.center();
        let s = 3.5;
        let stroke = Stroke::new(1.2, theme.text_muted);
        p.line_segment([c + vec2(-s, -s), c + vec2(s, s)], stroke);
        p.line_segment([c + vec2(-s, s), c + vec2(s, -s)], stroke);
    }

    let close = v.closable
        && ((show_close && close_resp.clicked()) || resp.clicked_by(egui::PointerButton::Middle));
    TabAction {
        select: !close && (resp.clicked() || close_resp.clicked() && !show_close),
        close,
    }
}

/// A small "+" button for the tab strip.
pub fn new_tab_button(ui: &mut Ui, theme: &Theme) -> Response {
    let (outer, resp) = ui.allocate_exact_size(vec2(34.0, TITLE_BAR_HEIGHT), Sense::click());
    let rect = Rect::from_center_size(outer.center() + vec2(0.0, 3.0), vec2(26.0, 26.0));
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, 5.0, theme.border);
    }
    let c = rect.center();
    let stroke = Stroke::new(1.3, theme.text_muted);
    p.hline((c.x - 5.0)..=(c.x + 5.0), c.y, stroke);
    p.vline(c.x, (c.y - 5.0)..=(c.y + 5.0), stroke);
    resp.on_hover_text("New tab (Ctrl+T)")
}

/// Invisible bands along the window edges that start an OS resize.
/// Call last in the frame so they sit on top of other widgets.
pub fn resize_handles(ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    if is_maximized(&ctx) {
        return;
    }
    let r = ctx.viewport_rect();
    let b = RESIZE_BAND;
    let (l, t, rt, bt) = (r.left(), r.top(), r.right(), r.bottom());
    let bands = [
        (
            Rect::from_min_max(pos2(l, t), pos2(l + b, t + b)),
            ResizeDirection::NorthWest,
            CursorIcon::ResizeNwSe,
        ),
        (
            Rect::from_min_max(pos2(rt - b, t), pos2(rt, t + b)),
            ResizeDirection::NorthEast,
            CursorIcon::ResizeNeSw,
        ),
        (
            Rect::from_min_max(pos2(l, bt - b), pos2(l + b, bt)),
            ResizeDirection::SouthWest,
            CursorIcon::ResizeNeSw,
        ),
        (
            Rect::from_min_max(pos2(rt - b, bt - b), pos2(rt, bt)),
            ResizeDirection::SouthEast,
            CursorIcon::ResizeNwSe,
        ),
        (
            Rect::from_min_max(pos2(l + b, t), pos2(rt - b, t + b)),
            ResizeDirection::North,
            CursorIcon::ResizeVertical,
        ),
        (
            Rect::from_min_max(pos2(l + b, bt - b), pos2(rt - b, bt)),
            ResizeDirection::South,
            CursorIcon::ResizeVertical,
        ),
        (
            Rect::from_min_max(pos2(l, t + b), pos2(l + b, bt - b)),
            ResizeDirection::West,
            CursorIcon::ResizeHorizontal,
        ),
        (
            Rect::from_min_max(pos2(rt - b, t + b), pos2(rt, bt - b)),
            ResizeDirection::East,
            CursorIcon::ResizeHorizontal,
        ),
    ];
    for (i, (rect, dir, cursor)) in bands.into_iter().enumerate() {
        let resp = ui
            .interact(rect, Id::unique(("resize_band", i)), Sense::drag())
            .on_hover_cursor(cursor);
        if resp.drag_started_by(egui::PointerButton::Primary) {
            ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
        }
    }
}

/// A 1 px frame around the undecorated window (hidden when maximized).
pub fn window_border(ctx: &egui::Context, theme: &Theme) {
    if is_maximized(ctx) {
        return;
    }
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        Id::unique("window_border"),
    ));
    painter.rect_stroke(
        ctx.viewport_rect(),
        0.0,
        Stroke::new(1.0, theme.border),
        StrokeKind::Inside,
    );
}
