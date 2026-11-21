//! "Indicators, metrics, and strategies" (TradingView's indicator picker):
//! search, Favorites / Technicals nav, an alphabetical list with stars and
//! keyboard navigation. Clicking a row (or Enter) adds the indicator; the
//! dialog stays open. The settings of one indicator live in
//! [`crate::indicator_settings`].

use crate::app::TradeTerm;
use crate::search::match_ranges;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use lucide_icons::Icon as Lucide;
use tt_core::StudyKind;
use tt_ui::hsla;
use tt_ui::prelude::*;
use tt_ui::tokens::*;

/// TradingView's picker (measured, docs/specs/indicators-tv-ref.md): 840
/// × 638 with the 68 px header.
pub const W_PICKER: f32 = 840.0;
/// The body under the header: search 40 + lists 530.
const H_PICKER_BODY: f32 = 570.0;
/// The left nav column (items 200 wide after the 20 px inset).
const W_PICKER_NAV: f32 = 230.0;
const H_PICKER_NAV: f32 = 40.0;
const H_PICKER_CAPTION: f32 = 30.0;
/// The list's "SCRIPT NAME" row.
const H_PICKER_HEADER: f32 = 40.0;
const H_PICKER_ROW: f32 = 32.0;
/// Star box of a row, icon box of a nav item.
const W_STAR: f32 = 22.0;
const W_NAV_ICON: f32 = 28.0;
/// The list's side inset (rows start 10 px into it).
const LIST_INSET: f32 = 10.0;

/// Rows a PageUp / PageDown moves (about one screen of 36 px rows).
const PAGE: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Favorites,
    Technicals,
}

impl Section {
    fn id(self) -> &'static str {
        match self {
            Section::Favorites => "favorites",
            Section::Technicals => "technicals",
        }
    }
}

/// Short names users type ("rsi", "ema", "bb"…) besides the full name.
fn aliases(kind: StudyKind) -> &'static [&'static str] {
    match kind {
        StudyKind::Volume => &["vol"],
        StudyKind::Sma => &["ma", "sma"],
        StudyKind::Ema => &["ema"],
        StudyKind::BollingerBands => &["bb"],
        StudyKind::Vwap => &["vwap"],
        StudyKind::Rsi => &["rsi"],
        StudyKind::Macd => &["macd"],
        StudyKind::Stochastic => &["stoch"],
        StudyKind::Atr => &["atr"],
        StudyKind::Obv => &["obv"],
    }
}

/// Catalog entries matching `query`, alphabetically (like TradingView).
pub fn matching(query: &str, only: Option<&[StudyKind]>) -> Vec<StudyKind> {
    let q = query.trim().to_ascii_lowercase();
    let mut kinds: Vec<StudyKind> = StudyKind::ALL
        .into_iter()
        .filter(|k| only.is_none_or(|f| f.contains(k)))
        .filter(|k| {
            q.is_empty()
                || k.name().to_ascii_lowercase().contains(&q)
                || aliases(*k).iter().any(|a| a.starts_with(&q))
        })
        .collect();
    kinds.sort_by_key(|k| k.name().to_ascii_lowercase());
    kinds
}

/// Keyboard selection over `len` rows: arrows clamp at the ends; from no
/// selection, down / page down start at the top and up / page up at the
/// bottom; Home / End act once a row is selected (so they still move the
/// caret while typing).
pub fn nav_key(selected: Option<usize>, key: &str, len: usize) -> Option<Option<usize>> {
    let last = len.checked_sub(1)?;
    Some(Some(match (key, selected) {
        ("down" | "pagedown", None) => 0,
        ("up" | "pageup", None) => last,
        ("down", Some(i)) => (i + 1).min(last),
        ("up", Some(i)) => i.saturating_sub(1),
        ("pagedown", Some(i)) => (i + PAGE).min(last),
        ("pageup", Some(i)) => i.saturating_sub(PAGE),
        ("home", Some(_)) => 0,
        ("end", Some(_)) => last,
        _ => return None,
    }))
}

/// "Indicators, metrics, and strategies".
pub struct IndicatorsPanel {
    app: WeakEntity<TradeTerm>,
    input: Entity<InputState>,
    query: String,
    section: Section,
    favorites: Vec<StudyKind>,
    selected: Option<usize>,
    scroll: ScrollHandle,
    _sub: Subscription,
}

impl IndicatorsPanel {
    pub fn new(
        app: WeakEntity<TradeTerm>,
        favorites: Vec<StudyKind>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let _sub = cx.subscribe(&input, |this, input, ev: &InputEvent, cx| match ev {
            InputEvent::Change => {
                this.query = input.read(cx).value().to_string();
                this.selected = None;
                cx.notify();
            }
            InputEvent::PressEnter { .. } => {
                let rows = this.rows();
                let pick = this
                    .selected
                    .or((rows.len() == 1).then_some(0))
                    .and_then(|i| rows.get(i).copied());
                if let Some(k) = pick {
                    this.add(k, cx);
                }
            }
            _ => {}
        });
        Self {
            app,
            input,
            query: String::new(),
            section: Section::Technicals,
            favorites,
            selected: None,
            scroll: ScrollHandle::new(),
            _sub,
        }
    }

