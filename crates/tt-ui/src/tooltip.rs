//! Tooltips (§3.5): label in sentence case, no trailing period, shortcut as
//! a `Kbd` chip (`Ctrl+T`, `Alt+Shift+R`, `/`), never in parentheses.
//!
//! Kit buttons use gpui-component's *managed* tooltips (show delay, warm
//! hand-off, hidden when the trigger moves or is clicked). [`tip`] and
//! [`tip_action`] are for other elements (rows, raw divs): gpui's native
//! element tooltip, which does not hide when its trigger moves. Prefer kit
//! buttons for anything that moves after a click.

use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::*;

/// Parses a shortcut written either way: gpui spec (`"alt-shift-r"`,
/// `"ctrl-t"`, `"/"`) or display form (`"Alt+Shift+R"`, `"Alt + T"`).
pub fn keystroke(keys: &str) -> Option<Keystroke> {
    let k = keys.trim();
    if k.is_empty() {
        return None;
    }
    let spec = if k.len() > 1 && k.contains('+') {
        k.split('+')
            .map(|p| p.trim().to_lowercase())
            .collect::<Vec<_>>()
            .join("-")
    } else {
        k.to_lowercase()
    };
    let os = if cfg!(target_os = "macos") {
        OperatingSystem::Mac
    } else if cfg!(target_os = "windows") {
        OperatingSystem::Windows
    } else {
        OperatingSystem::Linux
    };
    Keystroke::parse_with_os(&spec, os).ok()
}

/// A `Kbd` chip for a shortcut (see [`keystroke`] for accepted forms).
pub fn kbd(keys: &str) -> Option<Kbd> {
    keystroke(keys).map(Kbd::new)
}

/// One format everywhere: `Alt+T`, `Ctrl+Shift+D`, `/` (platform style).
pub fn format_keys(keys: &str) -> String {
    keystroke(keys)
        .map(|k| Kbd::format(&k))
        .unwrap_or_else(|| keys.to_owned())
}

/// Text tooltip with an optional shortcut chip, for `.tooltip(..)` on
/// elements that are not kit buttons.
pub fn tip(
    label: impl Into<SharedString>,
    keys: Option<&str>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let label: SharedString = label.into();
    let keys = keys.map(str::to_owned);
    move |window, cx| {
        Tooltip::new(label.clone())
            .key_binding(keys.as_deref().and_then(kbd))
            .build(window, cx)
    }
}

/// Tooltip showing the key bound to `action` (looked up at show time, in
/// `context` when given, e.g. `Some("Tradeterm")`).
pub fn tip_action(
    label: impl Into<SharedString>,
    action: &dyn Action,
    context: Option<&str>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let label: SharedString = label.into();
    let action = action.boxed_clone();
    let context: Option<SharedString> = context.map(|c| SharedString::from(c.to_owned()));
    move |window, cx| {
        Tooltip::new(label.clone())
            .action(action.as_ref(), context.as_deref())
            .build(window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn shortcut_forms_agree() {
        let a = keystroke("alt-shift-r").unwrap();
        let b = keystroke("Alt + Shift + R").unwrap();
        let c = keystroke("Alt+Shift+R").unwrap();
        assert_eq!(a, b);
        assert_eq!(b, c);
        assert!(a.modifiers.alt && a.modifiers.shift && a.key == "r");
        assert_eq!(keystroke("/").unwrap().key, "/");
        assert!(keystroke("").is_none());
    }

    #[cfg(not(target_os = "macos"))]
    #[::core::prelude::v1::test]
    fn display_format() {
        assert_eq!(format_keys("alt-t"), "Alt+T");
        assert_eq!(format_keys("Ctrl + Shift + D"), "Ctrl+Shift+D");
    }
}
