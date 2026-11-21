//! "Favorite indicators and indicator templates" (the button right of
//! Indicators; docs/specs/indicators.md §6, slice I-8): one kit menu with
//! FAVORITE INDICATORS (a click adds one), MY TEMPLATES ("Save indicator
//! template…", saved templates with star and delete) and TECHNICALS
//! (built-in). Applying a template replaces the chart's indicators and,
//! if it remembers them, switches the symbol and interval.

use crate::app::TradeTerm;
use crate::name_prompt::{self, PromptOpts, Submit};
use gpui_kit::*;
use tt_session::indicator_templates::{self as tpl, IndicatorTemplate};
use tt_session::settings::IndicatorConfig;
use tt_ui::prelude::*;

pub fn templates_menu(cx: &mut Context<TradeTerm>) -> impl IntoElement + use<> {
    let app = cx.entity().downgrade();
    ToolbarButton::icon("indicator-templates", Glyph::TEMPLATES)
        .tooltip("Favorite indicators and indicator templates")
        .menu(move |menu, _, cx| {
            let Some(a) = app.upgrade() else {
                return menu;
            };
            let a = a.read(cx);
            let (favorites, mine) = (a.favorite_indicators.clone(), a.indicator_templates.clone());
            let mut menu = menu
                .id("indicator-templates-menu")
                .width(MenuWidth::Md)
                .section("Favorite indicators");
            if favorites.is_empty() {
                menu = menu.row(
                    MenuRow::new("fav-none", "Star indicators in the Indicators dialog")
                        .disabled(true),
                );
            }
            for kind in favorites {
                let add = app.clone();
                let meta = kind.default_study().meta();
                menu = menu.row(
                    MenuRow::new(SharedString::from(format!("fav-{kind:?}")), meta.name).on_select(
                        move |_, cx| {
                            add.update(cx, |a, cx| a.add_indicator(kind, cx)).ok();
                        },
                    ),
                );
            }
            let save = app.clone();
            menu = menu.separator().section("My templates").row(
                MenuRow::new("tpl-save", "Save indicator template…")
                    .icon(Glyph::SAVE)
                    .on_select(move |window, cx| {
                        save.update(cx, |a, cx| a.save_indicator_template(window, cx))
                            .ok();
                    }),
            );
            // Starred templates first.
            let mut mine = mine;
            mine.sort_by_key(|t| !t.favorite);
            for t in mine {
                let (apply, star, del) = (app.clone(), app.clone(), app.clone());
                let (name, star_name, del_name) = (t.name.clone(), t.name.clone(), t.name.clone());
                menu = menu.row(
                    MenuRow::new(
                        SharedString::from(format!("tpl-{}", t.name)),
                        t.name.clone(),
                    )
                    .description(t.description())
                    .favorite(t.favorite, move |_, cx| {
                        star.update(cx, |a, cx| {
                            tpl::toggle_favorite(&mut a.indicator_templates, &star_name);
                            a.persist(cx);
                            cx.notify();
                        })
                        .ok();
                    })
                    .removable("Remove template", move |_, cx| {
                        del.update(cx, |a, cx| {
                            tpl::delete(&mut a.indicator_templates, &del_name);
                            a.persist(cx);
                            cx.notify();
                        })
                        .ok();
                    })
                    .on_select(move |_, cx| {
                        apply
                            .update(cx, |a, cx| a.apply_indicator_template_named(&name, cx))
                            .ok();
                    }),
                );
            }
            menu = menu.separator().section("Technicals");
            for t in tpl::builtin() {
                let apply = app.clone();
                menu = menu.row(
                    MenuRow::new(
                        SharedString::from(format!("tpl-builtin-{}", t.name)),
                        t.name.clone(),
                    )
                    .description(t.description())
                    .on_select(move |_, cx| {
                        apply
                            .update(cx, |a, cx| a.apply_indicator_template(&t, cx))
                            .ok();
                    }),
                );
            }
            menu
        })
}

impl TradeTerm {
    fn apply_indicator_template_named(&mut self, name: &str, cx: &mut Context<Self>) {
        let t = self
            .indicator_templates
            .iter()
            .find(|t| t.name == name)
            .cloned();
        if let Some(t) = t {
            self.apply_indicator_template(&t, cx);
        }
    }

    /// Replaces the active chart's indicators with `t`'s; switches symbol
    /// and interval if the template remembers them.
    pub(crate) fn apply_indicator_template(
        &mut self,
        t: &IndicatorTemplate,
        cx: &mut Context<Self>,
    ) {
        let t = t.clone();
        self.with_tab(cx, move |s, shared| {
            s.tab.chart.indicators = t
                .indicators
                .iter()
                .map(IndicatorConfig::to_indicator)
                .collect();
            if let Some(sym) = &t.symbol {
                s.tab.synthetic = None;
                s.tab.set_symbol(tt_core::Symbol::new(sym), shared);
            }
            if let Some(tf) = t.timeframe {
                s.tab.set_timeframe(tf, shared);
            }
        });
    }

    /// "Save indicator template…": name, remember symbol / interval.
    pub(crate) fn save_indicator_template(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(chart) = self.tab_chart() else {
            return;
        };
        let tab = &chart.read(cx).state.read(cx).source.tab;
        let indicators: Vec<IndicatorConfig> = tab
            .chart
            .indicators
            .iter()
            .map(IndicatorConfig::from_indicator)
            .collect();
        let (symbol, tf) = (tab.symbol.to_string(), tab.timeframe);
        let app = cx.entity().downgrade();
        name_prompt::open(
            window,
            cx,
            PromptOpts {
                title: "Save indicator template",
                label: "Template name",
                initial: String::new(),
                ok: "Save",
                checks: vec![("Remember symbol", false), ("Remember interval", false)],
            },
            move |name, checks, confirmed, _, cx| {
                let Some(app) = app.upgrade() else {
                    return Submit::Close;
                };
                if indicators.is_empty() {
                    return Submit::Error("Add indicators to the chart first.".into());
                }
                let exists = tpl::contains(&app.read(cx).indicator_templates, &name);
                if exists && !confirmed {
                    return Submit::Confirm(format!(
                        "“{}” already exists. Save again to replace it.",
                        name.trim()
                    ));
                }
                let t = IndicatorTemplate {
                    name,
                    indicators: indicators.clone(),
                    symbol: checks
                        .first()
                        .copied()
                        .unwrap_or(false)
                        .then(|| symbol.clone()),
                    timeframe: checks.get(1).copied().unwrap_or(false).then_some(tf),
                    favorite: false,
                };
                app.update(cx, |a, cx| {
                    let r = tpl::save(&mut a.indicator_templates, t);
                    a.persist(cx);
                    cx.notify();
                    match r {
                        Ok(_) => Submit::Close,
                        Err(e) => Submit::Error(e.message().into()),
                    }
                })
            },
        );
    }
}
