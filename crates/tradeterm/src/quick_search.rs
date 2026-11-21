//! Quick search (Ctrl+K), TradingView's command palette: one field over the
//! app's actions — intervals, chart types, indicators, indicator templates,
//! drawing tools, layouts, snapshot, tabs, theme, settings. Typing filters
//! (prefix > word start > substring > in-order letters); ↑/↓ move, Enter
//! runs, Esc closes.

use crate::app::{KEY_CONTEXT, NewTab, ToggleTheme, TradeTerm};
use crate::top_toolbar::{
    CopySnapshot, DownloadSnapshot, Redo, SaveLayout, ToggleFullscreen, Undo,
};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{WindowExt as _, h_flex, v_flex};
use gpui_kit::*;
use std::rc::Rc;
use tt_chart::ChartType;
use tt_ui::prelude::*;
use tt_ui::tokens::*;

type Run = Rc<dyn Fn(&mut TradeTerm, &mut Window, &mut Context<TradeTerm>)>;

#[derive(Clone)]
pub struct Command {
    pub label: SharedString,
    pub group: &'static str,
    pub icon: Option<Glyph>,
    /// Extra words it is found by ("RSI").
    pub keywords: SharedString,
    /// Shortcut shown on the right (the bound action).
    pub action: Option<(Rc<Box<dyn Action>>, Option<&'static str>)>,
    run: Run,
}

fn cmd(
    label: impl Into<SharedString>,
    group: &'static str,
    run: impl Fn(&mut TradeTerm, &mut Window, &mut Context<TradeTerm>) + 'static,
) -> Command {
    Command {
        label: label.into(),
        group,
        icon: None,
        keywords: SharedString::default(),
        action: None,
        run: Rc::new(run),
    }
}

impl Command {
    fn icon(mut self, g: Glyph) -> Self {
        self.icon = Some(g);
        self
    }

    fn keywords(mut self, k: impl Into<SharedString>) -> Self {
        self.keywords = k.into();
        self
    }

    fn score(&self, q: &str) -> Option<u32> {
        let k = (!self.keywords.is_empty())
            .then(|| score(&self.keywords, q))
            .flatten();
        score(&self.label, q).max(k)
    }

