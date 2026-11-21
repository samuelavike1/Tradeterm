//! The camera button: TradingView's CHART SNAPSHOT menu. Download image
//! (Ctrl+Alt+S) and Copy image (Ctrl+Shift+S) render the active chart with
//! [`crate::snapshot`]. Copy link / Open in new tab / Post image need an
//! image host (BACKLOG.md).

use crate::app::TradeTerm;
use crate::top_toolbar::{CopySnapshot, DownloadSnapshot};
use gpui_kit::component::WindowExt as _;
use gpui_kit::*;
use std::path::PathBuf;
use tt_ui::prelude::*;

pub fn snapshot_menu(cx: &mut Context<TradeTerm>) -> impl IntoElement + use<> {
    let app = cx.entity().downgrade();
    ToolbarButton::icon("snapshot", Glyph::CAMERA)
        .tooltip("Take a snapshot")
        .menu_right()
        .menu(move |menu, window, _| {
            let (dl, copy) = (app.clone(), app.clone());
            menu.id("snapshot-menu")
                .width(MenuWidth::Lg)
                .section("Chart snapshot")
                .row(
                    MenuRow::new("snap-download", "Download image")
                        .icon(Glyph::DOWNLOAD)
                        .shortcut_action(&DownloadSnapshot, None, window)
                        .on_select(move |window, cx| {
                            dl.update(cx, |a, cx| a.download_snapshot(window, cx)).ok();
                        }),
                )
                .row(
                    MenuRow::new("snap-copy", "Copy image")
                        .icon(Glyph::COPY)
                        .shortcut_action(&CopySnapshot, None, window)
                        .on_select(move |window, cx| {
                            copy.update(cx, |a, cx| a.copy_snapshot(window, cx)).ok();
                        }),
                )
        })
}

/// Where save dialogs start: the user's Downloads folder if it exists.
pub fn downloads_dir() -> PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from);
    match home {
        Some(h) if h.join("Downloads").is_dir() => h.join("Downloads"),
        Some(h) => h,
        None => std::env::current_dir().unwrap_or_default(),
    }
}

/// Asks for a path (save dialog), then writes `bytes` there in the
/// background and reports the outcome as a notification.
pub fn save_bytes_as(
    name: String,
    bytes: Vec<u8>,
    what: &'static str,
    window: &mut Window,
    cx: &mut Context<TradeTerm>,
) {
    let rx = cx.prompt_for_new_path(&downloads_dir(), Some(&name));
    cx.spawn_in(window, async move |_, cx| {
        let Ok(Ok(Some(path))) = rx.await else {
            return;
        };
        let write = cx
            .background_spawn({
                let path = path.clone();
                async move { std::fs::write(&path, bytes) }
            })
            .await;
        let msg = match write {
            Ok(()) => format!("{what} saved to {}", path.display()),
            Err(e) => format!("Could not save the {}: {e}", what.to_lowercase()),
        };
        cx.update(|window, cx| window.push_notification(msg, cx))
            .ok();
    })
    .detach();
}

impl TradeTerm {
    fn snapshot(&self, window: &Window, cx: &mut App) -> Option<crate::snapshot::Snapshot> {
        let chart = self.tab_chart()?.clone();
        crate::snapshot::render(&chart, window.scale_factor(), cx)
    }

    /// "Download image": the snapshot as a PNG file.
    pub(crate) fn download_snapshot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.snapshot(window, cx) else {
            window.push_notification("Nothing to snapshot yet", cx);
            return;
        };
        save_bytes_as(s.file_name, s.png, "Image", window, cx);
    }

    /// "Copy image": the snapshot as a PNG on the clipboard.
    pub(crate) fn copy_snapshot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.snapshot(window, cx) else {
            window.push_notification("Nothing to snapshot yet", cx);
            return;
        };
        let image = Image::from_bytes(ImageFormat::Png, s.png);
        cx.write_to_clipboard(ClipboardItem::new_image(&image));
        window.push_notification("Image copied to clipboard", cx);
    }
}
