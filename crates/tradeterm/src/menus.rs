//! Toolbar dropdowns with stars, TradingView-style:
//!
//! - Interval menu: collapsible Seconds / Minutes / Hours / Days sections;
//!   the star pins an interval to the toolbar; "Add custom interval…".
//! - Chart type menu: grouped types with their icon; the star pins a type to
//!   the toolbar as a quick button; hovering a row explains the type.

use crate::app::TradeTerm;
use crate::ui::{chart_type_icon, lucide};
use ecolor::Color32;
use gpui_kit::base::PopoverState;
use gpui_kit::component::button::*;
use gpui_kit::component::input::{InputEvent, InputState, NumberInput};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{Icon, IconName, Sizable, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use tt_chart::ChartType;
use tt_chart_gpui::hsla;
use tt_core::{TimeUnit, Timeframe};
use tt_theme::Theme;

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

const GOLD: Color32 = Color32::from_rgb(0xf5, 0xb5, 0x2e);

/// One menu row: optional icon, label, star; selected rows are inverted
/// like TradingView's.
#[allow(clippy::too_many_arguments)]
fn row(
    id: SharedString,
    icon: Option<lucide_icons::Icon>,
    label: String,
    selected: bool,
    favorite: bool,
    theme: &Theme,
    on_select: impl Fn(&mut Window, &mut App) + 'static,
    on_star: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let (bg, fg) = if selected {
        (theme.text, theme.bg)
    } else {
        (Color32::TRANSPARENT, theme.text)
    };
    let group = SharedString::from(format!("{id}-g"));
    h_flex()
        .id(id.clone())
        .group(group.clone())
        .h(px(32.))
        .px_2()
        .gap_2()
        .rounded(px(5.))
        .cursor_pointer()
        .bg(hsla(bg))
        .text_color(hsla(fg))
        .when(!selected, |d| d.hover(|s| s.bg(hsla(theme.selection))))
        .children(icon.map(|i| lucide(i, 17.)))
        .child(div().flex_1().text_size(px(13.5)).child(label))
        .child(
            div()
                .id(SharedString::from(format!("{id}-star")))
                .size(px(22.))
                .flex()
                .items_center()
                .justify_center()
                .when(favorite, |d| {
                    d.text_color(hsla(if selected { fg } else { GOLD }))
                })
                .when(!favorite, |d| {
                    d.invisible()
                        .group_hover(group, |s| s.visible())
                        .text_color(hsla(theme.text_muted))
                        .hover(|s| s.text_color(hsla(fg)))
                })
                .child(
                    Icon::new(if favorite {
                        IconName::StarFill
                    } else {
                        IconName::Star
                    })
                    .size_4(),
                )
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    on_star(window, cx)
                }),
        )
        .on_click(move |_, window, cx| on_select(window, cx))
}

/// Closes the popover that `pop` belongs to.
fn dismiss(pop: &Entity<PopoverState>, window: &mut Window, cx: &mut App) {
    pop.update(cx, |s, cx| s.dismiss(window, cx));
}

