//! Timeframe picker, modelled on TradingView's interval menu.
//!
//! - Toolbar: one quick button per *favorite* interval (sorted by length),
//!   plus the current interval if it isn't a favorite, then a chevron.
//! - Chevron menu: "Add custom interval…", collapsible SECONDS / MINUTES /
//!   HOURS / DAYS sections with full names ("5 minutes"); the current
//!   interval is drawn inverted; a star toggles favorites (filled when set,
//!   outline on hover); custom intervals can be removed.
//! - "Add custom interval" dialog: Type + Interval, Add disabled until valid.

use crate::icons::{self, Icon};
use crate::toolbar;
use egui::epaint::Mesh;
use egui::{
    Align2, Color32, FontId, Frame, Id, Key, Margin, Pos2, Rect, RichText, Sense, Shape, Stroke,
    TextEdit, Ui, pos2, vec2,
};
use std::collections::BTreeSet;
use tt_core::{TimeUnit, Timeframe};
use tt_theme::{Theme, with_alpha};

/// Menu sections, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Section {
    Seconds,
    Minutes,
    Hours,
    Days,
}

impl Section {
    pub const ALL: [Section; 4] = [
        Section::Seconds,
        Section::Minutes,
        Section::Hours,
        Section::Days,
    ];

    pub fn of(tf: Timeframe) -> Section {
        match tf.unit() {
            TimeUnit::Second => Section::Seconds,
            TimeUnit::Minute => Section::Minutes,
            TimeUnit::Hour => Section::Hours,
            TimeUnit::Day | TimeUnit::Week | TimeUnit::Month => Section::Days,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Section::Seconds => "SECONDS",
            Section::Minutes => "MINUTES",
            Section::Hours => "HOURS",
            Section::Days => "DAYS",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Section::Seconds => "seconds",
            Section::Minutes => "minutes",
            Section::Hours => "hours",
            Section::Days => "days",
        }
    }

    /// Built-in rows (TradingView's list).
    fn presets(self) -> Vec<Timeframe> {
        let n = |u, c| Timeframe::new(u, c);
        match self {
            Section::Seconds => [1, 5, 10, 15, 30, 45]
                .map(|c| n(TimeUnit::Second, c))
                .to_vec(),
            Section::Minutes => [1, 2, 3, 5, 10, 15, 30, 45]
                .map(|c| n(TimeUnit::Minute, c))
                .to_vec(),
            Section::Hours => [1, 2, 3, 4].map(|c| n(TimeUnit::Hour, c)).to_vec(),
            Section::Days => vec![
                n(TimeUnit::Day, 1),
                n(TimeUnit::Week, 1),
                n(TimeUnit::Month, 1),
                n(TimeUnit::Month, 3),
                n(TimeUnit::Month, 6),
                n(TimeUnit::Month, 12),
            ],
        }
    }
}

fn is_preset(tf: Timeframe) -> bool {
    Section::of(tf).presets().contains(&tf)
}

/// State of the "Add custom interval" dialog.
struct CustomDialog {
    unit: TimeUnit,
    text: String,
    focus: bool,
}

pub struct TimeframeMenu {
    pub favorites: Vec<Timeframe>,
    /// User-added intervals that are not in the preset lists.
    pub custom: Vec<Timeframe>,
    pub collapsed: BTreeSet<Section>,
    dialog: Option<CustomDialog>,
}

impl TimeframeMenu {
    pub fn new(favorites: Vec<Timeframe>, custom: Vec<Timeframe>, collapsed: &[String]) -> Self {
        let mut favorites = favorites;
        favorites.sort();
        favorites.dedup();
        let collapsed = Section::ALL
            .into_iter()
            .filter(|s| collapsed.iter().any(|c| c == s.key()))
            .collect();
        Self {
            favorites,
            custom: custom.into_iter().filter(|t| !is_preset(*t)).collect(),
            collapsed,
            dialog: None,
        }
    }

    pub fn collapsed_keys(&self) -> Vec<String> {
        self.collapsed.iter().map(|s| s.key().to_owned()).collect()
    }

    pub fn toggle_favorite(&mut self, tf: Timeframe) {
        if let Some(i) = self.favorites.iter().position(|f| *f == tf) {
            self.favorites.remove(i);
        } else {
            self.favorites.push(tf);
            self.favorites.sort();
        }
    }

