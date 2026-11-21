//! TradingView's right sidebar (docs/specs/right-panel.md): a 45 px icon
//! strip at the far right with Watchlist, Alerts and Object tree / data
//! window. A click opens the panel to the strip's left, a click on the lit
//! button closes it; the panel is resized by dragging its left edge. The
//! open panel and the width are saved with the settings.
//!
//! Panels are their own views, rendered *cached*: the app re-renders on
//! every tick and mouse move, the panels only when they are notified
//! ([`SidebarViews`]: quotes, chart edits, the crosshair changing bar).
//!
//! Alerts mount point: [`crate::alerts_panel::AlertsPanel`], created the
//! first time its button opens (`AlertsPanel::new(window, cx)`), kept
//! afterwards, rendered full-size in the panel area.

use crate::alerts_panel::AlertsPanel;
use crate::app::TradeTerm;
use crate::object_tree::ObjectTreePanel;
use crate::watchlist_panel::WatchlistPanel;
use gpui_kit::component::{Placement, h_flex, v_flex};
use gpui_kit::*;
use tt_session::settings::AppSettings;
use tt_session::tab::Shared;
use tt_session::watchlist::{PanelKind, RightPanelSettings};
use tt_ui::prelude::*;
use tt_ui::tokens::*;
use tt_ui::{IconButton, hsla};

/// Ids of the sidebar's views, so the app can wake them without leasing
/// them (`App::notify`), even from inside one of their own handlers.
#[derive(Debug, Clone, Copy)]
pub struct SidebarViews {
    pub sidebar: EntityId,
    pub watchlist: EntityId,
    pub tree: EntityId,
}

impl SidebarViews {
    /// Chart state changed (indicators, drawings, symbol, tab, theme):
    /// every panel may show it.
    pub fn changed(&self, cx: &mut App) {
        cx.notify(self.sidebar);
        cx.notify(self.watchlist);
        cx.notify(self.tree);
    }

    /// New market data on some tab: prices in the watchlist, the live bar
    /// in the data window.
    pub fn ticked(&self, cx: &mut App) {
        cx.notify(self.watchlist);
        cx.notify(self.tree);
    }
}

pub struct RightSidebar {
    app: WeakEntity<TradeTerm>,
    open: Option<PanelKind>,
    width: f32,
    pub(crate) watchlist: Entity<WatchlistPanel>,
    pub(crate) tree: Entity<ObjectTreePanel>,
    alerts: Option<Entity<AlertsPanel>>,
}

/// Drag payload of the resize grip.
struct PanelResize;

impl Render for PanelResize {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

impl RightSidebar {
    pub fn new(
        app: WeakEntity<TradeTerm>,
        shared: &Shared,
        settings: &AppSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let p = settings.right_panel;
        let watchlist = {
            let (app, lists) = (app.clone(), settings.watchlists.clone());
            cx.new(|cx| WatchlistPanel::new(app, shared, lists, window, cx))
        };
        let tree = {
            let app = app.clone();
            cx.new(|cx| ObjectTreePanel::new(app, p.data_window, cx))
        };
        let mut this = Self {
            app,
            open: p.open,
            width: RightPanelSettings::clamped_width(p.width),
            watchlist,
            tree,
            alerts: None,
        };
        if this.open == Some(PanelKind::Alerts) {
            this.alerts = Some(cx.new(|cx| AlertsPanel::new(window, cx)));
        }
        this.sync_live(cx);
        this
    }

    pub fn views(&self, cx: &Context<Self>) -> SidebarViews {
        SidebarViews {
            sidebar: cx.entity_id(),
            watchlist: self.watchlist.entity_id(),
            tree: self.tree.entity_id(),
        }
    }

    /// The saved state of the sidebar and its panels.
    pub fn snapshot(&self, s: &mut AppSettings, cx: &App) {
        s.watchlists = self.watchlist.read(cx).lists.clone();
        s.right_panel = RightPanelSettings {
            open: self.open,
            width: self.width,
            data_window: self.tree.read(cx).data_window,
        };
    }

