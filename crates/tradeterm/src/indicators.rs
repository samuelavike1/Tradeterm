//! "Indicators, metrics, and strategies" (TradingView's indicator picker):
//! search, Favorites / Technicals nav, an alphabetical list with stars and
//! keyboard navigation. Clicking a row (or Enter) adds the indicator; the
//! dialog stays open. The settings of one indicator live in
//! [`crate::indicator_settings`].

use crate::app::TradeTerm;
use crate::search::match_ranges;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use lucide_icons::Icon as Lucide;
use tt_core::StudyKind;
use tt_ui::hsla;
use tt_ui::prelude::*;
use tt_ui::tokens::*;

/// The picker's size (§3.6: a list with a left nav).
pub const SIZE: DialogSize = DialogSize::Lg;

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

    fn row(&self, i: usize, kind: StudyKind, cx: &Context<Self>) -> ListRow {
        let t = cx.ui();
        let fav = self.favorites.contains(&kind);
        let id = ElementId::from(("indicator", i));
        // ListRow's hover group (shows the star of unstarred rows).
        let group = SharedString::from(format!("list-row-{id}"));
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
        let short = kind.meta().short;
        let this = cx.entity().downgrade();
        let star = div()
            .id(("star", i))
            .flex_none()
            .size(px(CONTROL_XS))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(R_SM))
            .cursor_pointer()
            .when(fav, |d| d.text_color(hsla(t.favorite)))
            .when(!fav, |d| {
                d.invisible()
                    .group_hover(group, |s| s.visible())
                    .text_color(hsla(t.text_muted))
                    .hover(|s| s.text_color(hsla(t.text)))
            })
            .child(glyph(
                if fav { Glyph::StarFilled } else { Glyph::STAR },
                ICON_SM,
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
        let leading = h_flex()
            .flex_1()
            .min_w_0()
            .gap(px(S2))
            .map(|mut d| {
                // Take the row's width from its (empty) text column.
                d.style().flex_grow = Some(1000.);
                d
            })
            .child(div().whitespace_nowrap().text_ellipsis().child(name))
            .child(Tag::new(short));
        ListRow::new(id)
            .selected(self.selected == Some(i))
            .leading(leading)
            .trailing(star)
            .on_click(move |_, _, cx| {
                this.update(cx, |p, cx| p.add(kind, cx)).ok();
            })
    }
}

impl Render for IndicatorsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = hsla(cx.ui().text_muted);
        let rows = self.rows();
        let body = if rows.is_empty() {
            let msg = if self.section == Section::Favorites && self.query.is_empty() {
                "Star indicators to add them to your favorites"
            } else {
                "No indicators match your criteria"
            };
            EmptyState::new(msg).into_any_element()
        } else {
            let list: Vec<ListRow> = rows
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
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                .children(list)
                .into_any_element()
        };
        let this = cx.entity().downgrade();
        let nav = NavTabs::vertical()
            .item(
                Section::Favorites.id(),
                "Favorites",
                Some(Glyph::STAR),
            )
            .item(
                Section::Technicals.id(),
                "Technicals",
                Some(Glyph::Lucide(Lucide::ChartLine)),
            )
            .selected(self.section.id())
            .on_change(move |id, _, cx| {
                let section = if id == "favorites" {
                    Section::Favorites
                } else {
                    Section::Technicals
                };
                this.update(cx, |p, cx| {
                    p.section = section;
                    p.selected = None;
                    cx.notify();
                })
                .ok();
            });
        DialogFrame::new(SIZE).id("indicators").nav(nav).body(
            v_flex()
                .size_full()
                .gap(px(S2))
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
                    Input::new(&self.input)
                        .prefix(glyph(Glyph::SEARCH, ICON_SM).color(muted))
                        .cleanable(true),
                )
                .child(SectionHeader::new("Script name"))
                .child(body),
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