    /// Adds a user interval (presets are already listed). Returns it.
    pub fn add_custom(&mut self, tf: Timeframe) -> Timeframe {
        if !is_preset(tf) && !self.custom.contains(&tf) {
            self.custom.push(tf);
            self.custom.sort();
        }
        tf
    }

    /// Rows of a section: presets plus custom intervals, by length.
    fn rows(&self, section: Section) -> Vec<Timeframe> {
        let mut v = section.presets();
        v.extend(
            self.custom
                .iter()
                .copied()
                .filter(|t| Section::of(*t) == section),
        );
        v.sort();
        v.dedup();
        v
    }

    /// Quick buttons plus the chevron menu. Returns a newly chosen interval.
    pub fn toolbar_ui(
        &mut self,
        ui: &mut Ui,
        current: Timeframe,
        theme: &Theme,
    ) -> Option<Timeframe> {
        let mut chosen = None;
        let mut buttons = self.favorites.clone();
        if !buttons.contains(&current) {
            buttons.push(current);
            buttons.sort();
        }
        for tf in buttons {
            let text = RichText::new(tf.label()).size(13.5);
            if ui
                .selectable_label(tf == current, text)
                .on_hover_text(tf.long_name())
                .clicked()
                && tf != current
            {
                chosen = Some(tf);
            }
        }

        let chevron = toolbar::icon_button(ui, 22.0, false, theme, |p, rect, fg| {
            icons::paint(p, rect.center(), Icon::ChevronDown, 14.0, fg);
        })
        .on_hover_text("More intervals");
        egui::Popup::menu(&chevron)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                if let Some(tf) = self.menu_ui(ui, current, theme) {
                    chosen = Some(tf);
                    ui.close();
                }
            });
        chosen
    }

    fn menu_ui(&mut self, ui: &mut Ui, current: Timeframe, theme: &Theme) -> Option<Timeframe> {
        const W: f32 = 240.0;
        ui.set_width(W);
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let mut chosen = None;

        // "+ Add custom interval…"
        let (rect, resp) = ui.allocate_exact_size(vec2(W, 34.0), Sense::click());
        let p = ui.painter();
        if resp.hovered() {
            p.rect_filled(rect, 5.0, theme.selection);
        }
        icons::paint(
            p,
            pos2(rect.left() + 16.0, rect.center().y),
            Icon::Plus,
            16.0,
            theme.text,
        );
        p.text(
            pos2(rect.left() + 34.0, rect.center().y),
            Align2::LEFT_CENTER,
            "Add custom interval…",
            FontId::proportional(13.5),
            theme.text,
        );
        if resp.clicked() {
            self.dialog = Some(CustomDialog {
                unit: Section::of(current).default_unit(),
                text: String::new(),
                focus: true,
            });
            ui.close();
        }

        egui::ScrollArea::vertical()
            // Use the window height below the toolbar, like TradingView.
            .max_height((ui.ctx().content_rect().height() - 150.0).max(240.0))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for section in Section::ALL {
                    self.section_header(ui, section, theme, W);
                    if self.collapsed.contains(&section) {
                        continue;
                    }
                    for tf in self.rows(section) {
                        match self.row(ui, tf, tf == current, theme, W) {
                            RowAction::Select => chosen = Some(tf),
                            RowAction::ToggleFavorite => self.toggle_favorite(tf),
                            RowAction::Remove => {
                                self.custom.retain(|t| *t != tf);
                                self.favorites.retain(|t| *t != tf);
                            }
                            RowAction::None => {}
                        }
                    }
                }
            });
        chosen
    }

    fn section_header(&mut self, ui: &mut Ui, section: Section, theme: &Theme, w: f32) {
        ui.add_space(4.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
        let p = ui.painter();
        p.hline(rect.x_range(), rect.top(), Stroke::new(1.0, theme.border));
        let color = if resp.hovered() {
            theme.text
        } else {
            theme.text_muted
        };
        p.text(
            pos2(rect.left() + 12.0, rect.center().y),
            Align2::LEFT_CENTER,
            section.title(),
            FontId::proportional(11.0),
            color,
        );
        let collapsed = self.collapsed.contains(&section);
        let icon = if collapsed {
            Icon::ChevronDown
        } else {
            Icon::ChevronUp
        };
        icons::paint(
            p,
            pos2(rect.right() - 14.0, rect.center().y),
            icon,
            13.0,
            color,
        );
        if resp.clicked() {
            if collapsed {
                self.collapsed.remove(&section);
            } else {
                self.collapsed.insert(section);
            }
        }
    }

    fn row(&self, ui: &mut Ui, tf: Timeframe, selected: bool, theme: &Theme, w: f32) -> RowAction {
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
        let star_rect =
            Rect::from_center_size(pos2(rect.right() - 18.0, rect.center().y), vec2(22.0, 22.0));
        let star = ui.interact(
            star_rect,
            ui.make_persistent_id(("tf_star", tf.code())),
            Sense::click(),
        );
        let removable = self.custom.contains(&tf) && !selected;
        let remove_rect = star_rect.translate(vec2(-24.0, 0.0));
        let remove = removable.then(|| {
            ui.interact(
                remove_rect,
                ui.make_persistent_id(("tf_rm", tf.code())),
                Sense::click(),
            )
        });
        let hovered =
            resp.hovered() || star.hovered() || remove.as_ref().is_some_and(|r| r.hovered());

        let p = ui.painter();
        let (bg, fg) = if selected {
            (theme.text, theme.bg)
        } else if hovered {
            (theme.selection, theme.text)
        } else {
            (Color32::TRANSPARENT, theme.text)
        };
        p.rect_filled(rect.shrink2(vec2(4.0, 1.0)), 5.0, bg);
        p.text(
            pos2(rect.left() + 14.0, rect.center().y),
            Align2::LEFT_CENTER,
            tf.long_name(),
            FontId::proportional(13.5),
            fg,
        );

        let favorite = self.favorites.contains(&tf);
        if favorite {
            filled_star(p, star_rect.center(), 6.5, fg);
        } else if hovered {
            let c = if star.hovered() {
                fg
            } else {
                with_alpha(fg, 0.6)
            };
            icons::paint(p, star_rect.center(), Icon::Star, 14.0, c);
        }
        if let Some(rm) = &remove
            && hovered
        {
            let c = if rm.hovered() {
                fg
            } else {
                with_alpha(fg, 0.6)
            };
            icons::paint(p, remove_rect.center(), Icon::X, 14.0, c);
        }

        if star.clicked() {
            RowAction::ToggleFavorite
        } else if remove.is_some_and(|r| r.clicked()) {
            RowAction::Remove
        } else if resp.clicked() {
            RowAction::Select
        } else {
            RowAction::None
        }
    }

    /// The "Add custom interval" dialog, if open. Returns the added interval.
    pub fn dialog_ui(&mut self, ctx: &egui::Context, theme: &Theme) -> Option<Timeframe> {
        let dialog = self.dialog.as_mut()?;
        let count: Option<u32> = dialog.text.trim().parse().ok();
        let valid = count.is_some_and(|c| Timeframe::is_valid_custom(dialog.unit, c));
        let (enter, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, Key::Enter),
                i.consume_key(egui::Modifiers::NONE, Key::Escape),
            )
        });
        let mut close = esc;
        let mut add = enter && valid;

        let frame = Frame::new()
            .fill(theme.panel)
            .stroke(Stroke::new(1.0, theme.border))
            .corner_radius(10)
            .inner_margin(Margin::same(18))
            .shadow(egui::Shadow {
                offset: [0, 8],
                blur: 28,
                spread: 0,
                color: with_alpha(Color32::BLACK, if theme.dark { 0.6 } else { 0.18 }),
            });
        let modal = egui::Modal::new(Id::unique("custom_interval_modal"))
            .frame(frame)
            .backdrop_color(with_alpha(
                Color32::BLACK,
                if theme.dark { 0.45 } else { 0.2 },
            ))
            .show(ctx, |ui| {
                ui.set_width(330.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Add custom interval").size(17.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(r, 5.0, theme.selection);
                        }
                        icons::paint(ui.painter(), r.center(), Icon::X, 16.0, theme.text);
                        if resp.clicked() {
                            close = true;
                        }
                    });
                });
                ui.add_space(14.0);
                egui::Grid::new("custom_interval_grid")
                    .num_columns(2)
                    .spacing(vec2(24.0, 12.0))
                    .show(ui, |ui| {
                        ui.label("Type");
                        egui::ComboBox::from_id_salt("custom_interval_unit")
                            .selected_text(dialog.unit.plural_name())
                            .width(150.0)
                            .show_ui(ui, |ui| {
                                for u in TimeUnit::ALL {
                                    ui.selectable_value(&mut dialog.unit, u, u.plural_name());
                                }
                            });
                        ui.end_row();
                        ui.label("Interval");
                        let edit = ui.add(
                            TextEdit::singleline(&mut dialog.text)
                                .desired_width(150.0)
                                .hint_text(format!("1 – {}", dialog.unit.max_count())),
                        );
                        if dialog.focus {
                            edit.request_focus();
                            dialog.focus = false;
                        }
                        if edit.changed() {
                            dialog.text.retain(|c| c.is_ascii_digit());
                        }
                        ui.end_row();
                    });
                ui.add_space(16.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let add_btn = ui.add_enabled(
                        valid,
                        egui::Button::new(RichText::new("Add").strong())
                            .min_size(vec2(70.0, 30.0))
                            .fill(if valid { theme.text } else { theme.panel_alt }),
                    );
                    if add_btn.clicked() {
                        add = true;
                    }
                    if ui
                        .add(egui::Button::new("Cancel").min_size(vec2(70.0, 30.0)))
                        .clicked()
                    {
                        close = true;
                    }
                });
            });
        if modal.should_close() {
            close = true;
        }
        let unit = dialog.unit;
        if add && let Some(c) = count {
            self.dialog = None;
            return Some(self.add_custom(Timeframe::new(unit, c)));
        }
        if close {
            self.dialog = None;
        }
        None
    }
}

