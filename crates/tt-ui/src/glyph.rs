//! Icons: Lucide, drawn with the Lucide font (§3.2), plus our own 1-stroke
//! SVG set for chart types and drawing tools ([`Glyph::Svg`], served by
//! [`crate::Assets`]).
//!
//! Color follows `text_color`, so hover and active tints need no extra
//! state. [`Glyph::StarFilled`]: the font has no filled star, so it renders
//! gpui-component's `IconName::StarFill` (also Lucide).
//!
//! The SVG icons are drawn in a 28 box with a 1 px stroke and their own
//! inner margin (like TradingView's), so they are shown at 28 (menus, the
//! drawing toolbar, the chart-type button) where a Lucide glyph would be
//! about 20; [`Glyph::svg_size`] maps a Lucide slot size to that.

use crate::tokens::{ICON_MD, ICON_SM, ICON_SVG};
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
    /// One of our SVG icons: its asset path (`tt-icons/<name>.svg`).
    Svg(&'static str),
}

impl PartialEq for Glyph {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Glyph::Lucide(a), Glyph::Lucide(b)) => char::from(*a) == char::from(*b),
            (Glyph::StarFilled, Glyph::StarFilled) => true,
            (Glyph::Svg(a), Glyph::Svg(b)) => a == b,
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
    // App bar (docs/specs/appbar.md).
    pub const UNDO: Glyph = Glyph::Lucide(Lucide::Undo2);
    pub const REDO: Glyph = Glyph::Lucide(Lucide::Redo2);
    pub const CAMERA: Glyph = Glyph::Lucide(Lucide::Camera);
    pub const FULLSCREEN: Glyph = Glyph::Lucide(Lucide::Expand);
    pub const EXIT_FULLSCREEN: Glyph = Glyph::Lucide(Lucide::Shrink);
    pub const TEMPLATES: Glyph = Glyph::Lucide(Lucide::LayoutTemplate);
    pub const DOWNLOAD: Glyph = Glyph::Lucide(Lucide::ImageDown);
    pub const CLIPBOARD_COPY: Glyph = Glyph::Lucide(Lucide::ClipboardCopy);
    pub const MENU: Glyph = Glyph::Lucide(Lucide::Menu);
    pub const SAVE: Glyph = Glyph::Lucide(Lucide::Save);
    pub const SAVED: Glyph = Glyph::Lucide(Lucide::CloudCheck);
    pub const FOLDER_OPEN: Glyph = Glyph::Lucide(Lucide::FolderOpen);
    pub const FILE_PLUS: Glyph = Glyph::Lucide(Lucide::FilePlus);
    pub const FILE_DOWN: Glyph = Glyph::Lucide(Lucide::FileDown);
    pub const RENAME: Glyph = Glyph::Lucide(Lucide::TextCursorInput);
    pub const COMMAND: Glyph = Glyph::Lucide(Lucide::Command);
    // Floating drawing toolbar and comfortable dialogs (section 2.10).
    pub const PENCIL: Glyph = Glyph::Lucide(Lucide::Pencil);
    pub const TYPE: Glyph = Glyph::Lucide(Lucide::Type);
    pub const PAINT_BUCKET: Glyph = Glyph::Lucide(Lucide::PaintBucket);
    pub const GRID_PLUS: Glyph = Glyph::Lucide(Lucide::Grid2x2Plus);

    // Our SVG set (crates/tt-ui/assets/icons, written by scripts/icons/gen.py).
    pub const CHART_BARS: Glyph = Glyph::Svg("tt-icons/chart-bars.svg");
    pub const CHART_CANDLES: Glyph = Glyph::Svg("tt-icons/chart-candles.svg");
    pub const CHART_HOLLOW_CANDLES: Glyph = Glyph::Svg("tt-icons/chart-hollow-candles.svg");
    pub const CHART_VOLUME_CANDLES: Glyph = Glyph::Svg("tt-icons/chart-volume-candles.svg");
    pub const CHART_LINE: Glyph = Glyph::Svg("tt-icons/chart-line.svg");
    pub const CHART_LINE_MARKERS: Glyph = Glyph::Svg("tt-icons/chart-line-markers.svg");
    pub const CHART_STEP_LINE: Glyph = Glyph::Svg("tt-icons/chart-step-line.svg");
    pub const CHART_AREA: Glyph = Glyph::Svg("tt-icons/chart-area.svg");
    pub const CHART_HLC_AREA: Glyph = Glyph::Svg("tt-icons/chart-hlc-area.svg");
    pub const CHART_BASELINE: Glyph = Glyph::Svg("tt-icons/chart-baseline.svg");
    /// The bottom toolbar's Go to (calendar with an arrow).
    pub const GO_TO_DATE: Glyph = Glyph::Svg("tt-icons/go-to-date.svg");
    pub const CHART_COLUMNS: Glyph = Glyph::Svg("tt-icons/chart-columns.svg");
    pub const CHART_HIGH_LOW: Glyph = Glyph::Svg("tt-icons/chart-high-low.svg");
    pub const CHART_HEIKIN_ASHI: Glyph = Glyph::Svg("tt-icons/chart-heikin-ashi.svg");
    pub const CHART_RENKO: Glyph = Glyph::Svg("tt-icons/chart-renko.svg");
    pub const CHART_LINE_BREAK: Glyph = Glyph::Svg("tt-icons/chart-line-break.svg");
    pub const DRAW_CURSOR: Glyph = Glyph::Svg("tt-icons/draw-cursor-cross.svg");
    pub const DRAW_TREND_LINE: Glyph = Glyph::Svg("tt-icons/draw-trend-line.svg");
    pub const DRAW_RAY: Glyph = Glyph::Svg("tt-icons/draw-ray.svg");
    pub const DRAW_INFO_LINE: Glyph = Glyph::Svg("tt-icons/draw-info-line.svg");
    pub const DRAW_EXTENDED_LINE: Glyph = Glyph::Svg("tt-icons/draw-extended-line.svg");
    pub const DRAW_TREND_ANGLE: Glyph = Glyph::Svg("tt-icons/draw-trend-angle.svg");
    pub const DRAW_HORIZONTAL_LINE: Glyph = Glyph::Svg("tt-icons/draw-horizontal-line.svg");
    pub const DRAW_HORIZONTAL_RAY: Glyph = Glyph::Svg("tt-icons/draw-horizontal-ray.svg");
    pub const DRAW_VERTICAL_LINE: Glyph = Glyph::Svg("tt-icons/draw-vertical-line.svg");
    pub const DRAW_CROSS_LINE: Glyph = Glyph::Svg("tt-icons/draw-cross-line.svg");
    pub const DRAW_PARALLEL_CHANNEL: Glyph = Glyph::Svg("tt-icons/draw-parallel-channel.svg");
    pub const DRAW_ARROW: Glyph = Glyph::Svg("tt-icons/draw-arrow.svg");
    pub const DRAW_FIB_RETRACEMENT: Glyph = Glyph::Svg("tt-icons/draw-fib-retracement.svg");
    pub const DRAW_FIB_EXTENSION: Glyph = Glyph::Svg("tt-icons/draw-fib-extension.svg");
    pub const DRAW_RECTANGLE: Glyph = Glyph::Svg("tt-icons/draw-rectangle.svg");
    pub const DRAW_PRICE_RANGE: Glyph = Glyph::Svg("tt-icons/draw-price-range.svg");
    pub const DRAW_DATE_RANGE: Glyph = Glyph::Svg("tt-icons/draw-date-range.svg");
    pub const DRAW_DATE_PRICE_RANGE: Glyph = Glyph::Svg("tt-icons/draw-date-price-range.svg");
    pub const DRAW_MAGNET: Glyph = Glyph::Svg("tt-icons/draw-magnet.svg");
    pub const DRAW_KEEP_DRAWING: Glyph = Glyph::Svg("tt-icons/draw-keep-drawing.svg");
    pub const DRAW_LOCK: Glyph = Glyph::Svg("tt-icons/draw-lock.svg");
    pub const DRAW_UNLOCK: Glyph = Glyph::Svg("tt-icons/draw-unlock.svg");
    pub const DRAW_EYE: Glyph = Glyph::Svg("tt-icons/draw-eye.svg");
    pub const DRAW_EYE_OFF: Glyph = Glyph::Svg("tt-icons/draw-eye-off.svg");
    pub const DRAW_TRASH: Glyph = Glyph::Svg("tt-icons/draw-trash.svg");
    /// Alarm clock (alerts: app bar, panel, alert tags).
    pub const ALARM: Glyph = Glyph::Svg("tt-icons/alarm-clock.svg");
    /// Alarm clock with a plus ("Create alert", "Add alert").
    pub const ALARM_PLUS: Glyph = Glyph::Svg("tt-icons/alarm-clock-plus.svg");
    // Right sidebar (docs/specs/right-panel.md).
    pub const PANEL_WATCHLIST: Glyph = Glyph::Svg("tt-icons/panel-watchlist.svg");
    pub const PANEL_ALERTS: Glyph = Glyph::Svg("tt-icons/panel-alerts.svg");
    pub const PANEL_OBJECT_TREE: Glyph = Glyph::Svg("tt-icons/panel-object-tree.svg");
    pub const TREE_INDICATOR: Glyph = Glyph::Svg("tt-icons/tree-indicator.svg");

    /// Every SVG glyph with a name (gallery contact sheet).
    pub const SVG_SET: [(&'static str, Glyph); 47] = [
        ("Bars", Glyph::CHART_BARS),
        ("Candles", Glyph::CHART_CANDLES),
        ("Hollow candles", Glyph::CHART_HOLLOW_CANDLES),
        ("Volume candles", Glyph::CHART_VOLUME_CANDLES),
        ("Line", Glyph::CHART_LINE),
        ("Line with markers", Glyph::CHART_LINE_MARKERS),
        ("Step line", Glyph::CHART_STEP_LINE),
        ("Area", Glyph::CHART_AREA),
        ("HLC area", Glyph::CHART_HLC_AREA),
        ("Baseline", Glyph::CHART_BASELINE),
        ("Columns", Glyph::CHART_COLUMNS),
        ("High-low", Glyph::CHART_HIGH_LOW),
        ("Heikin Ashi", Glyph::CHART_HEIKIN_ASHI),
        ("Renko", Glyph::CHART_RENKO),
        ("Line break", Glyph::CHART_LINE_BREAK),
        ("Cross", Glyph::DRAW_CURSOR),
        ("Trend line", Glyph::DRAW_TREND_LINE),
        ("Ray", Glyph::DRAW_RAY),
        ("Info line", Glyph::DRAW_INFO_LINE),
        ("Extended line", Glyph::DRAW_EXTENDED_LINE),
        ("Trend angle", Glyph::DRAW_TREND_ANGLE),
        ("Horizontal line", Glyph::DRAW_HORIZONTAL_LINE),
        ("Horizontal ray", Glyph::DRAW_HORIZONTAL_RAY),
        ("Vertical line", Glyph::DRAW_VERTICAL_LINE),
        ("Cross line", Glyph::DRAW_CROSS_LINE),
        ("Parallel channel", Glyph::DRAW_PARALLEL_CHANNEL),
        ("Arrow", Glyph::DRAW_ARROW),
        ("Fib retracement", Glyph::DRAW_FIB_RETRACEMENT),
        ("Fib extension", Glyph::DRAW_FIB_EXTENSION),
        ("Rectangle", Glyph::DRAW_RECTANGLE),
        ("Price range", Glyph::DRAW_PRICE_RANGE),
        ("Date range", Glyph::DRAW_DATE_RANGE),
        ("Date and price range", Glyph::DRAW_DATE_PRICE_RANGE),
        ("Magnet", Glyph::DRAW_MAGNET),
        ("Keep drawing", Glyph::DRAW_KEEP_DRAWING),
        ("Lock", Glyph::DRAW_LOCK),
        ("Unlock", Glyph::DRAW_UNLOCK),
        ("Show", Glyph::DRAW_EYE),
        ("Hide", Glyph::DRAW_EYE_OFF),
        ("Remove", Glyph::DRAW_TRASH),
        ("Alert", Glyph::ALARM),
        ("Add alert", Glyph::ALARM_PLUS),
        ("Watchlist", Glyph::PANEL_WATCHLIST),
        ("Alerts", Glyph::PANEL_ALERTS),
        ("Object tree", Glyph::PANEL_OBJECT_TREE),
        ("Indicator", Glyph::TREE_INDICATOR),
        ("Go to date", Glyph::GO_TO_DATE),
    ];

    /// The font character (`None` for [`Glyph::StarFilled`] and SVGs).
    pub fn char(self) -> Option<char> {
        match self {
            Glyph::Lucide(i) => Some(char::from(i)),
            Glyph::StarFilled | Glyph::Svg(_) => None,
        }
    }

    /// One of our SVG icons.
    pub fn is_svg(self) -> bool {
        matches!(self, Glyph::Svg(_))
    }

    /// Display size in a slot sized for a Lucide glyph of `lucide` px. SVG
    /// icons carry their own inner margin (28 box, about 20 of ink), so they
    /// are drawn larger, at one of three sizes: 28 for slots of 18 and up
    /// (toolbars, large menu rows), 24 for 16 (menu rows), 20 below. Font
    /// glyphs keep `lucide`.
    pub fn svg_size(self, lucide: f32) -> f32 {
        match self {
            Glyph::Svg(_) if lucide >= ICON_MD => ICON_SVG,
            Glyph::Svg(_) if lucide >= ICON_SM => 24.0,
            Glyph::Svg(_) => 20.0,
            _ => lucide,
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
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
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
            // `svg()` paints with its own text color (not the inherited
            // one), so pass the inherited color unless one is set.
            Glyph::Svg(path) => base.child(
                svg()
                    .path(path)
                    .flex_none()
                    .size(s)
                    .text_color(self.color.unwrap_or(window.text_style().color)),
            ),
        }
    }
}
