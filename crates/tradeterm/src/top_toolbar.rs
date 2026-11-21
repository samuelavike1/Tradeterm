//! The top app bar, TradingView's header toolbar (docs/specs/appbar.md):
//!
//! left: ticker | intervals + interval menu | chart types | Indicators,
//! indicator templates; right: undo, redo | layout menu | quick search,
//! settings, fullscreen, snapshot.
//!
//! Every control is a kit `ToolbarButton` (32 px, 18 px icons), groups are
//! split by `Divider::vertical()`, tooltips sit below and show shortcuts
//! from the key bindings here.

use crate::app::{KEY_CONTEXT, OpenIndicators, TradeTerm};
use crate::ui::chart_type_icon;
use gpui_kit::component::h_flex;
use gpui_kit::*;
use tt_chart_gpui::ChartSource as _;
use tt_ui::hsla;
use tt_ui::prelude::*;
use tt_ui::tokens::*;

actions!(
    tradeterm,
    [
        /// Undo the last drawing change (Ctrl+Z).
        Undo,
        /// Redo (Ctrl+Y, Ctrl+Shift+Z).
        Redo,
        /// Command palette (Ctrl+K).
        QuickSearch,
        /// Shift+F on the chart.
        ToggleFullscreen,
        /// Save the current layout (Ctrl+S).
        SaveLayout,
        /// Snapshot to a PNG file (Ctrl+Alt+S).
        DownloadSnapshot,
        /// Snapshot to the clipboard (Ctrl+Shift+S).
        CopySnapshot,
        /// The chart Settings dialog (gear on the app bar).
        OpenChartSettings,
        /// "Create alert" (Alt+A).
        CreateAlert,
    ]
);

/// App bar shortcuts (TradingView's).
pub fn key_bindings() -> Vec<KeyBinding> {
    let chart = Some(KEY_CONTEXT);
    vec![
        KeyBinding::new("ctrl-z", Undo, chart),
        KeyBinding::new("ctrl-y", Redo, chart),
        KeyBinding::new("ctrl-shift-z", Redo, chart),
        KeyBinding::new("shift-f", ToggleFullscreen, chart),
        KeyBinding::new("ctrl-k", QuickSearch, None),
        KeyBinding::new("ctrl-,", OpenChartSettings, None),
        KeyBinding::new("ctrl-s", SaveLayout, None),
        KeyBinding::new("ctrl-alt-s", DownloadSnapshot, None),
        KeyBinding::new("ctrl-shift-s", CopySnapshot, None),
        KeyBinding::new("alt-a", CreateAlert, chart),
    ]
}

/// Registers the app bar's action handlers on the app root.
pub fn on_app_bar_actions(root: Stateful<Div>, cx: &mut Context<TradeTerm>) -> Stateful<Div> {
    root.on_action(cx.listener(|this, _: &Undo, window, cx| {
        if !this.chart_has_keyboard(window, cx) {
            return cx.propagate();
        }
        this.undo(cx);
    }))
    .on_action(cx.listener(|this, _: &Redo, window, cx| {
        if !this.chart_has_keyboard(window, cx) {
            return cx.propagate();
        }
        this.redo(cx);
    }))
    .on_action(cx.listener(|this, _: &ToggleFullscreen, window, cx| {
        if !this.chart_has_keyboard(window, cx) {
            return cx.propagate();
        }
        this.toggle_fullscreen(window, cx);
    }))
    .on_action(cx.listener(|this, _: &QuickSearch, window, cx| this.open_quick_search(window, cx)))
    .on_action(cx.listener(|this, _: &SaveLayout, window, cx| this.save_layout(window, cx)))
    .on_action(
        cx.listener(|this, _: &DownloadSnapshot, window, cx| this.download_snapshot(window, cx)),
    )
    .on_action(cx.listener(|this, _: &CopySnapshot, window, cx| this.copy_snapshot(window, cx)))
    .on_action(
        cx.listener(|this, _: &OpenChartSettings, window, cx| this.open_chart_settings(window, cx)),
    )
    .on_action(cx.listener(|this, _: &CreateAlert, window, cx| {
        if !this.chart_has_keyboard(window, cx) {
            return cx.propagate();
        }
        this.open_create_alert(window, cx);
    }))
}

impl TradeTerm {
    /// The chart area has the keyboard (no dialog, nothing else focused).
    pub(crate) fn chart_has_keyboard(&self, window: &mut Window, cx: &mut App) -> bool {
        use gpui_kit::component::WindowExt as _;
        !window.has_active_dialog(cx) && self.focus.is_focused(window)
    }

    pub(crate) fn undo(&mut self, cx: &mut Context<Self>) {
        if let Some(chart) = self.tab_chart().cloned() {
            chart.update(cx, |c, cx| c.undo_drawing(cx));
            cx.notify();
        }
    }

    pub(crate) fn redo(&mut self, cx: &mut Context<Self>) {
        if let Some(chart) = self.tab_chart().cloned() {
            chart.update(cx, |c, cx| c.redo_drawing(cx));
            cx.notify();
        }
    }