    pub fn focus_input(this: &Entity<Self>, window: &mut Window, cx: &mut App) {
        let input = this.read(cx).input.clone();
        let handle = input.read(cx).focus_handle(cx);
        handle.focus(window, cx);
    }

    fn rows(&self) -> Vec<StudyKind> {
        let only = (self.section == Section::Favorites).then_some(self.favorites.as_slice());
        matching(&self.query, only)
    }

    fn add(&mut self, kind: StudyKind, cx: &mut Context<Self>) {
        let app = self.app.clone();
        cx.defer(move |cx| {
            app.update(cx, |app, cx| app.add_indicator(kind, cx)).ok();
        });
    }

    fn toggle_favorite(&mut self, kind: StudyKind, cx: &mut Context<Self>) {
        if let Some(i) = self.favorites.iter().position(|k| *k == kind) {
            self.favorites.remove(i);
        } else {
            self.favorites.push(kind);
        }
        let favorites = self.favorites.clone();
        let app = self.app.clone();
        cx.defer(move |cx| {
            app.update(cx, |app, cx| app.set_favorite_indicators(favorites, cx))
                .ok();
        });
        cx.notify();
    }

    /// One result: `[☆] Name` (TradingView: 32 tall, radius 8, the star in
    /// a 22 px box, the match in the active color).
    fn row(&self, i: usize, kind: StudyKind, cx: &Context<Self>) -> Stateful<Div> {
        let t = cx.ui();
        let fav = self.favorites.contains(&kind);
        let selected = self.selected == Some(i);
        let group = SharedString::from(format!("indicator-row-{i}"));
        let name = StyledText::new(kind.name()).with_highlights(
            match_ranges(kind.name(), self.query.trim())
                .into_iter()
                .map(|r| {
                    (
                        r,
                        HighlightStyle {
                            color: Some(hsla(t.active)),
                            ..Default::default()
                        },
                    )
                }),
        );
        let this = cx.entity().downgrade();
        let star = div()
            .id(("star", i))
            .flex_none()
            .size(px(W_STAR))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(R_SM))
            .cursor_pointer()
            // TradingView's filled stars here are the text color.
            .when(fav, |d| d.text_color(hsla(t.text)))
            .when(!fav && !selected, |d| {
                d.invisible()
                    .group_hover(group.clone(), |s| s.visible())
                    .text_color(hsla(t.text_muted))
                    .hover(|s| s.text_color(hsla(t.text)))
            })
            .when(!fav && selected, |d| {
                d.text_color(hsla(t.text_muted))
                    .hover(|s| s.text_color(hsla(t.text)))
            })
            .child(glyph(
                if fav { Glyph::StarFilled } else { Glyph::STAR },
                ICON_MD,
            ))
            .tooltip(tip(
                if fav {
                    "Remove from favorites"
                } else {
                    "Add to favorites"
                },
                None,
            ))
            .on_click({
                let this = this.clone();
                move |_, _, cx| {
                    cx.stop_propagation();
                    this.update(cx, |p, cx| p.toggle_favorite(kind, cx)).ok();
                }
            });
        let (hover, active_bg) = (hsla(t.hover), hsla(t.active_bg));
        div()
            .id(("indicator", i))
            .group(group)
            .flex_none()
            .h(px(H_PICKER_ROW))
            .pl(px(S1))
            .pr(px(S4))
            .flex()
            .items_center()
            .gap(px(S1))
            .rounded(px(C_R_CONTROL))
            .cursor_pointer()
            .text_size(px(TEXT_BODY))
            .text_color(hsla(t.text))
            .when(selected, |d| d.bg(active_bg))
            .when(!selected, |d| d.hover(move |s| s.bg(hover)))
            .child(star)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(name),
            )
            .on_click(move |_, _, cx| {
                this.update(cx, |p, cx| p.add(kind, cx)).ok();
            })
    }

    /// A left-nav item: 40 tall, radius 8, icon in a 28 px box; the current
    /// one filled and semibold.
    fn nav_item(
        &self,
        section: Section,
        label: &'static str,
        icon: Glyph,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let t = cx.ui();
        let on = self.section == section;
        let (hover, sel) = (hsla(t.hover), hsla(t.selection));
        let this = cx.entity().downgrade();
        div()
            .id(section.id())
            .flex_none()
            .h(px(H_PICKER_NAV))
            .px(px(S2))
            .flex()
            .items_center()
            .gap(px(S2))
            .rounded(px(C_R_CONTROL))
            .cursor_pointer()
            .text_size(px(TEXT_BODY))
            .text_color(hsla(t.text))
            .when(on, |d| d.bg(sel).font_weight(FontWeight::SEMIBOLD))
            .when(!on, |d| d.hover(move |s| s.bg(hover)))
            .child(
                div()
                    .flex_none()
                    .size(px(W_NAV_ICON))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(glyph(icon, ICON_LG)),
            )
            .child(label)
            .on_click(move |_, _, cx| {
                this.update(cx, |p, cx| {
                    p.section = section;
                    p.selected = None;
                    cx.notify();
                })
                .ok();
            })
    }
}