    fn persist(&self, cx: &mut Context<Self>) {
        let app = self.app.clone();
        // After this handler: the app may notify this view while saving.
        cx.defer(move |cx| {
            app.update(cx, |a, cx| a.persist(cx)).ok();
        });
    }

    fn toggle(&mut self, kind: PanelKind, window: &mut Window, cx: &mut Context<Self>) {
        self.open = (self.open != Some(kind)).then_some(kind);
        if self.open == Some(PanelKind::Alerts) && self.alerts.is_none() {
            self.alerts = Some(cx.new(|cx| AlertsPanel::new(window, cx)));
        }
        self.sync_live(cx);
        self.persist(cx);
        cx.notify();
    }

    /// Watchlist quotes stream only while the watchlist shows.
    fn sync_live(&self, cx: &mut Context<Self>) {
        let live = self.open == Some(PanelKind::Watchlist);
        self.watchlist.update(cx, |w, _| w.set_live(live));
    }

    fn panel(&self) -> Option<AnyElement> {
        let style = StyleRefinement::default().size_full();
        let view = match self.open? {
            PanelKind::Watchlist => AnyView::from(self.watchlist.clone()),
            PanelKind::ObjectTree => AnyView::from(self.tree.clone()),
            PanelKind::Alerts => AnyView::from(self.alerts.clone()?),
        };
        Some(view.cached(style).into_any_element())
    }

    fn strip_button(
        &self,
        kind: PanelKind,
        glyph: Glyph,
        tip: &'static str,
        cx: &mut Context<Self>,
    ) -> IconButton {
        IconButton::new(SharedString::from(format!("sidebar-{kind:?}")), glyph)
            .size(ControlSize::Sidebar)
            .toggled(self.open == Some(kind))
            .tooltip(tip)
            .tooltip_placement(Placement::Left)
            .on_click(cx.listener(move |this, _, window, cx| this.toggle(kind, window, cx)))
    }
}

impl Render for RightSidebar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ui().clone();
        let border = hsla(t.border);
        let panel = self.panel().map(|view| {
            div()
                .id("right-panel")
                .relative()
                .flex_none()
                .w(self.width.px())
                .h_full()
                .border_l_1()
                .border_color(border)
                .overflow_hidden()
                .child(view)
                .child(
                    // Resize grip on the left edge.
                    div()
                        .id("right-panel-grip")
                        .absolute()
                        .top_0()
                        .left_0()
                        .w(PANEL_GRIP.px())
                        .h_full()
                        .cursor_col_resize()
                        .on_drag(PanelResize, |_, _, _, cx| cx.new(|_| PanelResize)),
                )
        });
        h_flex()
            .id("right-sidebar")
            .h_full()
            .flex_none()
            .on_drag_move(cx.listener(|this, e: &DragMoveEvent<PanelResize>, _, cx| {
                let right = f32::from(e.bounds.right()) - W_SIDEBAR;
                let w = RightPanelSettings::clamped_width(right - f32::from(e.event.position.x));
                if (w - this.width).abs() >= 0.5 {
                    this.width = w;
                    this.persist(cx);
                    cx.notify();
                }
            }))
            .children(panel)
            .child(
                v_flex()
                    .id("sidebar-strip")
                    .w(W_SIDEBAR.px())
                    .h_full()
                    .flex_none()
                    .items_center()
                    .gap(S0_5.px())
                    .pt(S0_5.px())
                    .border_l_1()
                    .border_color(border)
                    .child(self.strip_button(
                        PanelKind::Watchlist,
                        Glyph::PANEL_WATCHLIST,
                        "Watchlist, details, and news",
                        cx,
                    ))
                    .child(self.strip_button(PanelKind::Alerts, Glyph::PANEL_ALERTS, "Alerts", cx))
                    .child(self.strip_button(
                        PanelKind::ObjectTree,
                        Glyph::PANEL_OBJECT_TREE,
                        "Object tree and data window",
                        cx,
                    )),
            )
    }
}
