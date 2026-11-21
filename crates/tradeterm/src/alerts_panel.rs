//! Placeholder for the alerts panel (built separately; this stub is
//! replaced when it lands). Contract used by the right sidebar
//! (`right_sidebar.rs`): `AlertsPanel::new(window, cx)` and `impl Render`,
//! rendered full-size in the panel area, created the first time the Alerts
//! button opens and kept afterwards.

use gpui_kit::component::v_flex;
use gpui_kit::*;
use tt_ui::hsla;
use tt_ui::prelude::*;
use tt_ui::tokens::*;

pub struct AlertsPanel;

impl AlertsPanel {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for AlertsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ui();
        v_flex()
            .size_full()
            .child(
                div()
                    .h(H_PANEL_HEADER.px())
                    .flex()
                    .items_center()
                    .px(S4.px())
                    .text_size(TEXT_BODY.px())
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(hsla(t.text))
                    .child("Alerts"),
            )
            .child(EmptyState::new("No alerts yet"))
    }
}
