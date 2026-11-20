//! Use the operating system's UI fonts.
//!
//! egui's built-in proportional font is Ubuntu *Light*. Its thin strokes are
//! mostly anti-aliased gray at UI sizes, which makes all text look faded. We
//! load the platform's regular-weight UI and monospace fonts instead (no
//! bundled files), keeping egui's fonts as fallbacks for symbols such as ⚙ ▲.

use egui::{FontData, FontDefinitions, FontFamily};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

/// A font file and the face index inside it (for `.ttc` collections).
type Candidate = (PathBuf, u32);

fn ui_candidates() -> Vec<Candidate> {
    #[cfg(target_os = "windows")]
    {
        let fonts = windows_fonts_dir();
        vec![(fonts.join("segoeui.ttf"), 0), (fonts.join("arial.ttf"), 0)]
    }
    #[cfg(target_os = "macos")]
    {
        vec![
            ("/System/Library/Fonts/SFNS.ttf".into(), 0),
            ("/System/Library/Fonts/Helvetica.ttc".into(), 0),
            ("/System/Library/Fonts/Supplemental/Arial.ttf".into(), 0),
        ]
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        [
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
        ]
        .iter()
        .map(|p| (PathBuf::from(p), 0))
        .collect()
    }
}

fn mono_candidates() -> Vec<Candidate> {
    #[cfg(target_os = "windows")]
    {
        vec![(windows_fonts_dir().join("consola.ttf"), 0)]
    }
    #[cfg(target_os = "macos")]
    {
        vec![
            ("/System/Library/Fonts/SFNSMono.ttf".into(), 0),
            ("/System/Library/Fonts/Menlo.ttc".into(), 0),
        ]
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        [
            "/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf",
            "/usr/share/fonts/noto/NotoSansMono-Regular.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
            "/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono.ttf",
        ]
        .iter()
        .map(|p| (PathBuf::from(p), 0))
        .collect()
    }
}

#[cfg(target_os = "windows")]
fn windows_fonts_dir() -> PathBuf {
    std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join("Fonts")
}

/// Reads the first candidate that exists and loads.
fn first_existing(candidates: &[Candidate]) -> Option<(&Path, FontData)> {
    candidates.iter().find_map(|(path, index)| {
        let bytes = std::fs::read(path).ok()?;
        let mut data = FontData::from_owned(bytes);
        data.index = *index;
        Some((path.as_path(), data))
    })
}

/// Installs system fonts in front of egui's defaults (falling back to egui's
/// built-in fonts if none are found) plus the Lucide icon font.
pub fn install(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    for (family, name, candidates) in [
        (FontFamily::Proportional, "system-ui", ui_candidates()),
        (FontFamily::Monospace, "system-mono", mono_candidates()),
    ] {
        match first_existing(&candidates) {
            Some((path, data)) => {
                info!(font = %path.display(), ?family, "using system font");
                defs.font_data.insert(name.to_owned(), Arc::new(data));
                defs.families
                    .entry(family)
                    .or_default()
                    .insert(0, name.to_owned());
            }
            None => warn!(?family, "no system font found; using egui's built-in font"),
        }
    }
    crate::icons::register(&mut defs);
    ctx.set_fonts(defs);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_first_readable_candidate() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.ttf");
        let present = dir.path().join("present.ttf");
        std::fs::write(&present, b"not really a font").unwrap();
        let candidates = [(missing, 0), (present.clone(), 2)];
        let found = first_existing(&candidates).unwrap();
        assert_eq!(found.0, present.as_path());
        assert_eq!(found.1.index, 2);
        assert!(first_existing(&[]).is_none());
    }

    #[test]
    fn every_platform_has_candidates() {
        assert!(!ui_candidates().is_empty());
        assert!(!mono_candidates().is_empty());
    }
}