/// The interval dropdown (chevron next to the favorite intervals).
pub fn timeframe_menu(cx: &mut Context<TradeTerm>) -> impl IntoElement + use<> {
    let app = cx.entity().downgrade();
    Popover::new("tf-popover")
        .trigger(
            Button::new("tf-menu")
                .ghost()
                .small()
                .icon(IconName::ChevronDown)
                .tooltip("Intervals"),
        )
        .content(move |_, _, cx| {
            let pop = cx.entity();
            let Some(app_e) = app.upgrade() else {
                return div().into_any_element();
            };
            let a = app_e.read(cx);
            let (theme, current) = (a.theme().clone(), a.current_timeframe(cx));
            let (favorites, custom, collapsed) = (
                a.favorite_timeframes.clone(),
                a.custom_timeframes.clone(),
                a.collapsed_timeframe_sections.clone(),
            );
            let mut list = v_flex()
                .id("tf-list")
                .w(px(230.))
                .max_h(px(560.))
                .overflow_y_scroll();
            for section in Section::ALL {
                let open = !collapsed.iter().any(|k| k == section.key());
                let (app2, pop2) = (app.clone(), pop.clone());
                list = list.child(
                    h_flex()
                        .id(section.key())
                        .h(px(30.))
                        .px_2()
                        .mt_1()
                        .rounded(px(5.))
                        .cursor_pointer()
                        .hover(|s| s.bg(hsla(theme.selection)))
                        .text_size(px(11.5))
                        .text_color(hsla(theme.text_muted))
                        .child(div().flex_1().child(section.title()))
                        .child(
                            Icon::new(if open {
                                IconName::ChevronUp
                            } else {
                                IconName::ChevronDown
                            })
                            .size_3p5(),
                        )
                        .on_click(move |_, _, cx| {
                            app2.update(cx, |a, cx| a.toggle_timeframe_section(section.key(), cx))
                                .ok();
                            pop2.update(cx, |_, cx| cx.notify());
                        }),
                );
                if !open {
                    continue;
                }
                for tf in section_rows(section, &custom) {
                    let (a_sel, p_sel, a_star, p_star) =
                        (app.clone(), pop.clone(), app.clone(), pop.clone());
                    let removable = custom.contains(&tf);
                    let (a_rm, p_rm) = (app.clone(), pop.clone());
                    let remove = div()
                        .id(SharedString::from(format!("tf-rm-{}", tf.code())))
                        .size(px(22.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .text_color(hsla(theme.text_muted))
                        .hover(|s| s.text_color(hsla(theme.text)))
                        .child(Icon::new(IconName::Close).size_3p5())
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            a_rm.update(cx, |a, cx| a.remove_custom_timeframe(tf, cx))
                                .ok();
                            p_rm.update(cx, |_, cx| cx.notify());
                        });
                    list = list.child(
                        row(
                            format!("tf-{}", tf.code()).into(),
                            None,
                            tf.long_name(),
                            tf == current,
                            favorites.contains(&tf),
                            &theme,
                            move |window, cx| {
                                a_sel.update(cx, |a, cx| a.set_timeframe(tf, cx)).ok();
                                dismiss(&p_sel, window, cx);
                            },
                            move |_, cx| {
                                a_star
                                    .update(cx, |a, cx| a.toggle_favorite_timeframe(tf, cx))
                                    .ok();
                                p_star.update(cx, |_, cx| cx.notify());
                            },
                        )
                        .when(removable, |r| r.child(remove)),
                    );
                }
            }
            let (a_add, p_add) = (app.clone(), pop.clone());
            v_flex()
                .child(list)
                .child(div().h(px(1.)).my_1().bg(hsla(theme.border)))
                .child(
                    h_flex()
                        .id("tf-add")
                        .h(px(32.))
                        .px_2()
                        .gap_2()
                        .rounded(px(5.))
                        .cursor_pointer()
                        .hover(|s| s.bg(hsla(theme.selection)))
                        .child(Icon::new(IconName::Plus).size_4())
                        .child("Add custom interval…")
                        .on_click(move |_, window, cx| {
                            dismiss(&p_add, window, cx);
                            a_add
                                .update(cx, |a, cx| a.open_custom_interval(window, cx))
                                .ok();
                        }),
                )
                .into_any_element()
        })
}

