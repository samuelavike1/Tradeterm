//! Dialog openers: symbol search, indicators, indicator settings, chart
//! settings, custom interval. Each is self-contained, so the dialog slices
//! can each own their function.

use crate::app::{TabChart, TradeTerm};
use crate::indicators::{IndicatorSettingsPanel, IndicatorsPanel};
use crate::search::{self, SymbolSearch};
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::*;
use tt_theme::Theme;

impl TradeTerm {
    /// Opens the symbol search, optionally with text already typed.
    pub fn open_search(&mut self, initial: &str, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity().downgrade();
        let (source, handle) = (self.shared.source.clone(), self.shared.handle.clone());
        let (theme, recent) = (self.theme.clone(), search::recent_rows(&self.recent));
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
                app, source, handle, theme, recent, &initial, quote, select_all, window, cx,
            )
        });
        let focus_once = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if focus_once.replace(false) {
                SymbolSearch::focus_input(&panel, window, cx);
            }
            dialog
                .title("Symbol Search")
                .w(px(680.))
                .entrance(DialogEntrance::FadeSlide)
                .child(panel.clone())
        });
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

    /// TradingView's chart "Settings" dialog.
    pub fn open_chart_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity().downgrade();
        let (style, base) = (self.chart_style.clone(), Theme::for_dark(self.theme.dark));
        let panel = cx
            .new(|cx| crate::chart_settings::ChartSettingsPanel::new(app, style, base, window, cx));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title("Settings")
                .w(px(780.))
                .entrance(DialogEntrance::FadeSlide)
                .child(panel.clone())
        });
    }

    pub fn open_custom_interval(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity().downgrade();
        let panel = cx.new(|cx| crate::custom_interval::CustomInterval::new(app, window, cx));
        let focus_once = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if focus_once.replace(false) {
                crate::custom_interval::CustomInterval::focus_input(&panel, window, cx);
            }
            dialog
                .title("Add custom interval")
                .w(px(520.))
                .entrance(DialogEntrance::FadeSlide)
                .child(panel.clone())
        });
    }
}
