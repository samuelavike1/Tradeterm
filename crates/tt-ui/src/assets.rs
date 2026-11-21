//! The app's asset source: our own SVG icons (`tt-icons/*.svg`, from
//! `crates/tt-ui/assets/icons`, embedded at compile time) on top of
//! gpui-kit's bundled Lucide SVGs (`icons/*.svg`, used by gpui-component
//! internals).
//!
//! Register it instead of `gpui_kit::assets::Assets`:
//! `application().with_assets(tt_ui::Assets)`.
//!
//! The SVGs are tradeterm's own 1-stroke set (28 × 28 viewBox, stroke 1,
//! `currentColor`), written by `scripts/icons/gen.py`. gpui rasterizes them
//! as alpha masks and tints them with the element's text color, so
//! [`crate::Glyph::Svg`] follows `text_color` like the font glyphs.

use gpui_kit::{AssetSource, Result, SharedString};
use std::borrow::Cow;

/// Path prefix of our icons in the asset source.
pub const ICON_PREFIX: &str = "tt-icons/";

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        /// (asset path, SVG bytes) of every embedded icon.
        pub const ICONS: &[(&str, &[u8])] = &[
            $((
                concat!("tt-icons/", $name, ".svg"),
                include_bytes!(concat!("../assets/icons/", $name, ".svg")),
            ),)*
        ];
    };
}

icons!(
    "chart-bars",
    "chart-candles",
    "chart-hollow-candles",
    "chart-volume-candles",
    "chart-line",
    "chart-line-markers",
    "chart-step-line",
    "chart-area",
    "chart-hlc-area",
    "chart-baseline",
    "chart-columns",
    "chart-high-low",
    "chart-heikin-ashi",
    "chart-renko",
    "chart-line-break",
    "draw-cursor-cross",
    "draw-trend-line",
    "draw-ray",
    "draw-info-line",
    "draw-extended-line",
    "draw-trend-angle",
    "draw-horizontal-line",
    "draw-horizontal-ray",
    "draw-vertical-line",
    "draw-cross-line",
    "draw-parallel-channel",
    "draw-arrow",
    "draw-fib-retracement",
    "draw-fib-extension",
    "draw-rectangle",
    "draw-price-range",
    "draw-date-range",
    "draw-date-price-range",
    "draw-magnet",
    "draw-keep-drawing",
    "draw-lock",
    "draw-unlock",
    "draw-eye",
    "draw-eye-off",
    "draw-trash",
    "alarm-clock",
    "alarm-clock-plus",
    "go-to-date",
    "panel-watchlist",
    "panel-alerts",
    "panel-object-tree",
    "tree-indicator",
    "app-fullscreen",
    "app-exit-fullscreen",
    "app-settings",
);

/// Our icons plus gpui-kit's default asset bundle.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.starts_with(ICON_PREFIX) {
            return Ok(ICONS
                .iter()
                .find(|(p, _)| *p == path)
                .map(|(_, bytes)| Cow::Borrowed(*bytes)));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut out = gpui_kit::assets::Assets.list(path)?;
        out.extend(
            ICONS
                .iter()
                .filter(|(p, _)| p.starts_with(path))
                .map(|(p, _)| SharedString::from(*p)),
        );
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn icons_are_one_stroke_28_box_svgs() {
        for (path, bytes) in ICONS {
            let s = std::str::from_utf8(bytes).unwrap();
            assert!(s.contains(r#"viewBox="0 0 28 28""#), "{path}");
            assert!(s.contains(r#"stroke-width="1""#), "{path}");
            assert!(s.contains(r#"stroke="currentColor""#), "{path}");
            assert!(!s.contains("stroke-width=\"2"), "{path}");
        }
    }

    #[::core::prelude::v1::test]
    fn loads_ours_and_falls_back_to_gpui_kit() {
        let a = Assets;
        assert!(a.load("tt-icons/chart-candles.svg").unwrap().is_some());
        assert!(a.load("tt-icons/nope.svg").unwrap().is_none());
        assert!(a.load("icons/check.svg").unwrap().is_some());
        assert_eq!(a.list("tt-icons/").unwrap().len(), ICONS.len());
    }

    #[::core::prelude::v1::test]
    fn every_svg_glyph_has_an_asset() {
        assert_eq!(crate::Glyph::SVG_SET.len(), ICONS.len());
        for (name, g) in crate::Glyph::SVG_SET {
            let crate::Glyph::Svg(path) = g else {
                panic!("{name} is not an SVG glyph");
            };
            assert!(ICONS.iter().any(|(p, _)| *p == path), "{name}: {path}");
        }
    }
}