    pub(crate) fn toggle_fullscreen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.toggle_fullscreen();
        cx.notify();
    }

    /// (can undo, can redo) on the active chart.
    fn undo_state(&self, cx: &App) -> (bool, bool) {
        self.tab_chart().map_or((false, false), |c| {
            let d = &c.read(cx).state.read(cx).source.view().drawings;
            (d.can_undo(), d.can_redo())
        })
    }

    pub(crate) fn toolbar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(chart) = self.tab_chart() else {
            return div();
        };
        let state = chart.read(cx).state.read(cx);
        let tab = &state.source.tab;
        let (symbol, tf, chart_type) = (tab.display_symbol(), tab.timeframe, tab.chart.chart_type);

        // Favorite intervals (plus the current one) and the interval menu.
        let mut tfs = h_flex().gap(S0_5.px()).items_center();
        let mut favorites = self.favorite_timeframes.clone();
        if !favorites.contains(&tf) {
            favorites.push(tf);
        }
        favorites.sort_by_key(|t| t.duration_ms());
        for t in favorites {
            tfs = tfs.child(
                ToolbarButton::text(SharedString::from(format!("tf-{}", t.code())), t.label())
                    .selected(t == tf)
                    .tooltip(t.long_name())
                    .on_click(cx.listener(move |this, _, _, cx| this.set_timeframe(t, cx))),
            );
        }
        tfs = tfs.child(crate::menus::timeframe_menu(cx));

        // Favorite chart types as quick buttons, then the current type's menu.
        let mut chart_types = h_flex().gap(S0_5.px()).items_center();
        for fav in self.favorite_chart_types.clone() {
            if fav == chart_type {
                continue;
            }
            chart_types = chart_types.child(
                ToolbarButton::icon(
                    SharedString::from(format!("ct-fav-{}", fav.label())),
                    chart_type_icon(fav),
                )
                .tooltip(fav.label())
                .on_click(cx.listener(move |this, _, _, cx| this.set_chart_type(fav, cx))),
            );
        }
        chart_types = chart_types.child(crate::menus::chart_type_menu(chart_type, cx));

        let indicators = h_flex()
            .gap(S0_5.px())
            .items_center()
            .child(
                ToolbarButton::text("indicators", "Indicators")
                    .leading(Glyph::SQUARE_FUNCTION)
                    .tooltip_action(
                        "Indicators, metrics, and strategies",
                        &OpenIndicators,
                        Some(KEY_CONTEXT),
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.open_indicators(window, cx))),
            )
            .child(crate::indicator_templates::templates_menu(cx))
            .child(
                ToolbarButton::text("create-alert", "Alert")
                    .leading(Glyph::ALARM_PLUS)
                    .tooltip_action("Create alert", &CreateAlert, Some(KEY_CONTEXT))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_create_alert(window, cx)),
                    ),
            );

        let (can_undo, can_redo) = self.undo_state(cx);
        let history = h_flex()
            .gap(S0_5.px())
            .items_center()
            .child(
                ToolbarButton::icon("undo", Glyph::UNDO)
                    .disabled(!can_undo)
                    .tooltip_action("Undo", &Undo, Some(KEY_CONTEXT))
                    .on_click(cx.listener(|this, _, _, cx| this.undo(cx))),
            )
            .child(
                ToolbarButton::icon("redo", Glyph::REDO)
                    .disabled(!can_redo)
                    .tooltip_action("Redo", &Redo, Some(KEY_CONTEXT))
                    .on_click(cx.listener(|this, _, _, cx| this.redo(cx))),
            );

        let full = window.is_fullscreen();
        let tools = h_flex()
            .gap(S0_5.px())
            .items_center()
            .child(
                ToolbarButton::icon("quick-search", Glyph::SEARCH)
                    .tooltip_action("Quick search", &QuickSearch, None)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_quick_search(window, cx)),
                    ),
            )
            .child(
                ToolbarButton::icon("chart-settings", Glyph::APP_SETTINGS)
                    .tooltip("Settings")
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_chart_settings(window, cx)),
                    ),
            )
            .child(
                ToolbarButton::icon(
                    "fullscreen",
                    if full {
                        Glyph::APP_EXIT_FULLSCREEN
                    } else {
                        Glyph::APP_FULLSCREEN
                    },
                )
                .selected(full)
                .tooltip_action(
                    if full {
                        "Exit fullscreen mode"
                    } else {
                        "Fullscreen mode"
                    },
                    &ToggleFullscreen,
                    Some(KEY_CONTEXT),
                )
                .on_click(cx.listener(|this, _, window, cx| this.toggle_fullscreen(window, cx))),
            )
            .child(crate::snapshot_menu::snapshot_menu(cx));

        let t = cx.ui();
        h_flex()
            .flex_none()
            .h(H_TOOLBAR.px())
            .px(S2.px())
            .gap(S1.px())
            .items_center()
            .bg(hsla(t.panel))
            .border_b_1()
            .border_color(hsla(t.border))
            .child(
                ToolbarButton::field("ticker", symbol)
                    .tooltip("Symbol search")
                    .on_click(cx.listener(|this, _, window, cx| this.open_search("", window, cx))),
            )
            .child(Divider::vertical())
            .child(tfs)
            .child(Divider::vertical())
            .child(chart_types)
            .child(Divider::vertical())
            .child(indicators)
            .child(div().flex_1().min_w_0())
            .child(history)
            .child(Divider::vertical())
            .child(crate::layouts_ui::layout_menu(self, cx))
            .child(Divider::vertical())
            .child(tools)
    }
}