/// An 11 px muted caption ("PERSONAL", "SCRIPT NAME").
fn caption(text: &str, h: f32, muted: Hsla) -> Div {
    div()
        .flex_none()
        .h(px(h))
        .flex()
        .items_center()
        .text_size(px(TEXT_CAPTION))
        .text_color(muted)
        .child(text.to_uppercase())
}

impl Render for IndicatorsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ui().clone();
        let muted = hsla(t.text_muted);
        let rows = self.rows();
        let list: AnyElement = if rows.is_empty() {
            let msg = if self.section == Section::Favorites && self.query.is_empty() {
                "Star indicators to add them to your favorites"
            } else {
                "No indicators match your criteria"
            };
            EmptyState::new(msg).into_any_element()
        } else {
            let items: Vec<Stateful<Div>> = rows
                .iter()
                .enumerate()
                .map(|(i, k)| self.row(i, *k, cx))
                .collect();
            // Rows are the list's only children, so row i is item i for
            // scroll-into-view.
            v_flex()
                .id("indicator-list")
                .flex_1()
                .min_h_0()
                .px(px(LIST_INSET))
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                .children(items)
                .into_any_element()
        };
        // Searching groups results under "Technicals", like TradingView.
        let header = if self.query.trim().is_empty() {
            "Name"
        } else {
            "Technicals"
        };
        let sidebar = v_flex()
            .flex_none()
            .w(px(W_PICKER_NAV))
            .pl(px(C_PAD_X))
            .pr(px(LIST_INSET))
            .pt(px(S2))
            .child(caption("Personal", H_PICKER_CAPTION, muted).pl(px(S3)))
            .child(self.nav_item(Section::Favorites, "Favorites", Glyph::STAR, cx))
            .child(div().h(px(S2)))
            .child(caption("Built-in", H_PICKER_CAPTION, muted).pl(px(S3)))
            .child(self.nav_item(
                Section::Technicals,
                "Technicals",
                Glyph::Lucide(Lucide::ChartLine),
                cx,
            ));
        v_flex()
            .id("indicators")
            .w_full()
            .h(px(H_PICKER_BODY))
            .capture_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                let k = &e.keystroke;
                if k.modifiers.control || k.modifiers.alt || k.modifiers.shift {
                    return;
                }
                let n = this.rows().len();
                if let Some(next) = nav_key(this.selected, k.key.as_str(), n) {
                    this.selected = next;
                    if let Some(i) = next {
                        this.scroll.scroll_to_item(i);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(
                div()
                    .flex_none()
                    .px(px(C_PAD_X))
                    .child(SearchField::boxed(&self.input)),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_start()
                    .child(sidebar)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .pb(px(S2))
                            .child(div().pl(px(LIST_INSET + S1 + W_STAR + S1)).child(caption(
                                header,
                                H_PICKER_HEADER,
                                muted,
                            )))
                            .child(list),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn search_matches_names_and_aliases_alphabetically() {
        let all = matching("", None);
        assert_eq!(all.len(), StudyKind::ALL.len());
        assert_eq!(all[0], StudyKind::Atr);
        assert_eq!(matching("rsi", None), [StudyKind::Rsi]);
        assert_eq!(matching("Moving", None), [StudyKind::Ema, StudyKind::Sma]);
        assert_eq!(matching("", Some(&[StudyKind::Macd])), [StudyKind::Macd]);
    }

    #[::core::prelude::v1::test]
    fn keyboard_navigation_clamps_and_starts_at_the_ends() {
        assert_eq!(nav_key(None, "down", 5), Some(Some(0)));
        assert_eq!(nav_key(None, "up", 5), Some(Some(4)));
        assert_eq!(nav_key(Some(4), "down", 5), Some(Some(4)));
        assert_eq!(nav_key(Some(0), "up", 5), Some(Some(0)));
        assert_eq!(nav_key(Some(1), "pagedown", 30), Some(Some(11)));
        assert_eq!(nav_key(Some(3), "end", 5), Some(Some(4)));
        assert_eq!(nav_key(None, "home", 5), None, "caret keys while typing");
        assert_eq!(nav_key(None, "down", 0), None);
        assert_eq!(nav_key(Some(1), "a", 5), None);
    }
}