impl Section {
    fn default_unit(self) -> TimeUnit {
        match self {
            Section::Seconds => TimeUnit::Second,
            Section::Minutes => TimeUnit::Minute,
            Section::Hours => TimeUnit::Hour,
            Section::Days => TimeUnit::Day,
        }
    }
}

enum RowAction {
    None,
    Select,
    ToggleFavorite,
    Remove,
}

/// A solid five-pointed star (Lucide's star is outline-only).
pub fn filled_star(p: &egui::Painter, c: Pos2, r: f32, color: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(c, color);
    for k in 0..10 {
        let radius = if k % 2 == 0 { r } else { r * 0.45 };
        let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 5.0;
        mesh.colored_vertex(c + vec2(a.cos(), a.sin()) * radius, color);
    }
    for k in 0..10u32 {
        mesh.add_triangle(0, 1 + k, 1 + (k + 1) % 10);
    }
    p.add(Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tf(code: &str) -> Timeframe {
        Timeframe::parse(code).unwrap()
    }

    #[test]
    fn sections_and_rows() {
        let mut m = TimeframeMenu::new(Timeframe::ALL.to_vec(), vec![tf("7m"), tf("15m")], &[]);
        // 15m is a preset, so it isn't kept as custom.
        assert_eq!(m.custom, [tf("7m")]);
        let minutes: Vec<String> = m.rows(Section::Minutes).iter().map(|t| t.code()).collect();
        assert_eq!(
            minutes,
            ["1m", "2m", "3m", "5m", "7m", "10m", "15m", "30m", "45m"]
        );
        let days: Vec<String> = m.rows(Section::Days).iter().map(|t| t.code()).collect();
        assert_eq!(days, ["1D", "1W", "1M", "3M", "6M", "12M"]);
        assert_eq!(Section::of(tf("2W")), Section::Days);
        m.add_custom(tf("2D"));
        assert!(m.rows(Section::Days).contains(&tf("2D")));
    }

    #[test]
    fn favorites_toggle_and_stay_sorted() {
        let mut m = TimeframeMenu::new(vec![tf("1h"), tf("1m")], vec![], &[]);
        assert_eq!(m.favorites, [tf("1m"), tf("1h")]);
        m.toggle_favorite(tf("30m"));
        assert_eq!(m.favorites, [tf("1m"), tf("30m"), tf("1h")]);
        m.toggle_favorite(tf("1m"));
        assert_eq!(m.favorites, [tf("30m"), tf("1h")]);
    }

    #[test]
    fn collapsed_sections_round_trip() {
        let m = TimeframeMenu::new(vec![], vec![], &["seconds".into(), "bogus".into()]);
        assert!(m.collapsed.contains(&Section::Seconds));
        assert_eq!(m.collapsed_keys(), ["seconds"]);
    }
}