    fn keys(mut self, a: &dyn Action, context: Option<&'static str>) -> Self {
        self.action = Some((Rc::new(a.boxed_clone()), context));
        self
    }
}

/// Every command, in the order shown for an empty query.
fn commands(app: &TradeTerm) -> Vec<Command> {
    let mut v = vec![
        cmd(
            "Indicators, metrics, and strategies",
            "Actions",
            |a, w, cx| a.open_indicators(w, cx),
        )
        .icon(Glyph::SQUARE_FUNCTION)
        .keys(&crate::app::OpenIndicators, Some(KEY_CONTEXT)),
        cmd("Symbol search", "Actions", |a, w, cx| {
            a.open_search("", w, cx)
        })
        .icon(Glyph::SEARCH),
        cmd("Chart settings", "Actions", |a, w, cx| {
            a.open_chart_settings(w, cx)
        })
        .icon(Glyph::SETTINGS),
        cmd("Undo", "Actions", |a, _, cx| a.undo(cx))
            .icon(Glyph::UNDO)
            .keys(&Undo, Some(KEY_CONTEXT)),
        cmd("Redo", "Actions", |a, _, cx| a.redo(cx))
            .icon(Glyph::REDO)
            .keys(&Redo, Some(KEY_CONTEXT)),
        cmd("Download image", "Snapshot", |a, w, cx| {
            a.download_snapshot(w, cx)
        })
        .icon(Glyph::DOWNLOAD)
        .keys(&DownloadSnapshot, None),
        cmd("Copy image", "Snapshot", |a, w, cx| a.copy_snapshot(w, cx))
            .icon(Glyph::COPY)
            .keys(&CopySnapshot, None),
        cmd("Fullscreen mode", "Actions", |a, w, cx| {
            a.toggle_fullscreen(w, cx)
        })
        .icon(Glyph::FULLSCREEN)
        .keys(&ToggleFullscreen, Some(KEY_CONTEXT)),
        cmd("Save layout", "Layouts", |a, w, cx| a.save_layout(w, cx))
            .icon(Glyph::SAVE)
            .keys(&SaveLayout, None),
        cmd("Open layout…", "Layouts", |a, w, cx| {
            a.open_layouts_dialog(w, cx)
        })
        .icon(Glyph::FOLDER_OPEN),
        cmd("Make a copy of the layout…", "Layouts", |a, w, cx| {
            a.copy_layout(w, cx)
        })
        .icon(Glyph::COPY),
        cmd("Rename layout…", "Layouts", |a, w, cx| {
            a.rename_layout(w, cx)
        })
        .icon(Glyph::RENAME),
        cmd("Create new layout…", "Layouts", |a, w, cx| {
            a.new_layout(w, cx)
        })
        .icon(Glyph::FILE_PLUS),
        cmd("Download chart data…", "Layouts", |a, w, cx| {
            a.download_chart_data(w, cx)
        })
        .icon(Glyph::FILE_DOWN),
        cmd(
            if app.layout_autosave {
                "Turn autosave off"
            } else {
                "Turn autosave on"
            },
            "Layouts",
            |a, _, cx| a.set_layout_autosave(!a.layout_autosave, cx),
        ),
        cmd("New tab", "Tabs", |a, w, cx| a.new_tab(w, cx))
            .icon(Glyph::PLUS)
            .keys(&NewTab, None),
        cmd("Toggle theme", "Appearance", |a, w, cx| {
            a.toggle_theme(w, cx)
        })
        .icon(if app.theme.dark {
            Glyph::SUN
        } else {
            Glyph::MOON
        })
        .keys(&ToggleTheme, None),
        cmd(
            "Save indicator template…",
            "Indicator templates",
            |a, w, cx| a.save_indicator_template(w, cx),
        )
        .icon(Glyph::TEMPLATES),
    ];
    for t in tt_session::indicator_templates::builtin()
        .into_iter()
        .chain(app.indicator_templates.iter().cloned())
    {
        let label = format!("Apply template: {}", t.name);
        v.push(
            cmd(label, "Indicator templates", move |a, _, cx| {
                a.apply_indicator_template(&t, cx)
            })
            .icon(Glyph::TEMPLATES),
        );
    }
    for tf in crate::menus::Section::ALL
        .iter()
        .flat_map(|s| crate::menus::section_rows(*s, &app.custom_timeframes))
    {
        v.push(cmd(
            format!("Interval: {}", tf.long_name()),
            "Intervals",
            move |a, _, cx| a.set_timeframe(tf, cx),
        ));
    }
    for t in ChartType::ALL {
        v.push(
            cmd(
                format!("Chart type: {}", t.label()),
                "Chart types",
                move |a, _, cx| a.set_chart_type(t, cx),
            )
            .icon(Glyph::CHART_CANDLESTICK),
        );
    }
    for kind in tt_core::StudyKind::ALL {
        let meta = kind.default_study().meta();
        v.push(
            cmd(
                format!("Add indicator: {}", meta.name),
                "Indicators",
                move |a, _, cx| a.add_indicator(kind, cx),
            )
            .icon(Glyph::SQUARE_FUNCTION)
            .keywords(meta.short),
        );
    }
    for tool in tt_chart::DrawingTool::ALL {
        let mut c = cmd(
            format!("Drawing: {}", tool.label()),
            "Drawing tools",
            move |a, _, cx| a.pick_tool(Some(tool), cx),
        )
        .icon(Glyph::PENCIL_LINE);
        if let Some(action) = crate::drawing_toolbar::pick_action(tool) {
            c.action = Some((Rc::new(action), Some(KEY_CONTEXT)));
        }
        v.push(c);
    }
    v
}

/// Match score of `label` for `q` (lowercase), higher is better; `None`
/// when it does not match.
pub fn score(label: &str, q: &str) -> Option<u32> {
    if q.is_empty() {
        return Some(0);
    }
    let l = label.to_lowercase();
    if l.starts_with(q) {
        return Some(400);
    }
    // Word start, also after "Interval: " style prefixes.
    if l.split(|c: char| !c.is_alphanumeric())
        .any(|w| w.starts_with(q))
    {
        return Some(300);
    }
    // Initials ("rsi" ~ "Relative Strength Index").
    let initials: String = l
        .split(|c: char| !c.is_alphanumeric())
        .filter_map(|w| w.chars().next())
        .collect();
    if q.len() >= 2 && initials.contains(q) {
        return Some(250);
    }
    if l.contains(q) {
        return Some(200);
    }
    // Every query word somewhere ("rsi add").
    let words: Vec<&str> = q.split_whitespace().collect();
    if words.len() > 1 && words.iter().all(|w| l.contains(w)) {
        return Some(150);
    }
    // Letters in order ("stoch rsi" style typos), only for longer queries:
    // short ones match nearly everything this way.
    if q.chars().count() < 4 {
        return None;
    }
    let mut it = l.chars();
    q.chars()
        .filter(|c| !c.is_whitespace())
        .all(|qc| it.any(|lc| lc == qc))
        .then_some(100)
}

/// The key bound to `action` as display text (`Ctrl+K`), like menu rows.
fn shortcut(action: &dyn Action, context: Option<&str>, window: &Window) -> Option<String> {
    let binding = match context {
        Some(c) => KeyContext::parse(c)
            .ok()
            .and_then(|ctx| window.highest_precedence_binding_for_action_in_context(action, ctx)),
        None => window.highest_precedence_binding_for_action(action),
    }?;
    let k = binding.keystrokes().first()?.as_keystroke().clone();
    Some(gpui_kit::component::kbd::Kbd::format(&k))
}

/// Rows shown at most (the list scrolls).
const MAX_ROWS: usize = 200;

pub struct QuickSearchPanel {
    app: WeakEntity<TradeTerm>,
    input: Entity<InputState>,
    all: Vec<Command>,
    shown: Vec<usize>,
    selected: usize,
    scroll: ScrollHandle,
    _sub: Subscription,
}

impl QuickSearchPanel {
    fn new(
        app: &TradeTerm,
        weak: WeakEntity<TradeTerm>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search actions, settings, indicators, tools")
        });
        let _sub = cx.subscribe_in(
            &input,
            window,
            |this: &mut Self, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::Change => this.filter(cx),
                InputEvent::PressEnter { .. } => this.run_selected(window, cx),
                _ => {}
            },
        );
        let mut p = Self {
            app: weak,
            input,
            all: commands(app),
            shown: Vec::new(),
            selected: 0,
            scroll: ScrollHandle::new(),
            _sub,
        };
        p.filter(cx);
        p
    }

    fn filter(&mut self, cx: &mut Context<Self>) {
        let q = self.input.read(cx).value().trim().to_lowercase();
        let mut hits: Vec<(u32, usize)> = self
            .all
            .iter()
            .enumerate()
            .filter_map(|(i, c)| c.score(&q).map(|s| (s, i)))
            .collect();
        // Best score first; ties keep the command order.
        hits.sort_by_key(|(s, i)| (std::cmp::Reverse(*s), *i));
        self.shown = hits.into_iter().map(|(_, i)| i).take(MAX_ROWS).collect();
        self.selected = 0;
        self.scroll.scroll_to_item(0);
        cx.notify();
    }

    fn run(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = self.shown.get(index).and_then(|i| self.all.get(*i)) else {
            return;
        };
        let run = c.run.clone();
        window.close_dialog(cx);
        self.app.update(cx, |a, cx| run(a, window, cx)).ok();
    }

    fn run_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.run(self.selected, window, cx);
    }

    fn on_key(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let len = self.shown.len();
        if len == 0 {
            return;
        }
        let last = len - 1;
        let next = match e.keystroke.key.as_str() {
            "down" => (self.selected + 1).min(last),
            "up" => self.selected.saturating_sub(1),
            "pagedown" => (self.selected + 8).min(last),
            "pageup" => self.selected.saturating_sub(8),
            _ => return,
        };
        self.selected = next;
        self.scroll.scroll_to_item(next);
        cx.stop_propagation();
        cx.notify();
    }
}