/// The chart type dropdown (current type's icon + chevron).
pub fn chart_type_menu(
    current: ChartType,
    cx: &mut Context<TradeTerm>,
) -> impl IntoElement + use<> {
    let app = cx.entity().downgrade();
    Popover::new("ct-popover")
        .trigger(
            Button::new("chart-type")
                .ghost()
                .small()
                .child(lucide(chart_type_icon(current), 17.))
                .dropdown_caret(true)
                .tooltip(current.label()),
        )
        .content(move |_, _, cx| {
            let pop = cx.entity();
            let Some(app_e) = app.upgrade() else {
                return div().into_any_element();
            };
            let a = app_e.read(cx);
            let theme = a.theme().clone();
            let current = a.current_chart_type(cx);
            let favorites = a.favorite_chart_types.clone();
            let mut list = v_flex().w(px(240.));
            for (g, group) in ChartType::GROUPS.iter().enumerate() {
                if g > 0 {
                    list = list.child(div().h(px(1.)).my_1().bg(hsla(theme.border)));
                }
                for &t in group.iter() {
                    let (a_sel, p_sel, a_star, p_star) =
                        (app.clone(), pop.clone(), app.clone(), pop.clone());
                    let desc = t.description();
                    list = list.child(
                        row(
                            format!("ct-{}", t.label()).into(),
                            Some(chart_type_icon(t)),
                            t.label().to_owned(),
                            t == current,
                            favorites.contains(&t),
                            &theme,
                            move |window, cx| {
                                a_sel.update(cx, |a, cx| a.set_chart_type(t, cx)).ok();
                                dismiss(&p_sel, window, cx);
                            },
                            move |_, cx| {
                                a_star
                                    .update(cx, |a, cx| a.toggle_favorite_chart_type(t, cx))
                                    .ok();
                                p_star.update(cx, |_, cx| cx.notify());
                            },
                        )
                        .tooltip(move |window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(desc).build(window, cx)
                        }),
                    );
                }
            }
            list.into_any_element()
        })
}

/// "Add custom interval": a number and a unit.
pub struct CustomInterval {
    app: WeakEntity<TradeTerm>,
    input: Entity<InputState>,
    unit: TimeUnit,
    error: Option<String>,
    _sub: Subscription,
}

const UNITS: [(TimeUnit, &str); 6] = [
    (TimeUnit::Second, "seconds"),
    (TimeUnit::Minute, "minutes"),
    (TimeUnit::Hour, "hours"),
    (TimeUnit::Day, "days"),
    (TimeUnit::Week, "weeks"),
    (TimeUnit::Month, "months"),
];

impl CustomInterval {
    pub fn new(app: WeakEntity<TradeTerm>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .step(1.)
                .min(1.)
                .max(1000.)
                .default_value("2")
        });
        let _sub = cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.add(window, cx);
            }
        });
        Self {
            app,
            input,
            unit: TimeUnit::Minute,
            error: None,
            _sub,
        }
    }

    pub fn focus_input(this: &Entity<Self>, window: &mut Window, cx: &mut App) {
        let input = this.read(cx).input.clone();
        let handle = input.read(cx).focus_handle(cx);
        handle.focus(window, cx);
    }

    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let count = self
            .input
            .read(cx)
            .value()
            .trim()
            .parse::<u32>()
            .unwrap_or(0);
        if !Timeframe::is_valid_custom(self.unit, count) {
            self.error = Some("Not a supported interval for this unit.".into());
            cx.notify();
            return;
        }
        let tf = Timeframe::new(self.unit, count);
        let app = self.app.clone();
        cx.defer(move |cx| {
            app.update(cx, |a, cx| a.add_custom_timeframe(tf, cx)).ok();
        });
        window.close_dialog(cx);
    }
}

impl Render for CustomInterval {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut units = h_flex().gap_1().flex_wrap();
        for (unit, label) in UNITS {
            units = units.child(
                Button::new(label)
                    .small()
                    .label(label)
                    .when(unit == self.unit, |b| b.primary())
                    .when(unit != self.unit, |b| b.outline())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.unit = unit;
                        this.error = None;
                        cx.notify();
                    })),
            );
        }
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .gap_3()
                    .child(div().w(px(120.)).child(NumberInput::new(&self.input)))
                    .child(units),
            )
            .children(self.error.clone().map(|e| {
                div()
                    .text_size(px(12.5))
                    .text_color(gpui_kit::red())
                    .child(e)
            }))
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("cancel")
                            .outline()
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("add")
                            .primary()
                            .label("Add")
                            .on_click(cx.listener(|this, _, window, cx| this.add(window, cx))),
                    ),
            )
    }
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
