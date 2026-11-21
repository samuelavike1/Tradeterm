//! One icon set: Lucide, drawn with the Lucide font (§3.2).
//!
//! Color follows `text_color`, so hover and active tints need no extra
//! state. The one exception is [`Glyph::StarFilled`]: the font has no filled
//! star, so it renders gpui-component's `IconName::StarFill` (also Lucide).

use gpui_kit::component::{Icon, IconName};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use lucide_icons::Icon as Lucide;
use std::borrow::Cow;

/// Font family of the Lucide icon font registered by [`register_icon_font`].
/// The same family the chart canvas uses (`tt_chart_gpui::ICON_FONT`).
pub const ICON_FONT: &str = "lucide";

struct IconFontRegistered;
impl Global for IconFontRegistered {}

/// Registers the Lucide icon font once (later calls do nothing).
pub fn register_icon_font(cx: &mut App) {
    if cx.has_global::<IconFontRegistered>() {
        return;
    }
    cx.text_system()
        .add_fonts(vec![Cow::Borrowed(lucide_icons::LUCIDE_FONT_BYTES)])
        .expect("register the Lucide icon font");
    cx.set_global(IconFontRegistered);
}

/// An icon.
#[derive(Clone, Copy, Debug)]
pub enum Glyph {
    Lucide(Lucide),
    StarFilled,
}

impl PartialEq for Glyph {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Glyph::Lucide(a), Glyph::Lucide(b)) => char::from(*a) == char::from(*b),
            (Glyph::StarFilled, Glyph::StarFilled) => true,
            _ => false,
        }
    }
}

impl Eq for Glyph {}

impl From<Lucide> for Glyph {
    fn from(i: Lucide) -> Self {
        Glyph::Lucide(i)
    }
}

impl Glyph {
    pub const PLUS: Glyph = Glyph::Lucide(Lucide::Plus);
    pub const X: Glyph = Glyph::Lucide(Lucide::X);
    pub const CHECK: Glyph = Glyph::Lucide(Lucide::Check);
    pub const SEARCH: Glyph = Glyph::Lucide(Lucide::Search);
    pub const SETTINGS: Glyph = Glyph::Lucide(Lucide::Settings);
    pub const SUN: Glyph = Glyph::Lucide(Lucide::Sun);
    pub const MOON: Glyph = Glyph::Lucide(Lucide::Moon);
    pub const CHEVRON_DOWN: Glyph = Glyph::Lucide(Lucide::ChevronDown);
    pub const CHEVRON_UP: Glyph = Glyph::Lucide(Lucide::ChevronUp);
    pub const CHEVRON_LEFT: Glyph = Glyph::Lucide(Lucide::ChevronLeft);
    pub const CHEVRON_RIGHT: Glyph = Glyph::Lucide(Lucide::ChevronRight);
    pub const STAR: Glyph = Glyph::Lucide(Lucide::Star);
    pub const EYE: Glyph = Glyph::Lucide(Lucide::Eye);
    pub const EYE_OFF: Glyph = Glyph::Lucide(Lucide::EyeOff);
    pub const LOCK: Glyph = Glyph::Lucide(Lucide::Lock);
    pub const LOCK_OPEN: Glyph = Glyph::Lucide(Lucide::LockOpen);
    pub const TRASH: Glyph = Glyph::Lucide(Lucide::Trash2);
    pub const MAGNET: Glyph = Glyph::Lucide(Lucide::Magnet);
    pub const CROSSHAIR: Glyph = Glyph::Lucide(Lucide::Crosshair);
    pub const PENCIL_LINE: Glyph = Glyph::Lucide(Lucide::PencilLine);
    pub const GRIP_VERTICAL: Glyph = Glyph::Lucide(Lucide::GripVertical);
    pub const SQUARE_FUNCTION: Glyph = Glyph::Lucide(Lucide::SquareFunction);
    pub const MAXIMIZE: Glyph = Glyph::Lucide(Lucide::Maximize2);
    pub const MINIMIZE: Glyph = Glyph::Lucide(Lucide::Minimize2);
    pub const ELLIPSIS: Glyph = Glyph::Lucide(Lucide::Ellipsis);
    pub const PALETTE: Glyph = Glyph::Lucide(Lucide::Palette);
    pub const COPY: Glyph = Glyph::Lucide(Lucide::Copy);
    pub const CHART_CANDLESTICK: Glyph = Glyph::Lucide(Lucide::ChartCandlestick);

    /// The font character (`None` for [`Glyph::StarFilled`]).
    pub fn char(self) -> Option<char> {
        match self {
            Glyph::Lucide(i) => Some(char::from(i)),
            Glyph::StarFilled => None,
        }
    }
}

/// A glyph in a `size × size` box, centered, `line_height = size` (no
/// vertical drift against neighbours).
#[derive(IntoElement)]
pub struct GlyphEl {
    glyph: Glyph,
    size: f32,
    color: Option<Hsla>,
}

/// A glyph at `size` px (use the `ICON_*` tokens).
pub fn glyph(g: impl Into<Glyph>, size: f32) -> GlyphEl {
    GlyphEl {
        glyph: g.into(),
        size,
        color: None,
    }
}

impl GlyphEl {
    /// Fixed color (default: inherit `text_color`).
    pub fn color(mut self, c: Hsla) -> Self {
        self.color = Some(c);
        self
    }
}

impl RenderOnce for GlyphEl {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let s = px(self.size);
        let base = div()
            .flex_none()
            .size(s)
            .flex()
            .items_center()
            .justify_center()
            .when_some(self.color, |d, c| d.text_color(c));
        match self.glyph {
            Glyph::Lucide(i) => base
                .font_family(ICON_FONT)
                .text_size(s)
                .line_height(s)
                .child(char::from(i).to_string()),
            Glyph::StarFilled => base.child(
                Icon::new(IconName::StarFill)
                    .size(s)
                    .when_some(self.color, |i, c| i.text_color(c)),
            ),
        }
    }
}