impl Render for QuickSearchPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ui().clone();
        let mut list = v_flex()
            .id("quick-search-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.scroll);
        if self.shown.is_empty() {
            list = list.child(EmptyState::new("No matching actions"));
        }
        for (row, &i) in self.shown.iter().enumerate() {
            let c = &self.all[i];
            let keys = c
                .action
                .as_ref()
                .and_then(|(a, ctx)| shortcut(a.as_ref().as_ref(), *ctx, window));
            let mut r = ListRow::new(("qs-row", row))
                .primary(c.label.clone())
                .selected(row == self.selected)
                .on_click(cx.listener(move |this, _, window, cx| this.run(row, window, cx)));
            if let Some(g) = c.icon {
                r = r.leading(tt_ui::glyph(g, ICON_SM));
            }
            // Group, then the shortcut, right-aligned on one line.
            r = r.trailing(
                h_flex()
                    .gap(S3.px())
                    .text_size(TEXT_SMALL.px())
                    .text_color(tt_ui::hsla(t.text_muted))
                    .child(c.group)
                    .child(h_flex().w((S6 * 4.0).px()).justify_end().children(keys)),
            );
            list = list.child(r);
        }
        v_flex()
            .h(H_DIALOG_BODY_MD.px())
            .gap(S2.px())
            .on_key_down(cx.listener(Self::on_key))
            .child(Input::new(&self.input).prefix(tt_ui::glyph(Glyph::SEARCH, ICON_SM)))
            .child(list)
            .child(
                h_flex()
                    .gap(S1.px())
                    .text_size(TEXT_SMALL.px())
                    .text_color(tt_ui::hsla(t.text_muted))
                    .child("↑↓ to move, Enter to run, Esc to close"),
            )
    }
}

impl TradeTerm {
    pub(crate) fn open_quick_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_dialog(cx) {
            return;
        }
        let weak = cx.entity().downgrade();
        let panel = cx.new(|cx| QuickSearchPanel::new(self, weak, window, cx));
        let input = panel.read(cx).input.clone();
        let opts = tt_ui::DialogOpts::new("Quick search", DialogSize::Md)
            .on_open(move |w, cx| input.update(cx, |s, cx| s.focus(w, cx)));
        tt_ui::open_dialog(window, cx, opts, panel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn scoring_prefers_prefix_then_word_then_substring() {
        assert_eq!(score("Save layout", "save"), Some(400));
        assert_eq!(score("Interval: 15 minutes", "15"), Some(300));
        assert_eq!(score("Download image", "load"), Some(200));
        assert_eq!(score("Add indicator: RSI", "rsi add"), Some(150));
        assert_eq!(score("Relative Strength Index", "rsi"), Some(250));
        assert_eq!(score("Exponential Moving Average", "emvg"), Some(100));
        assert_eq!(score("Indicators, metrics, and strategies", "rsi"), None);
        assert_eq!(score("Redo", "xyz"), None);
        assert_eq!(score("anything", ""), Some(0));
    }
}
