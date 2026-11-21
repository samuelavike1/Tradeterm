//! Toolbar dropdowns with stars, TradingView-style:
//!
//! - Interval menu: "Add custom interval…" first, then collapsible Seconds /
//!   Minutes / Hours / Days sections; the star pins an interval to the
//!   toolbar; custom intervals have a remove ×.
//! - Chart type menu: grouped types with their icon; the star pins a type to
//!   the toolbar as a quick button; hovering a row explains the type.
//!
//! Both are kit `Menu`s (`tt_ui`): 32 px rows, a ✓ on the current item,
//! keyboard navigation, and a row click closes the menu.

use crate::app::TradeTerm;
use crate::ui::chart_type_icon;
use gpui_kit::*;
use tt_chart::ChartType;
use tt_core::{TimeUnit, Timeframe};
use tt_ui::prelude::*;
use tt_ui::tokens::*;

/// Interval menu sections; `key` is what settings persist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub fn presets(self) -> Vec<Timeframe> {
        let n = Timeframe::new;
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

pub fn is_preset(tf: Timeframe) -> bool {
    Section::of(tf).presets().contains(&tf)
}

/// Rows of a section: presets plus custom intervals, sorted by length.
pub fn section_rows(section: Section, custom: &[Timeframe]) -> Vec<Timeframe> {
    let mut rows = section.presets();
    rows.extend(
        custom
            .iter()
            .copied()
            .filter(|t| Section::of(*t) == section),
    );
    rows.sort_by_key(|t| t.duration_ms());
    rows.dedup();
    rows
}

/// Menu height cap: the window minus the chrome rows and a margin, so a long
/// interval list scrolls instead of running off screen.
fn menu_max_h(window: &Window) -> Pixels {
    let reserved = H_TITLE_BAR + H_TOOLBAR + H_STATUS_BAR + S4;
    (window.viewport_size().height - reserved.px()).max((H_MENU_ROW * 6.).px())
}

/// The interval dropdown (chevron next to the favorite intervals).
pub fn timeframe_menu(cx: &mut Context<TradeTerm>) -> impl IntoElement + use<> {
    let app = cx.entity().downgrade();
    ToolbarButton::icon("tf-menu", Glyph::CHEVRON_DOWN)
        .tooltip("Intervals")
        .menu(move |menu, window, cx| {
            let Some(app_e) = app.upgrade() else {
                return menu;
            };
            let a = app_e.read(cx);
            let current = a.current_timeframe(cx);
            let (favorites, custom, collapsed) = (
                a.favorite_timeframes.clone(),
                a.custom_timeframes.clone(),
                a.collapsed_timeframe_sections.clone(),
            );
            let add = app.clone();
            let mut menu = menu
                .id("tf-menu-list")
                .width(MenuWidth::Md)
                .max_h(menu_max_h(window))
                .row(
                    MenuRow::new("tf-add", "Add custom interval…")
                        .icon(Glyph::PLUS)
                        .on_select(move |window, cx| {
                            add.update(cx, |a, cx| a.open_custom_interval(window, cx))
                                .ok();
                        }),
                )
                .separator();
            for section in Section::ALL {
                let open = !collapsed.iter().any(|k| k == section.key());
                let toggle = app.clone();
                menu = menu.collapsible_section(section.title(), open, move |_, cx| {
                    toggle
                        .update(cx, |a, cx| a.toggle_timeframe_section(section.key(), cx))
                        .ok();
                });
                if !open {
                    continue;
                }
                for tf in section_rows(section, &custom) {
                    let (select, star, remove) = (app.clone(), app.clone(), app.clone());
                    let id = SharedString::from(format!("tf-{}", tf.code()));
                    let row = MenuRow::new(id, tf.long_name())
                        .checked(tf == current)
                        .favorite(favorites.contains(&tf), move |_, cx| {
                            star.update(cx, |a, cx| a.toggle_favorite_timeframe(tf, cx))
                                .ok();
                        })
                        .on_select(move |_, cx| {
                            select.update(cx, |a, cx| a.set_timeframe(tf, cx)).ok();
                        });
                    let row = if custom.contains(&tf) {
                        row.removable("Remove interval", move |_, cx| {
                            remove
                                .update(cx, |a, cx| a.remove_custom_timeframe(tf, cx))
                                .ok();
                        })
                    } else {
                        row
                    };
                    menu = menu.row(row);
                }
            }
            menu
        })
}

/// The chart type dropdown (current type's icon + caret).
pub fn chart_type_menu(
    current: ChartType,
    cx: &mut Context<TradeTerm>,
) -> impl IntoElement + use<> {
    let app = cx.entity().downgrade();
    ToolbarButton::icon("chart-type", chart_type_icon(current))
        .caret()
        .tooltip(current.label())
        .menu(move |menu, _, cx| {
            let Some(app_e) = app.upgrade() else {
                return menu;
            };
            let a = app_e.read(cx);
            let current = a.current_chart_type(cx);
            let favorites = a.favorite_chart_types.clone();
            let mut menu = menu.id("ct-menu-list").width(MenuWidth::Md);
            for (g, group) in ChartType::GROUPS.iter().enumerate() {
                if g > 0 {
                    menu = menu.separator();
                }
                for &t in group.iter() {
                    let (select, star) = (app.clone(), app.clone());
                    let id = SharedString::from(format!("ct-{}", t.label()));
                    menu = menu.row(
                        MenuRow::new(id, t.label())
                            .icon(chart_type_icon(t))
                            .checked(t == current)
                            .description(t.description())
                            .favorite(favorites.contains(&t), move |_, cx| {
                                star.update(cx, |a, cx| a.toggle_favorite_chart_type(t, cx))
                                    .ok();
                            })
                            .on_select(move |_, cx| {
                                select.update(cx, |a, cx| a.set_chart_type(t, cx)).ok();
                            }),
                    );
                }
            }
            menu
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn custom_intervals_join_their_section_sorted() {
        let custom = [
            Timeframe::new(TimeUnit::Minute, 7),
            Timeframe::new(TimeUnit::Hour, 12),
        ];
        let minutes = section_rows(Section::Minutes, &custom);
        let pos = minutes.iter().position(|t| *t == custom[0]).unwrap();
        assert_eq!(minutes[pos - 1], Timeframe::new(TimeUnit::Minute, 5));
        assert!(section_rows(Section::Hours, &custom).ends_with(&[custom[1]]));
        assert!(is_preset(Timeframe::M15) && !is_preset(custom[0]));
    }
}
