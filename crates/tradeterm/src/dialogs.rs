//! Dialog openers: symbol search, indicators, indicator settings, chart
//! settings, custom interval. Each is self-contained, so the dialog slices
//! can each own their function.

use crate::app::{TabChart, TradeTerm};
use crate::indicator_settings::IndicatorSettingsPanel;
use crate::indicators::IndicatorsPanel;
use crate::search::{self, SymbolSearch};
use gpui_kit::*;

impl TradeTerm {
    /// Opens the symbol search, optionally with text already typed.
    pub fn open_search(&mut self, initial: &str, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity().downgrade();
        let (source, handle) = (self.shared.source.clone(), self.shared.handle.clone());
        let recent = search::recent_rows(&self.recent);
        // Opened from the ticker: the current symbol, selected.
        let select_all = initial.is_empty();
        let initial = if select_all {
            self.tab_chart()
                .map(|c| c.read(cx).state.read(cx).source.tab.display_symbol())
                .unwrap_or_default()
        } else {
            initial.to_owned()
        };
        let quote = self.search_quote;
        let panel = cx.new(|cx| {
            SymbolSearch::new(
                app, source, handle, recent, &initial, quote, select_all, window, cx,
            )
        });
        let focus = panel.clone();
        // Picker dialog: no footer, Enter picks (§3.6).
        let opts = tt_ui::DialogOpts::new("Symbol search", tt_ui::DialogSize::Md)
            .on_open(move |window, cx| SymbolSearch::focus_input(&focus, window, cx));
        tt_ui::open_dialog(window, cx, opts, panel);
    }

    /// "Indicators, metrics, and strategies" (toolbar button or "/").
    pub fn open_indicators(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity().downgrade();
        let favorites = self.favorite_indicators.clone();
        let panel = cx.new(|cx| IndicatorsPanel::new(app, favorites, window, cx));
        let focus = panel.clone();
        // Picker dialog: no footer, Enter adds (§3.6); TradingView's
        // measured header and size.
        let opts =
            tt_ui::DialogOpts::new("Indicators, metrics, and strategies", tt_ui::DialogSize::Lg)
                .comfortable()
                .width(crate::indicators::W_PICKER)
                .on_open(move |window, cx| IndicatorsPanel::focus_input(&focus, window, cx));
        tt_ui::open_dialog(window, cx, opts, panel);
    }

    /// Settings of one indicator (legend gear or double-click).
    pub(crate) fn open_indicator_settings(
        &mut self,
        chart: TabChart,
        id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let exists = chart
            .read(cx)
            .state
            .read(cx)
            .source
            .tab
            .chart
            .indicators
            .iter()
            .any(|i| i.id == id);
        if !exists {
            return;
        }
        let app = cx.entity().downgrade();
        let panel = cx.new(|cx| {
            IndicatorSettingsPanel::new(app, chart, id, window, cx)
                .expect("indicator checked above")
        });
        let title = panel.read(cx).title();
        let menu_panel = panel.downgrade();
        let ok_panel = panel.downgrade();
        let width = panel.read(cx).width.clone();
        let footer = tt_ui::DialogFooter::new()
            .comfortable()
            .template(
                tt_ui::TemplateMenu::build(move |menu, _, cx| {
                    IndicatorSettingsPanel::defaults_menu(&menu_panel, menu, cx)
                })
                .label("Defaults"),
            )
            .cancel()
            .ok("Ok")
            .on_ok(move |_, cx| {
                ok_panel.update(cx, |p, cx| p.commit(cx)).ok();
                true
            });
        // TradingView's indicator dialog: draggable header, no rename, the
        // width follows the tab.
        let opts = tt_ui::DialogOpts::new(title, crate::indicator_settings::SIZE)
            .comfortable()
            .live_width(width)
            .footer(footer);
        tt_ui::open_dialog(window, cx, opts, panel);
    }

    /// TradingView's chart "Settings" dialog for the active tab.
    pub fn open_chart_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_chart_settings_page(crate::chart_settings::Page::Symbol, window, cx);
    }

    /// The Settings dialog on `page` (the price-scale gear opens Scales).
    pub fn open_chart_settings_page(
        &mut self,
        page: crate::chart_settings::Page,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        crate::chart_settings::ChartSettingsPanel::open(self, page, window, cx);
    }

    pub fn open_custom_interval(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity().downgrade();
        let panel = cx.new(|cx| crate::custom_interval::CustomInterval::new(app, window, cx));
        crate::custom_interval::CustomInterval::open(panel, window, cx);
    }
}
