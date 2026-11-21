//! Dialog openers: symbol search, indicators, indicator settings, chart
//! settings, custom interval. Each is self-contained, so the dialog slices
//! can each own their function.

use crate::app::{TabChart, TradeTerm};
use crate::indicators::{IndicatorSettingsPanel, IndicatorsPanel};
use crate::search::{self, SymbolSearch};
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::DialogEntrance;
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
        let (theme, favorites) = (self.theme.clone(), self.favorite_indicators.clone());
        let panel = cx.new(|cx| IndicatorsPanel::new(app, theme, favorites, window, cx));
        let focus_once = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if focus_once.replace(false) {
                IndicatorsPanel::focus_input(&panel, window, cx);
            }
            dialog
                .title("Indicators, metrics, and strategies")
                .w(px(780.))
                .entrance(DialogEntrance::FadeSlide)
                .child(panel.clone())
        });
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
        let theme = self.theme.clone();
        let panel = cx.new(|cx| {
            IndicatorSettingsPanel::new(chart, id, theme, window, cx)
                .expect("indicator checked above")
        });
        let title = panel.read(cx).title();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(title.clone())
                .w(px(460.))
                .entrance(DialogEntrance::FadeSlide)
                .child(panel.clone())
        });
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
