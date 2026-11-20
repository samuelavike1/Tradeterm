//! Lucide icons (<https://lucide.dev>, ISC license) rendered from the icon
//! font bundled in the `lucide-icons` crate.
//!
//! The font is registered as its own family, so icons never replace text
//! glyphs; draw them with [`paint`] (or as text with [`font`]). Being glyphs,
//! they take any theme color and size.

use egui::{Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Painter, Pos2};
use std::sync::Arc;

pub use lucide_icons::Icon;

const FAMILY: &str = "lucide";

/// Registers the Lucide font family.
pub fn register(defs: &mut FontDefinitions) {
    defs.font_data.insert(
        FAMILY.to_owned(),
        Arc::new(FontData::from_static(lucide_icons::LUCIDE_FONT_BYTES)),
    );
    defs.families
        .insert(FontFamily::Name(FAMILY.into()), vec![FAMILY.to_owned()]);
}

pub fn family() -> FontFamily {
    FontFamily::Name(FAMILY.into())
}

pub fn font(size: f32) -> FontId {
    FontId::new(size, family())
}

/// The icon as a one-character string in the Lucide font.
pub fn glyph(icon: Icon) -> String {
    char::from(icon).to_string()
}

/// Draws `icon` centered on `center`.
pub fn paint(p: &Painter, center: Pos2, icon: Icon, size: f32, color: Color32) {
    p.text(
        center,
        Align2::CENTER_CENTER,
        glyph(icon),
        font(size),
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_map_to_private_use_codepoints() {
        for icon in [Icon::Settings, Icon::Search, Icon::X, Icon::ChevronDown] {
            let c = char::from(icon) as u32;
            // Icon fonts live in the Unicode Private Use Area.
            assert!((0xE000..=0xF8FF).contains(&c), "{icon:?} -> {c:#x}");
        }
    }

    #[test]
    fn font_family_registered() {
        let mut defs = FontDefinitions::default();
        register(&mut defs);
        assert!(defs.font_data.contains_key(FAMILY));
        assert!(defs.families.contains_key(&FontFamily::Name(FAMILY.into())));
    }
}
