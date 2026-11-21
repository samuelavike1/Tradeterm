# tradeterm UI guidelines and consistency audit

Status: proposal. Audit date: 2026-11-20. Scope: every gpui surface of `tradeterm` (title bar, tabs, top toolbar, menus, dialogs, status bar, left drawing toolbar) and the chart's own controls (`tt-chart-gpui` overlay widgets, `tt-chart` canvas legend and tags).

Goal: TradingView-level polish. Every control of one kind looks and behaves the same everywhere, colors come from tokens, motion is short and interruptible, and data (bars, prices, crosshair) is never animated. Performance outranks convenience: nothing below adds per-frame work to the chart layers.

How to use this document:

- §1 lists what is inconsistent today, with file:line and the fix.
- §2 is the token spec. Numbers in code come from here, nowhere else.
- §3 is the shared component kit (`crates/tt-ui`) with API sketches on top of gpui-component 0.10.
- §4 splits the migration into slices that engineers can apply in parallel once slice 0 (the kit) has landed.

Line numbers refer to the tree as audited. Screenshots referenced are in the session scratchpad (`draw_base.png`, `draw_flyout.png`, `draw_trend.png`, `draw_fib.png`, `draw_measure.png`, `pane_hover2.png`, `pane_collapsed.png`, `pane_max2.png`, `corner_menu.png`, `corner_hover2.png`, `settings_dlg.png`, plus `tf_menu.png`, `ind_dialog.png`, `ind_gear3.png`).

---

## 0. What the audit found, in brief

1. **Four button implementations** with four sizes, radii and hover colors: gpui-component `Button` (24 px small / 32 px medium, hover = gpui-component `accent`), hand-rolled `div` buttons (20, 22, 34 px, hover = tt `selection` or opacity 0.85), and canvas-drawn legend buttons (20 px, no per-button hover).
2. **Two icon sets mixed on the same surface.** Lucide font glyphs (`ui::lucide`, `chart.rs glyph()`) next to gpui-component `IconName` SVGs (Plus, Sun, Settings, Search, ChevronDown, Star, Close, ChartPie). They differ in stroke weight, optical size and vertical alignment. The flyout chevron (10 px glyph in a 20 px button) renders as a misplaced "›".
3. **Two menu systems.** gpui-component `PopupMenu` (26 px rows, 14 px text, check on the left, keyboard nav) and custom `Popover` lists (32 px rows, 13.5 px text, inverted selection, no keyboard nav). Their hover colors differ because they read two palettes (next item).
4. **Two palettes.** `tt-theme` drives our hand-rolled widgets and the chart; gpui-component keeps its own shadcn "neutral" palette (`ui::apply_theme` only switches mode). So hover, selection and border shades differ between components and custom rows.
5. **Five dialog layouts.** Widths 460/520/680/780/780. Footers are "Reset to defaults" (plain button), "Defaults ▾" (menu), or none. The tab styles are an underline (indicator settings) or a sidebar (chart settings). Label columns are 150 or 200 px, and row heights are 36 or 38. Checkboxes sit outside the label grid in indicator settings.
6. **Three tooltip mechanisms.** Managed tooltips (gpui-component `Button::tooltip`, with show delay and a warm hand-off), raw gpui `.tooltip(Tooltip::new().build)` on divs (different timing, and it goes stale after a click moves the trigger: see `pane_collapsed.png` and `pane_max2.png`), and none (the titlebar gear, tab pills, stars, legend buttons, quote chips). Shortcut formats also vary: `(Ctrl+T)`, `(Alt + T)`, `(/)`.
7. **Eleven text sizes**: 9.5, 10, 11, 11.5, 12, 12.5, 13, 13.5, 14 (component `text_sm`), 14.5 and 15. **Seven radii**: 2, 3, 4, 5, 6, 7, 8, plus full and the component theme radius. **Nine control heights**: 20, 22, 24, 30, 32, 34, 36, 38, 48.
8. **Colors hard-coded** outside `tt-theme`: the active-tool blue `#2962ff`, the favorite gold `#f5b52e` (twice), the "connecting" amber, `gpui_kit::red()` and `Consolas`. Selection is shown as an *inverted* row or chip (text color as background) in several places. In the light theme that becomes black rows.
9. **Z-order and overlap bugs.** The drawing flyout is centered on its trigger and covers the tab strip and top toolbar. The floating drawing toolbar is placed with a magic `axis_w + 70` offset, collides with the OHLC readout, and is 3 px off the pane buttons' baseline. Pane buttons fade in but never fade out. Stale tooltips survive layout changes.
10. **A bug in chart Settings.** The dialog chrome takes its colors from the *draft chart style*. Editing "Scales → Text" recolors the dialog's section headers, and editing "Scales → Lines" recolors its footer border.

---

## 1. Inventory of inconsistencies

Legend for the **Kit** column: components defined in §3 (`IconButton`, `ToolbarButton`, `Menu`/`MenuRow`, `SectionHeader`, `DialogFrame`, `DialogFooter`, `NavTabs`, `FormRow`, `NumberField`, `Select`, `ColorField`, `Glyph`, `tip`, tokens `T::*`).

### 1.1 App chrome: `crates/tradeterm/src/app.rs`, `ui.rs`

| # | Location | What is wrong | Fix |
|---|---|---|---|
| A1 | app.rs:800-805 | New-tab `Button … .small().icon(IconName::Plus)`: a 24 px SVG icon button, while the tab pills next to it are 30 px tall. | `IconButton::new("new-tab", Glyph::Plus).size(Sm)`, vertically centered in the 28 px tab height. Tooltip via `tip("New tab", "Ctrl+T")`. |
| A2 | app.rs:812-822 | Theme toggle uses `IconName::Sun/Moon` (SVG set), while the rest of the chrome uses Lucide glyphs. | `IconButton` with `Glyph::Sun`/`Glyph::Moon` (font). |
| A3 | app.rs:824-827 | The settings gear button has **no tooltip**. | `.tooltip("Settings")`. |
| A4 | app.rs:828-868 | The settings dropdown is a `PopupMenu` (26 px rows). Other toolbar menus are custom popovers (32 px rows). "Load 1M synthetic bars" does not show its shortcut (Ctrl+Shift+D) and is a debug item in the user menu. | Build it with `Menu` (§3.4). Rows show `shortcut("Ctrl+Shift+D")`. Move debug items under a `SectionHeader("Developer")`, or hide them behind a cfg/env flag. |
| A5 | app.rs:897-903 | Interval buttons are `Button::ghost().small()` (24 px tall, gpui-component hover/selected colors) in a 40 px toolbar. | `ToolbarButton::text(label).selected(t == tf)` at the 32 px control height. |
| A6 | app.rs:912-919 | Favorite chart-type buttons are `Button … .child(lucide(…,17.))`. Because a child is present, gpui-component lays them out as *text* buttons (h 24, px 8): 33 px wide and not square, unlike the icon-only buttons. | `IconButton::new(id, chart_type_glyph(fav)).size(Md)` (square 32). |
| A7 | app.rs:923-929 | Toolbar is `h 40 / px_2 / gap_1`, with a trailing `div().flex_1()` and nothing on the right. | `Toolbar` frame: `T::H_TOOLBAR`, `T::S2` padding, `T::S1` gap. The right group is reserved for future undo/redo, layout and fullscreen. |
| A8 | app.rs:931-937 | Ticker is a ghost button with an SVG `IconName::Search`. TradingView shows the symbol as a pill-shaped search field. | `ToolbarButton::field(symbol).leading(Glyph::Search)` with `panel_alt` background, `R::MD` radius and 14 px text. |
| A9 | app.rs:945-957 | The Indicators button builds `h_flex(lucide 17 + text)` inside `Button::child`, so the icon size (17) matches nothing else. | `ToolbarButton::text("Indicators").leading(Glyph::SquareFunction)` (icon 18, `T::ICON_MD`). |
| A10 | ui.rs:62-64 | Separator is `h 20, mx_1`, the drawing toolbar uses `w 26 h 1 my_1` (drawing_toolbar.rs:325), and menus use `h 1 my_1` (menus.rs:286, 336). | `Divider::vertical()` / `Divider::horizontal()` from the kit: 1 px, `border` token, inset per the token table. |
| A11 | ui.rs:117-201 | Tab pill: h 30, radius 7, text 13, badge 18 with text 10, change text 12, close 18 with radius 4 and icon 13. None of these values exist elsewhere. Hover is `mix(chrome, tab_active, .5)`, an ad-hoc color. | `TabPill` in the kit using tokens: h 28, `R::MD`, text 13 (`TEXT_DENSE`), badge 16 with text 11, close `IconButton` xs (20) with `Glyph::X` 14. Hover uses the `hover` token. |
| A12 | ui.rs:171 | Change % uses the unicode ▲/▼ while everything else is a glyph. | Keep the triangles (TradingView uses them), but render them at 11 px through `Glyph::TriangleUp/Down` so the baseline matches. |
| A13 | ui.rs:200 | `.text_color(cx.theme().foreground)` (gpui-component palette) on an element whose children use tt-theme colors. | All chrome reads the global `UiTheme` (§3.1). |
| A14 | app.rs:961-1028 | Status bar: text 12.5, `gap_4`, no separators between groups, and the "Connecting" amber is hard-coded (`Color32::from_rgb(0xf5,0xb5,0x2e)`, line 971). | Text `TEXT_SMALL` (12), `Divider::vertical` between groups, and `warning` token for connecting. |
| A15 | app.rs:587-634 | Perf overlay: radius 8, `font_family("Consolas")`, magic `top 36 / right 90`, and its own panel styling. | `Overlay` surface (§2.5: `overlay_bg`, `R::LG`, `shadow_md`) with the mono font from the component theme (`mono_font_family`). Anchor it with tokens. |
| A16 | app.rs:487-490, 505-508, 557-562, 670-674, 759-761 | Dialog widths are 680, 780, 460, 780 and 520. Titles are mixed: "Symbol Search" (title case), "Add custom interval" (sentence), "Settings", and the bare "MA" (first word of the legend). | `DialogFrame` sizes `Sm 480 / Md 680 / Lg 800`. Titles in sentence case: "Symbol search", "Indicators, metrics and strategies", "Chart settings", "Add custom interval", and indicator settings titled with the full study name ("Moving Average Simple"). |
| A17 | app.rs:1043-1087 | Type-to-search, "/" and Alt shortcuts are handled in `on_key_down` with string matching. Drawing shortcuts are not real `Action`s, so tooltips cannot show bound keys. | Define actions (`PickTool(TrendLine)`, `OpenIndicators`, ...) and bind them in `bind_keys`. The kit's `tip_action()` then shows the real binding through `Kbd`. |

### 1.2 Toolbar dropdowns: `crates/tradeterm/src/menus.rs`

| # | Location | What is wrong | Fix |
|---|---|---|---|
| M1 | menus.rs:108 | `GOLD` is hard-coded (duplicated at indicators.rs:182). | `favorite` token. |
| M2 | menus.rs:113-172 | Custom row: h 32, radius 5, text 13.5, icon 17, star box 22 holding an SVG `IconName::Star/StarFill` at 16. The **selected row is inverted** (`bg = text, fg = bg`, lines 123-127), which no other list does and which is a black row in light theme. | `MenuRow` (§3.4): h 32, `R::MD`, text 14, icon 16. The current item shows `selection` background plus a leading check column (as in PopupMenu). The star is a trailing `RowAction::Favorite`. |
| M3 | menus.rs:180-188 | The interval trigger is an SVG `IconName::ChevronDown` icon button with tooltip "Intervals". | `ToolbarButton::caret()` (`Glyph::ChevronDown` 14) attached to the favorite-interval group. |
| M4 | menus.rs:211-235 | The collapsible section header is h 30, text 11.5, with SVG chevrons at 14. The header in the drawing flyout is text 11 with padding only (drawing_toolbar.rs:390-397). In chart settings it is text 11 with pt 16 (chart_settings.rs:289-296), and in indicators text 11 with px 20 (indicators.rs:255-263). | One `SectionHeader` (h 28, 11 px, uppercase, `text_muted`, optional collapse chevron). |
| M5 | menus.rs:244-259 | The remove-custom-interval "×" is an SVG `IconName::Close` at 14 that only changes text color on hover (no background), and has no tooltip. | `RowAction::Remove` → `IconButton::xs(Glyph::X).tooltip("Remove interval")`. |
| M6 | menus.rs:283-304 | "Add custom interval…" sits at the bottom after a manual divider. TradingView (and the earlier build, `tf_menu.png`) has it at the top. | First row of the menu, with `Glyph::Plus`, then a `Divider`. |
| M7 | menus.rs:202-206, 333 | Menu widths are 230 (intervals) and 240 (chart types), against 240 (settings menu), 250 (flyout) and 260 (price-scale menu). | `Menu::width(MenuWidth::Md = 240)`. Use `Lg = 280` only when a shortcut column is present. |
| M8 | menus.rs:315-323 | The chart-type trigger is `Button::child(lucide 17).dropdown_caret(true)`, which puts gpui-component's caret (SVG) next to a font glyph. | `ToolbarButton::icon(chart_type_glyph(cur)).caret()`. |
| M9 | menus.rs:361-363 | Row description tooltips use raw `.tooltip(Tooltip::new(desc).build)` (unmanaged and slow to appear). | Use the kit's `tip()` helper so the delay matches buttons. For rows, prefer TradingView's approach: show the description as a second muted line on hover, or skip it. |
| M10 | menus.rs:440-488 | Custom interval: the unit picker is six `Button`s toggled between `primary()` and `outline()` (a pseudo-segmented control). The error text uses `gpui_kit::red()` at 12.5. Cancel/Add are built inline in the body. | `ToggleGroup::segmented()` from gpui-component (`button/toggle.rs`), `danger` token at `TEXT_SMALL`, and `DialogFooter::new().cancel().ok("Add")`. |
| M11 | menus.rs:174-177, drawing_toolbar.rs:361-363 | `dismiss()` is duplicated, and every row handler re-clones `(app, pop)`. | `Menu` closes itself on item click (`MenuItem::keep_open()` opts out, for stars and collapse toggles). |
| M12 | menus.rs (all custom popovers) | No keyboard navigation (↑/↓/Enter/Esc only closes), no focus ring, and no type-ahead. PopupMenu has all of these. | `MenuState` in the kit gives arrow navigation, Enter, Esc, Home/End and type-ahead for every menu. |

### 1.3 Left drawing toolbar: `crates/tradeterm/src/drawing_toolbar.rs`

| # | Location | What is wrong | Fix |
|---|---|---|---|
| D1 | :24 | `WIDTH = 46`. TradingView's drawing toolbar is about 52 px wide. | `T::W_DRAWING_TOOLBAR = 48` with 36 px buttons (6 px gutters). |
| D2 | :26 | `ACTIVE = #2962ff` is hard-coded and is not in the theme. The theme `accent` is neutral gray, so "accent" means two different things. | Add the `active` token (blue) and `active_bg`. Keep `accent` for focus rings until it is renamed (§2.5). |
| D3 | :163-178 | `tool_button` is a hand-rolled 34 px div with radius 6, icon 19, hover = `selection`, and a raw tooltip. The active state uses the same `selection` background as hover, so hovering an inactive tool looks "selected". | `IconButton::new(id, glyph).size(Lg).toggled(active)`: 36 px, icon 20, hover = `hover`, toggled = `active` fg on `selection` bg. |
| D4 | :179-182 | Tooltip text is "Trend line (Alt + T)". Elsewhere it is "New tab (Ctrl+T)". | Use `tip(label, shortcut)`, which renders the label plus a `Kbd` chip (`Alt+T`) with one format everywhere. |
| D5 | :204-214, :376-383 | The flyout chevron is a gpui-component `Button::xsmall()` (20 px) containing a 10 px glyph, absolutely placed at `right(-6) top(9)` so it overlaps the tool button and the toolbar border. It renders as a misplaced "›" (`draw_base.png`). | The kit's `IconButton::with_flyout()` draws a 4 px corner caret inside the button's bottom-right corner (TradingView style). The 10 px hit strip on the right half opens the menu, and a long-press or right-click on the button also opens it. |
| D6 | :375-377 | The flyout `Popover` uses `Anchor::LeftCenter`. It centers vertically on the trigger, which pushes the menu up over the **tab strip and top toolbar** (`draw_flyout.png`: the menu top sits at y≈12, over the tabs). | Anchor `TopLeft` at the trigger's right edge with `offset(T::S1)`, so the menu top aligns with the button top. Clamp inside the chart area (window bounds minus title bar). See the z-order rules in §2.7. |
| D7 | :390-424 | Flyout rows: h 32, radius 5, icon 17, shortcut text 11 in muted color with no right alignment column. The selected row uses `selection` here but is inverted in menus.rs. | `MenuRow::new(label).icon(glyph).shortcut(k).checked(current)`. |
| D8 | :288-323 | Remove-all uses a gpui-component `Button::ghost()` at *medium* size (h 32, px 10, so 39 px wide). That is a different size and hover color from its 34 px siblings. The confirmation is a bare text row with no destructive styling. | `IconButton` Lg like the others. Confirmation is a `Menu` with a `MenuRow::danger("Remove 3 drawings")`, plus "Remove drawings and indicators" later (as TradingView does). |
| D9 | :325 | Separator is `w 26`. | `Divider::horizontal().inset(T::S2)`. |
| D10 | whole file | No keyboard focus and no tab stops for tools (TradingView doesn't have them either, but tooltips must show the Alt shortcut). | Tooltips through actions (A17). |

### 1.4 Symbol search: `crates/tradeterm/src/search.rs`

| # | Location | What is wrong | Fix |
|---|---|---|---|
| S1 | :26, app.rs:465 | The `Theme` is cloned at open time, so the dialog does not follow a theme switch. The same applies to indicators.rs:67, chart_settings.rs:156 and indicators.rs:354. | Read `cx.global::<UiTheme>()` at render (§3.1). |
| S2 | :282, indicators.rs:181 | Match highlight uses `series_palette[0]`, so a data color is reused as a UI accent. | `active` token. |
| S3 | :287-313 | Quote chips: px 12, py 4, rounded_full, text 12.5, selected **inverted**, hover `opacity(0.85)`. They are the only pill-shaped and opacity-hover controls. | `Chip` = `ToggleGroup` item: h 24, `R::FULL` (chips are the documented exception for full radius), text 12, selected = `selection` bg + `text`, unselected = `text_muted`, hover = `hover`. |
| S4 | :337-344 | The "Recent" header is 12 px and not uppercase, while every other list header is 11 px uppercase. | `SectionHeader("RECENT")`. |
| S5 | :356-409 | Result row: h 48, radius 6, badge 30 with text 13, symbol 14.5, meta 12, SPOT tag 9.5 on a `border` background, exchange 12.5. That is five sizes in one row. | `ListRow::tall()` (48): badge 28 with text 12, primary 14, secondary 12, tag `Tag::new("SPOT")` (11 px, `R::SM`, `panel_alt`), trailing exchange 12 muted. |
| S6 | :317 | List height is fixed at 420, the indicators body at 480, and chart settings at 470. | `DialogFrame` sets body height per size (§2.6). |
| S7 | :412-421 | Up/Down work, but there is no PageUp/PageDown or Home/End. Indicators (indicators.rs:267-282) lacks scroll-into-view entirely, so the selection can move off screen. | `ListNav` helper in the kit (shared by search, indicators and menus) with `ScrollHandle::scroll_to_item`. |

### 1.5 Indicator dialogs: `crates/tradeterm/src/indicators.rs`

| # | Location | What is wrong | Fix |
|---|---|---|---|
| I1 | :148-174 | Sidebar items: h 36, mx 8, px 12, radius 6, SVG `IconName` at 16. Chart settings' sidebar is h 38, px 12, radius 6, Lucide glyph 16 (chart_settings.rs:311-331). | `NavTabs::vertical()` (h 36, glyph 16), one look for both dialogs. |
| I2 | :194-253 | List rows are h 36. Menus use 32 and search uses 48. The star is on the left here and on the right in menus. | `ListRow` (36). The star is always trailing, like menus (TradingView puts the star on the left in the indicators list; we pick one place for the whole app: trailing). |
| I3 | :255-263 | Heading `px_5 pt_3 pb_1 11px` is a fourth variant of the section header. | `SectionHeader`. |
| I4 | :647-669 | The Inputs/Style tabs are a hand-rolled underline (`border_b_2`, `pb_2`) instead of gpui-component's `TabBar::underline()`. | `NavTabs::horizontal()` (wraps `TabBar::underline().small()`). |
| I5 | :687-694 | Form row: h 36, label w 150, label `text_muted`. Chart settings uses h 38, label w 200, label in normal text color. | `FormRow` (§3.7): h 36, label column from the dialog size (Sm 160, Lg 220), label color `text`. |
| I6 | :742-758, 769-794 | Checkboxes are added straight to the body, outside the label/control grid, so they misalign with the rows above. "Visible" sits at the end of the Style page (TradingView has a separate "Visibility" tab). | `FormRow::check(…)`. Visibility moves to its own tab or a top-of-Style row. |
| I7 | :703 vs chart_settings.rs:548 | `NumberInput` width is 140 here and 130 there. | `NumberField` with width `T::W_FIELD_NUMBER = 120`. |
| I8 | :711-735 | The source dropdown is `Button::outline().small()` + PopupMenu, and the chart settings dropdowns are built the same way but separately. | `Select<Source>` (§3.8). |
| I9 | :808-844 | Footer: "Defaults ▾" on the left, Cancel/Ok on the right. Chart settings has a plain "Reset to defaults" button instead. | `DialogFooter::new().template(TemplateMenu::defaults()).cancel().ok()` in both dialogs. |
| I10 | :576-584 | The title is the first word of the legend ("MA"; see `ind_gear3.png`). | The study's full name. |
| I11 | :684 | "This indicator was removed." is rendered as bare text in the dialog body. | `EmptyState` (muted 14, centered) and disable Ok. |

### 1.6 Chart settings: `crates/tradeterm/src/chart_settings.rs`

| # | Location | What is wrong | Fix |
|---|---|---|---|
| C1 | :307-308 | **Bug.** `let t = self.draft.theme(&self.base)` is used for dialog chrome (muted section headers, sidebar selection, footer border). That is the *chart's* theme with user overrides, so ScaleText overrides `text_muted` and ScaleLines overrides `border` *in the dialog*. | The chrome reads `UiTheme`. Only color *swatches* show draft colors. |
| C2 | :298-303 | Row label column is 200 px. "Color bars based on previous close" wraps onto two lines (`settings_dlg.png`). | `FormRow` label width `Lg = 220`, single line, ellipsis plus a tooltip on overflow. |
| C3 | :262-269 | `ColorPicker::small()` swatches are about 20 px. The floating drawing toolbar has its own 14 px swatch and 22 px palette grid (chart.rs:616-654). | `ColorField` (§3.9). It wraps `ColorPicker` with `featured_colors(PALETTE)` everywhere, so the swatch is 16 inside a 24 trigger. |
| C4 | :152 | `FONT_SIZES` includes 12.5. | Offer integers only: 10, 11, 12, 13, 14, 16, 18. 12.5 is no longer a token. |
| C5 | :420-443, 519-541 | Dropdowns are `Button::outline().small()` + PopupMenu (24 px tall), while NumberInput small is about 24 and ColorPicker small about 20. | Every form control is 28 px tall (`T::H_FIELD`): `Select`, `NumberField`, `ColorField`. |
| C6 | :556-600 | Footer: `pt_3 border_t` inside the body, buttons at the default 32 px. The body is a fixed `h 470` with a `gap_5` sidebar. | `DialogFrame::lg()` owns the paddings, body height, footer separator and button sizes. |
| C7 | :580-585 | "Reset to defaults" is an immediate action with no confirmation, and it differs from the indicator dialog. | `TemplateMenu` → "Apply defaults" (TradingView: Template ▾ → Apply defaults / Save as… ). |
| C8 | n/a | Enter does not trigger Ok, and the dialog has no keyboard path between sections. | `DialogFrame` binds Enter→Ok and Ctrl+Tab between nav tabs. |

### 1.7 In-chart controls: `crates/tt-chart-gpui/src/chart.rs`

| # | Location | What is wrong | Fix |
|---|---|---|---|
| G1 | :586-592 | `glyph()` duplicates `tradeterm::ui::lucide` (the chart crate cannot see the app's helpers). | Both use `tt_ui::Glyph`. tt-chart-gpui depends on tt-ui. |
| G2 | :613, 1042-1043 | The floating toolbar and pane buttons use `state.theme`. The corner controls use `view.style.theme(&state.theme)` (:787). The same overlay therefore gets different colors under user overrides. | Chrome colors come from `UiTheme`. The chart-style theme is only for canvas content. |
| G3 | :616-654 | Color popover: a hand-made 6×26 px grid, swatch hover `opacity(0.8)`, no "custom color", no opacity control. | `ColorField::compact()` (same popover as the dialogs). |
| G4 | :625-716 | Floating toolbar buttons are `Button::xsmall()` (20 px) with mixed children: swatch, "2px" text, "Solid" text, 14 px glyphs. Text dropdowns have no caret. | `FloatingToolbar` kit frame: `IconButton::sm` (24) and `ToolbarButton::text().caret()` (24), icons 16, text 12. Line style shows a line-style glyph instead of the word "Solid", as TradingView does. |
| G5 | :720-735 | Position is `right(axis_w + 70) top(pane.top + 6)`, a magic offset that depends on the pane-button width (up to about 122 px with 5 buttons). In `draw_trend.png`/`draw_fib.png` it covers the OHLC readout ("+276.00 (+0.43…" is clipped). In `draw_measure.png` its top (y 93) is 5 px off the pane buttons (y 88). | Default position: top-center of the price pane at `pane.top + T::S2`. The user can drag it (handle on the left, as in TradingView), and the offset is remembered per session. Collision rule from §2.7: it is never placed in the pane-controls gutter (`PaneControls::width(n)` is exported by the kit). |
| G6 | :731-736 | Floating surface uses radius 6, `bg = theme.bg`, `shadow_md` and text 12. It is the only floating surface without a hover/drag affordance. | `Overlay` surface token (`overlay_bg`, `R::LG`, `border`, `shadow_md`). |
| G7 | :799-805, 1044-1050 | Fades use a literal `Transition::new(Duration::from_millis(90))` with no easing, while ui.rs uses `snappy()` EaseOut. | `motion::fade_fast()` (90 ms EaseOut). |
| G8 | :807-829 | A/L/% are hand-rolled 20 px divs with text 11.5, active state **inverted**, hover `opacity(0.85)` and raw tooltips. | `IconButton::text("A").size(Xs).toggled(auto)`: toggled = `active` fg on `selection` bg (TradingView shows the active letter in blue). Text 12. |
| G9 | :831-843 | The A/L/% strip has an opaque `theme.bg` background that hides price labels behind it, with no separator. | Keep it opaque (labels must not show through), but give it the `panel` token and a top `border` hairline so it reads as chrome. |
| G10 | :895-910 | The gear is `Button::xsmall().icon(IconName::Settings)`, an SVG icon inside the chart where every other chart icon is a Lucide glyph. | `IconButton::new("scale-settings", Glyph::Settings).size(Sm)`. |
| G11 | :910-1001 | The price-scale menu is a PopupMenu (26 px rows) and opens upward over the price axis (`corner_menu.png`). The "Labels and lines" section label is sentence case, while the other headers are uppercase. Only "More settings…" has an icon, so the whole menu reserves an icon column. | `Menu` with `SectionHeader("LABELS AND LINES")`, and no icon on "More settings…" (or icons on every row). The anchor is unchanged (BottomRight is correct). |
| G12 | :1029-1051 | Pane controls fade in, but `pane?` returns `None` as soon as `shown` is `None`, so the fade-out is cut and the row vanishes. | Keep the last pane in `Chart` state (`last_pane_controls`) and render until opacity reaches 0, as the A/L/% strip does (`.invisible()` below 0.01). |
| G13 | :1057-1078 | Pane buttons are a third hand-rolled button (22 px, radius 4, glyph 15, raw tooltip). Their tooltips go stale after a click moves them (`pane_collapsed.png`: "Collapse pane" floats 130 px from its button; `pane_max2.png` likewise). | `IconButton::new(..).size(Sm)` (24, icon 16) on gpui-component `Button`, which has a managed tooltip that hides when the trigger moves or is clicked. |
| G14 | :1079-1089 | The pane-controls row background is `with_alpha(bg, 0.9)` with radius 4. The floating toolbar uses an opaque bg with radius 6 and a border. | Both use the `Overlay` surface. Pane buttons can stay borderless (TradingView), but share radius `R::MD`. |

### 1.8 Canvas legend and tags: `crates/tt-chart/src/draw.rs`

| # | Location | What is wrong | Fix |
|---|---|---|---|
| L1 | :924-925 | `LEGEND_ICON 15 / LEGEND_BUTTON 20`. The gpui overlay buttons are 22 (pane) and 20 (A/L/%), and the kit will use 24 for in-chart buttons. | Read `tt_theme::metrics::{ICON_SM=16, CONTROL_XS=20}`. In-canvas legend buttons stay at 20 (rows are `font_size + 10`), but the icon becomes 14 (`ICON_XS`) for the same 3 px inset ratio as gpui buttons. |
| L2 | :966-967, 1024-1027 | `fill_rounded_rect(r, 4|3)` followed by `stroke_rect` (square corners), so the border corners poke out of the rounded fill. | Add `Canvas::stroke_rounded_rect`, or drop the stroke and rely on `overlay_bg`. Use radius `R::SM = 4` for both. |
| L3 | :1044-1052 | Legend eye/gear/trash have no per-button hover (only the row is highlighted) and no tooltips (canvas-drawn). | Pass `legend_hover_button: Option<(u64, LegendButton)>` in `RenderInput` and fill the hovered button with `hover`. Tooltips: the chart view shows a kit tooltip positioned on the button rect, from `LegendHit` (overlay layer, so this is cheap). |
| L4 | `pane_hover2.png` | Pane level lines (RSI 70/30 dashed) are drawn through the legend text ("33.99" is struck through) because the legend has no backdrop unless hovered. | Draw a `with_alpha(bg, 0.85)` backdrop behind every legend row (one rounded rect per row; cheap). |
| L5 | :1166 | Axis tags use radius 2. That is fine, but it should be named. | `metrics::R_XS = 2` (tags only). |

### 1.9 Theme and global: `crates/tt-theme/src/lib.rs`, `ui.rs:32-50`

| # | Location | What is wrong | Fix |
|---|---|---|---|
| T1 | lib.rs:277-278 | `accent` is neutral gray (`#d4d4d4` dark, `#171717` light) and is documented as "focus rings and emphasis". The UI needs a real blue active color, which is hard-coded instead. | Add `active` and `active_bg` (§2.5). |
| T2 | lib.rs:287-289 | `selection` is used for hover *and* selected *and* text selection, so hover cannot be told apart from current. | Add `hover` (one step lighter than `selection` in dark, darker in light). |
| T3 | lib.rs:279-280 | `text_on_accent` (white) is used on the series-palette badges, including yellow `#fdd835`, which gives low contrast. | `badge_fg(color)` picks black or white by luminance. |
| T4 | ui.rs:32-50 | `apply_theme` switches gpui-component's mode but never maps tt tokens into gpui-component's `ThemeColor`, so there are two palettes. | `tt_ui::theme::sync(&Theme, window, cx)` writes `background, foreground, border, popover, accent (=hover), secondary_active (=selection), ring (=active), primary, list.*, tab.*` (§2.5). |
| T5 | n/a | No `favorite`, `warning`, `danger`, `overlay_bg`, `text_disabled` or `focus_ring` tokens, so they get hard-coded. | Added in §2.5. |
| T6 | light theme | Inverted selections (M2, S3, G8) render as solid black bars. The A/L/% "A" becomes a black square. The chart-settings OK button is black, which is fine. | Removing the inverted pattern (§2.8) fixes this. Check the light theme in slice 9 screenshots. |

---

## 2. Design tokens

Tokens live in two places:

- **`tt_theme::metrics`** (new module, no gpui dependency): numeric sizes shared by the canvas (`tt-chart`) and the gpui kit.
- **`tt_theme::Theme`**: colors (extended below).

`tt_ui::tokens` re-exports `metrics` as gpui `Pixels` helpers (`T::H_TOOLBAR.px()`) and adds motion durations.

gpui-component's rem stays at 16 px (`Theme::font_size`, which `Root` uses as `window.set_rem_size`). Changing it would rescale every component's geometry to fractional pixels. We set explicit px sizes in the kit and keep the component theme's `radius = 6` and `radius_lg = 10`.

### 2.1 Spacing (4 px grid, with a 2 px half step)

| Token | px | gpui helper | Use |
|---|---|---|---|
| `S0_5` | 2 | `gap_0p5` | icon-to-caret, gaps inside a tight cluster |
| `S1` | 4 | `gap_1` | between toolbar buttons, menu edge padding |
| `S1_5` | 6 | `gap_1p5` | icon to label |
| `S2` | 8 | `gap_2` | row horizontal padding, toolbar padding |
| `S3` | 12 | `gap_3` | between form controls in a row |
| `S4` | 16 | `gap_4` | between groups (status bar), section top margin |
| `S5` | 20 | `p_5` | dialog horizontal padding |
| `S6` | 24 | `p_6` | empty-state padding |

Banned: `gap_5` and other odd values used ad hoc (chart_settings.rs:561), plus `mx_2 + px_3` combinations. Rows always use `px(S2)`, and lists inset by `S1`.

### 2.2 Radii

| Token | px | Use |
|---|---|---|
| `R_XS` | 2 | canvas axis tags only |
| `R_SM` | 4 | swatches, `Kbd`, tags, legend buttons, xs (20 px) controls |
| `R_MD` | 6 | buttons, rows, inputs, tab pills, nav items (= component `radius`) |
| `R_LG` | 8 | menus/popovers, floating toolbars, overlays |
| `R_XL` | 10 | dialogs (= component `radius_lg`) |
| `R_FULL` | ∞ | only chips, status dot, avatar badges |

### 2.3 Typography

Font is `Segoe UI` (Windows) or `.SystemUIFont`. Mono is the component theme's `mono_font_family` (Consolas on Windows). Do not name fonts in feature code.

| Token | px | Weight | Use |
|---|---|---|---|
| `TEXT_CAPTION` | 11 | 500, uppercase | section headers, `Kbd`, tags, badge initials, shortcut hints in menus |
| `TEXT_SMALL` | 12 | 400 | status bar, secondary lines, floating-toolbar labels, A/L/%, error text |
| `TEXT_DENSE` | 13 | 400 | tab strip, dense lists (search result meta is SMALL) |
| `TEXT_BODY` | 14 | 400 | default: toolbar labels, menus, form labels/values, buttons (= component `text_sm`) |
| `TEXT_TITLE` | 16 | 600 | dialog titles |

Chart canvas text stays user-configurable (`ChartStyle::font_size`, integers only after C4).

### 2.4 Icon sizes and control heights

Icons: Lucide (one set, see §3.2).

| Token | px | Use |
|---|---|---|
| `ICON_XS` | 14 | inside 20 px controls, carets, legend canvas buttons, tab close |
| `ICON_SM` | 16 | menus, lists, nav tabs, in-chart 24 px buttons, input prefix |
| `ICON_MD` | 18 | top toolbar (32 px buttons), title bar buttons |
| `ICON_LG` | 20 | left drawing toolbar (36 px buttons) |

Rule: control size = icon + 2 × inset, with inset 3 (xs), 4 (sm), 7 (md) or 8 (lg).

| Token | px | Use |
|---|---|---|
| `H_TITLE_BAR` | 34 | gpui-component `TITLE_BAR_HEIGHT` |
| `H_TAB` | 28 | chart tab pills |
| `H_TOOLBAR` | 40 | top toolbar (TradingView ≈ 38–40) |
| `H_STATUS_BAR` | 28 | status bar |
| `CONTROL_XS` | 20 | A/L/%, swatches in rows, row actions (star, ×), canvas legend buttons |
| `CONTROL_SM` | 24 | in-chart overlay buttons (pane, gear, floating toolbar), chips, title bar buttons |
| `H_FIELD` | 28 | inputs, selects, color fields inside forms |
| `CONTROL_MD` | 32 | top toolbar buttons, dialog footer buttons |
| `CONTROL_LG` | 36 | drawing toolbar buttons |
| `H_MENU_ROW` | 32 | every menu row (custom and former PopupMenu) |
| `H_SECTION` | 28 | `SectionHeader` |
| `H_LIST_ROW` | 36 | nav tabs, indicator list, form rows |
| `H_LIST_ROW_TALL` | 48 | symbol search results |

| Width token | px | Use |
|---|---|---|
| `W_DRAWING_TOOLBAR` | 48 | left toolbar (TradingView ≈ 52) |
| `W_MENU_SM / MD / LG` | 200 / 240 / 280 | menus (LG only with a shortcut column) |
| `W_DIALOG_SM / MD / LG` | 480 / 680 / 800 | dialogs |
| `W_NAV` | 200 | dialog left nav |
| `W_LABEL_SM / LG` | 160 / 220 | form label column in Sm/Lg dialogs |
| `W_FIELD_NUMBER` | 120 | number fields |
| `W_FIELD_SELECT` | 160 | selects |

### 2.5 Colors (tt-theme tokens)

Existing tokens keep their meaning: `chrome, tab_active, bg, panel, panel_alt, border, grid, text, text_muted, up, down, …, series_palette`. New or changed tokens (hex values: dark / light):

| Token | Dark | Light | Meaning |
|---|---|---|---|
| `hover` | `#232323` | `#f0f0f0` | hover background of any control or row |
| `selection` (kept) | `#2e2e2e` | `#e5e5e5` | current/selected item background, toggled-button background, text selection |
| `active` (new) | `#2962ff` | `#2962ff` | "on" state: active drawing tool icon, A/L/% when on, search match highlight, focus ring, links |
| `active_bg` (new) | `#2962ff` @ 18% | `#2962ff` @ 12% | tinted background where blue fill is wanted (chips that mean a filter is on, optional) |
| `text_disabled` | `#5c5c5c` | `#a3a3a3` | disabled labels |
| `favorite` | `#f5b52e` | `#f5a623` | stars |
| `warning` | `#f5b52e` | `#d48806` | connecting status, warnings |
| `danger` | `#f23645` | `#e0293a` | destructive menu rows, validation errors (separate from `down` so users can recolor candles) |
| `overlay_bg` | `#141414` | `#ffffff` | floating toolbars, perf overlay, in-chart menus (the `panel_alt` family) |
| `focus_ring` | `active` @ 60% | `active` @ 50% | keyboard focus outline |
| `accent` | unchanged, **deprecated** | | kept for chart crosshair label use. UI code must use `active`/`selection`. |

Mapping into gpui-component (`tt_ui::theme::sync`, called from `apply_theme` and on every `set_chart_style`):

| gpui-component `ThemeColor` | ← tt token |
|---|---|
| `background` | `panel` |
| `foreground` | `text` |
| `muted_foreground` | `text_muted` |
| `border`, `input` | `border` |
| `popover`, `popover_foreground` | `overlay_bg`, `text` |
| `accent`, `accent_foreground` (ghost hover) | `hover`, `text` |
| `secondary_active`, `list_active` (selected) | `selection` |
| `primary`, `primary_foreground` | `text`, `bg` (TradingView-style neutral Ok button) |
| `ring` | `focus_ring` |
| `danger` | `danger` |
| `tab_active`, `tab_bar` | `tab_active`, `chrome` |
| `title_bar` | `chrome` |

### 2.6 Surfaces and elevation

| Surface | bg | border | radius | shadow | padding |
|---|---|---|---|---|---|
| Title bar / tab strip | `chrome` | none | 0 | none | `S2` |
| Toolbars, status bar | `panel` | 1 px `border` hairline on the chart side | 0 | none | `S2` |
| Menu / popover | `overlay_bg` | 1 px `border` | `R_LG` | `shadow_md` | `S1` |
| Floating toolbar, perf overlay | `overlay_bg` | 1 px `border` | `R_LG` | `shadow_md` | `S0_5` / `S3` |
| Dialog | `panel` | 1 px `border` | `R_XL` | `shadow_lg` | header `S5`×`S4`, body `S5`, footer `S5`×`S3` |
| Tooltip | gpui-component tooltip (themed via `popover`) | | `R_SM` | | |

Dialog body heights (fixed, so switching tabs does not jump): Sm 280, Md 460, Lg 480. Header 52, footer 60.

### 2.7 Z-order and placement rules

From bottom to top:

1. Canvas base and overlay layers.
2. Canvas legend (inside the overlay layer).
3. In-chart chrome: pane controls, A/L/% strip, gear. They live in the chart's top-right *pane-controls gutter*, whose width is `PaneControls::width(n)`.
4. Floating drawing toolbar. It never enters the gutter and never overlaps the OHLC readout row. It defaults to top-center and can be dragged.
5. Popovers and menus (deferred, window-level).
6. Tooltips.
7. Dialogs and their overlay.

Placement rules:

- Menus from the left drawing toolbar open to the right, top-aligned (`Anchor::TopLeft`, offset `S1`), and clamp below `H_TITLE_BAR + H_TOOLBAR`.
- Toolbar menus open below (`Anchor::TopLeft`, offset `S1`). In-chart corner menus open upward (`BottomRight`).
- A menu never covers its own trigger.
- Tooltips appear below for top-bar controls, to the right for left-toolbar controls, and above for in-chart bottom controls.

### 2.8 State model (applies to every interactive element)

| State | Visual |
|---|---|
| Rest | transparent bg, `text` (icons) or `text_muted` (secondary) |
| Hover | `hover` bg, **instant** (0 ms, no fade) |
| Pressed | `selection` bg |
| Current / selected (one of many: interval, menu item, nav tab) | `selection` bg. Menus add a leading ✓ instead of inverting. |
| Toggled on (binary tool/setting: drawing tool, magnet, A/L/%) | `active` fg + `selection` bg |
| Focused (keyboard) | 2 px `focus_ring` outline (component default `focus_ring: true`) |
| Disabled | `text_disabled`, no hover |
| Destructive | `danger` fg. Hover bg `danger` @ 12%. |

**Inverted fills (bg = text) are banned.** Primary buttons are the one filled control.

### 2.9 Motion

All motion is UI chrome, never data. Everything is interruptible (`transition()` retargets from the current value) and honors reduce-motion (gpui-kit snaps).

| Token | ms | Easing | Use |
|---|---|---|---|
| `INSTANT` | 0 | n/a | hover, pressed, selection changes |
| `FAST` | 90 | EaseOut | in-chart control fades (A/L/%, pane buttons, both directions), caret rotation |
| `NORMAL` | 130 | EaseOut | menu/popover open, tab active fill (was 120) |
| `SLOW` | 200 | EaseOut | dialog entrance (`DialogEntrance::FadeSlide`) |
| `SPRING_CONTROL` | 140 | spring | toggles and switches (already set in `apply_theme`) |
| `TOOLTIP_DELAY` | 450 | n/a | first tooltip. 0 while warm (gpui-base hand-off). |

Banned: animating prices, candles, crosshair, axis labels, legend values, or the tab change %.

### 2.10 Dialog & floating toolbar density (TradingView-measured)

Measured 2026-11-21 on tradingview.com/chart (dark theme, CSS px, read with `getComputedStyle` / `getBoundingClientRect` on a temporary horizontal ray's settings dialog, line popover and floating toolbar; light colors sampled from `docs/specs/img/tv-ref/`). The drawing settings dialog and the floating drawing toolbar use this **comfortable** density; the app chrome (top toolbar, menus, lists, status bar) keeps the compact tokens of §2.1–2.4: TradingView's own top bar is 38 px with 38 px buttons, close to our 40 / 32, so it was left alone.

Tokens: `tt_theme::metrics::C_*` (dialog) and `FT_*` (floating toolbar).

**Dialog** (`DialogOpts::comfortable`, `DialogFrame::comfortable`, `DialogFooter::comfortable`)

| Part | TradingView | Token |
|---|---|---|
| Surface | radius 20, translucent bg + blur, shadow `0 0 2px`, `0 3px 9px` | `C_R_DIALOG` 20 |
| Width | 380 min; grows with the widest tab (Visibility 453) | `C_W_DIALOG` 380; the ray dialog is fixed at 454 |
| Height | fits the tab (Style 311, Text 413, Visibility 598); short tabs keep 2 rows | `DialogFrame::min_body(2 × C_ROW)` |
| Header | 68 tall, padding 0 20; title 20 px / 600 / line 28 at y 20 | `C_H_HEADER`, `C_TEXT_TITLE`, `C_TITLE_LINE` |
| Rename ✎ | 34 square right after the title (gap 5), 28 px icon; turns the title into an input | `C_HEADER_BUTTON`, `DialogOpts::rename` |
| Close × | 34 square, icon 18, radius 8 | `C_HEADER_BUTTON` |
| Tabs | 16 px / 600, gap 24, 8 px under the label, strip 32 tall | `C_TEXT_TAB`, `C_TAB_GAP`, `C_H_TABS` |
| Tab underline | 4 px, radius 2, `text` color; a faint full-width 4 px track under all tabs | `C_TAB_UNDERLINE` |
| Body padding | 16 top / bottom, 20 left / right | `C_PAD_Y`, `C_PAD_X` |
| Row | 50 tall (8 + 34 + 8) | `C_ROW` |
| Label column | label width + 20 (table cells); Visibility labels 96 | `FormRow::label_auto`, `C_LABEL_GAP` |
| Inputs / selects | 100 × 34, radius 8, 1 px border (`#303030` dark), text 14, chevron 18 | `C_W_FIELD`, `C_CONTROL`, `C_R_CONTROL` |
| Gap between controls | 8 | `C_GAP` |
| Checkbox | 18 box, radius 3, checked = `text` fill with a ✓ in `bg`; label 14 at +8 | `C_CHECKBOX`, `CheckBox` |
| Square toggles (B, I, color) | 34 × 34, radius 6, 1 px border (`#555` dark) | `C_R_TOGGLE`, `ToggleSquare` |
| Swatch inside | 24 × 24, radius 3, 4 px inset | `C_SWATCH` |
| Line control | ONE button 74.6 × 34: swatch + 30 px line preview (its real width / dash) | `LinePicker`, `W_LINE_PICKER` |
| Text area | full width × 100, radius 8 | `C_H_TEXTAREA` |
| Range slider | track 100 × 10, radius 5 (`#303030` / light `#f2f2f2`); selected part `#dbdbdb` / light `#464646`; handles 12 round, 2 px ring; 9 + 8 px to the fields | `C_W_SLIDER`, `C_SLIDER_TRACK`, `C_SLIDER_HANDLE` |
| Footer | 66 tall, padding 16 20; Template 100 × 34 outline; Cancel / Ok 34 tall, 16 px labels, padding 0 11, radius 8, gap 12; Ok = `text` fill | `C_H_FOOTER`, `C_TEXT_BUTTON` |

**Popover (line / color)**: 248 wide, radius 6, padding 6 / 12; palette 10 columns × 8 rows of 16.6 px swatches (radius 2) on a 22.6 pitch (`TV_PALETTE`); separator; "Opacity" 12 px muted, slider 169 × 10 with a transparent→color gradient + a 47 × 26 percent box; "Thickness" four 57 × 32 segments with 1–4 px bars; "Line style" three segments (solid / dashed 5-on-3-off / dotted 2-on-3-off). The selected segment is inverted (`text` fill).

**Floating drawing toolbar** (`Overlay::floating`, `ControlSize::Xl`, `ColorBarButton`, `LineWidthButton`, `LineStyleButton`)

| Part | TradingView | Token |
|---|---|---|
| Bar | 38 tall, radius 6, no padding, no gaps, no border (shadow `0 0 2px`, `0 3px 9px`) | `FT_HEIGHT`, `FT_RADIUS` |
| Grip | 24 wide, six dots | `FT_GRIP` |
| Buttons | 38 × 38 squares, icons drawn in a 28 box (~18–20 px glyphs), color `#b8b8b8` dark | `FT_BUTTON`, `FT_ICON` 20 |
| Color buttons | icon 16 at y 7 + color bar 22 × 4 (radius 2) at y 27: pencil = line, "T" = text, bucket = fill | `FT_BAR_ICON`, `FT_BAR_W`, `FT_BAR_H` |
| Line width | 68 wide: an 18 px line preview at x 10, then "1px" (14 px) | `FT_W_WIDTH_BUTTON` |
| Order | grip, template (grid +), line color, text color, width, style, settings, *alert*, lock, remove, … | alert is in BACKLOG.md |

**Text on lines**: TradingView centers the text on the line (horizontal line / ray default: 12 px, Center / Middle, 1 px line) and breaks the line behind it, about 6 px each side (`TEXT_LINE_GAP`; `lines::line_with_text` does any angle). Top / Bottom text sits above / below an unbroken line. The text shows whenever it is not empty (the comfortable Text tab has no "Show text").

**Colors** (dark → light): text `#b8b8b8` / `#131722`-ish (we keep our `text`); control border `#303030` / `#e0e3eb`; toggle border `#555`; primary (Ok, checkbox, tab underline) `#f2f2f2` on dark, `#222` on light (our `text`); muted labels `#8c8c8c`.

---

## 3. Shared component kit: `crates/tt-ui`

### 3.0 Why a crate

`tt-chart-gpui` needs the same IconButton, Menu, Overlay, tooltips and Glyph as the app (G1–G14), and it cannot depend on `tradeterm`. So the kit is a new crate:

```
crates/tt-ui/
  Cargo.toml        # gpui-kit =0.10.0, lucide-icons, ecolor, tt-theme
  src/lib.rs        # init(), re-exports, prelude
  src/tokens.rs     # T:: metrics as Pixels, motion constants
  src/theme.rs      # UiTheme global, sync() into gpui-component
  src/glyph.rs      # Glyph (Lucide font), GlyphSize
  src/tooltip.rs    # tip(), tip_action()
  src/button.rs     # IconButton, ToolbarButton, TabPill, Chip
  src/divider.rs    # Divider
  src/menu.rs       # Menu, MenuRow, MenuItem, SectionHeader, MenuState
  src/list.rs       # ListRow, ListNav, EmptyState, Tag
  src/overlay.rs    # Overlay surface, FloatingToolbar, PaneControls::width
  src/dialog.rs     # DialogFrame, DialogFooter, TemplateMenu, NavTabs
  src/form.rs       # FormRow, NumberField, Select, ColorField
  examples/gallery.rs  # every component in dark/light, for screenshot review
```

Dependency graph: `tt-theme` ← `tt-ui` ← `tt-chart-gpui` ← `tradeterm`. `tt-chart` (canvas) depends only on `tt-theme::metrics`.

### 3.1 Theme access

```rust
/// The current UI palette, readable from any render.
#[derive(Clone)]
pub struct UiTheme(pub tt_theme::Theme);
impl gpui::Global for UiTheme {}

pub trait ActiveUiTheme { fn ui(&self) -> &tt_theme::Theme; }
impl ActiveUiTheme for App { fn ui(&self) -> &tt_theme::Theme { &self.global::<UiTheme>().0 } }

/// Sets the global, switches gpui-component's mode, maps tokens (§2.5) into
/// its ThemeColor, sets radius 6 / radius_lg 10 and the motion durations.
pub fn sync(theme: &tt_theme::Theme, window: &mut Window, cx: &mut App);
```

Dialog panels drop their cloned `theme: Theme` fields (S1) and read `cx.ui()`.

### 3.2 Glyph: one icon set

Decision: **Lucide, rendered with the Lucide font** everywhere we draw our own icons.

- The canvas legend already needs the font, and the font gives the complete catalog with no asset registration. Color follows `text_color`, so hover and active tints need no extra state.
- gpui-component `IconName` is not used in our code. Its SVG icons remain only inside third-party internals: checkbox tick, dialog ×, input clear, PopupMenu check. Those are Lucide too.
- Documented exception: a filled star. The Lucide font has no filled variant, so `Glyph::StarFilled` renders `gpui_kit::component::IconName::StarFill` through `Icon`.

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Glyph { Lucide(lucide_icons::Icon), StarFilled }

impl Glyph {
    pub const PLUS: Glyph = Glyph::Lucide(lucide_icons::Icon::Plus);
    // … consts for every icon the app uses (Search, Settings, Sun, Moon, X,
    // ChevronDown/Up/Right, Star, Eye/EyeOff, Lock/LockOpen, Trash2, Magnet, …)
}

#[derive(IntoElement)]
pub struct GlyphEl { glyph: Glyph, size: f32, color: Option<Hsla> }

pub fn glyph(g: impl Into<Glyph>, size: f32) -> GlyphEl;   // square box, centered, line-height = size
impl GlyphEl { pub fn color(self, c: Hsla) -> Self; }

/// Chart-type and drawing-tool icons live here (moved from ui.rs / tt-chart).
pub fn chart_type_glyph(t: tt_chart::ChartType) -> Glyph;   // behind feature "chart"
```

`GlyphEl` renders a `size × size` flex box, centered, with `line_height = size`. This fixes the vertical drift of raw glyphs (D5).

### 3.3 Buttons

All built on gpui-component `Button`, so they get managed tooltips (delay, warm hand-off, hiding on move/click: G13), focus handling, `selected` and `toggled` accessibility, and `on_click`. Colors come from a `ButtonCustomVariant` built from tokens: color transparent, hover `hover`, active `selection`, foreground `text`.

```rust
#[derive(Clone, Copy)]
pub enum ControlSize { Xs /*20, icon 14*/, Sm /*24, 16*/, Md /*32, 18*/, Lg /*36, 20*/ }

#[derive(IntoElement)]
pub struct IconButton { /* id, glyph, size, toggled, selected, tooltip, flyout, danger, disabled, on_click */ }

impl IconButton {
    pub fn new(id: impl Into<ElementId>, glyph: impl Into<Glyph>) -> Self;
    /// Single-letter/short text instead of an icon (A / L / %).
    pub fn text(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self;
    pub fn size(self, s: ControlSize) -> Self;              // default Md
    pub fn toggled(self, on: bool) -> Self;                  // active fg + selection bg, aria-pressed
    pub fn selected(self, on: bool) -> Self;                 // selection bg
    pub fn danger(self) -> Self;
    pub fn disabled(self, d: bool) -> Self;
    pub fn tooltip(self, label: impl Into<SharedString>) -> Self;
    /// Tooltip with the action's bound key as a Kbd chip ("Trend line  Alt+T").
    pub fn tooltip_action(self, label: impl Into<SharedString>, action: &dyn Action) -> Self;
    /// Corner caret + right-edge hit strip that opens `menu` (drawing toolbar groups).
    pub fn with_flyout(self, menu: impl Fn(Menu, &mut Window, &mut App) -> Menu + 'static) -> Self;
    pub fn on_click(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self;
}

#[derive(IntoElement)]
pub struct ToolbarButton { /* label, leading glyph, caret, selected, field style */ }

impl ToolbarButton {
    pub fn text(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self; // h 32, px 8, text 14
    pub fn icon(id: impl Into<ElementId>, glyph: impl Into<Glyph>) -> Self;        // square 32
    pub fn field(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self; // ticker pill (panel_alt bg)
    pub fn leading(self, g: impl Into<Glyph>) -> Self;       // icon 18, gap 6
    pub fn caret(self) -> Self;                              // ChevronDown 14, rotates 180° (FAST) while open
    pub fn size(self, s: ControlSize) -> Self;               // Sm for the floating toolbar
    pub fn selected(self, on: bool) -> Self;
    pub fn tooltip(self, t: impl Into<SharedString>) -> Self;
    pub fn tooltip_action(self, t: impl Into<SharedString>, a: &dyn Action) -> Self;
    /// Opens a kit Menu anchored below (TopLeft, offset S1).
    pub fn menu(self, build: impl Fn(Menu, &mut Window, &mut App) -> Menu + 'static) -> Self;
    pub fn on_click(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self;
}

pub struct TabPill;   // tab strip pill: badge, label, change %, close IconButton::Xs
pub struct Chip;      // h 24, R_FULL, text 12, selected = selection bg
pub struct Divider;   // ::vertical() h 20 / ::horizontal(); .inset(px)
```

Verification note for slice 0: gpui-component `Button` treats any `.child()` as a text button (`button.rs:694-724`, height from `Size` and px padding). `IconButton` must therefore set `Size::Size(px(h))` and pass the glyph as its only child with an explicit `.w(h).h(h).px_0()` style refinement. Confirm that the instance style wins, and add a gallery test that measures bounds.

### 3.4 Menus

One menu system for all dropdowns, built on `gpui_kit::component::popover::Popover` (anchoring and dismissal), with kit-owned rows and keyboard handling. Reasons:

- Four menus need rich rows that `PopupMenu` cannot express cleanly: star, remove, collapsible sections and descriptions.
- One look requires one row implementation.
- PopupMenu's row height is rem-based (26 px) and cannot be set to 32 without changing the rem.

```rust
pub struct Menu { /* items, width, max_h, state */ }

impl Menu {
    pub fn new() -> Self;
    pub fn width(self, w: MenuWidth) -> Self;                 // Sm 200 / Md 240 / Lg 280
    pub fn max_h(self, h: Pixels) -> Self;                    // scrolls; keeps the current row in view
    pub fn row(self, row: MenuRow) -> Self;
    pub fn section(self, title: impl Into<SharedString>) -> Self;            // SectionHeader
    pub fn collapsible_section(self, title: impl Into<SharedString>, open: bool,
        on_toggle: impl Fn(&mut Window, &mut App) + 'static) -> Self;
    pub fn separator(self) -> Self;
    pub fn submenu(self, row: MenuRow, build: impl Fn(Menu) -> Menu + 'static) -> Self; // opens right, hover 0 ms
}

#[derive(IntoElement)]
pub struct MenuRow { /* id, glyph, label, description, shortcut, checked, actions, danger, disabled */ }

impl MenuRow {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self; // h 32, R_MD, text 14
    pub fn icon(self, g: impl Into<Glyph>) -> Self;            // 16, muted unless hovered
    pub fn checked(self, on: bool) -> Self;                     // leading ✓ column (16) when any row is checkable
    pub fn shortcut(self, keys: impl Into<SharedString>) -> Self;  // right-aligned Kbd text, 11 muted
    pub fn shortcut_action(self, a: &dyn Action) -> Self;
    pub fn description(self, d: impl Into<SharedString>) -> Self;  // tooltip after TOOLTIP_DELAY
    pub fn favorite(self, on: bool, f: impl Fn(&mut Window, &mut App) + 'static) -> Self; // trailing star, visible on hover or when on
    pub fn removable(self, tip: impl Into<SharedString>, f: impl Fn(&mut Window, &mut App) + 'static) -> Self;
    pub fn danger(self) -> Self;
    pub fn disabled(self, d: bool) -> Self;
    pub fn keep_open(self) -> Self;                             // default: click closes the menu
    pub fn on_select(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self;
}

#[derive(IntoElement)]
pub struct SectionHeader { /* title, collapsible, open */ }   // h 28, 11 px uppercase, text_muted, px 8

/// Keyboard and highlight state shared by Menu, ListNav users and Select.
pub struct MenuState { /* focus, highlighted, scroll */ }
// ↑/↓ move, Home/End, PageUp/PageDown, Enter selects, Esc closes, typing jumps to the first label match.
// Pointer hover sets the highlight; a key press under a still pointer keeps it (as gpui-component does).
```

The settings gear menu (A4), price-scale menu (G11), line width/style (G4), Source (I8), font size, scale-buttons option (C5), and the interval, chart-type and flyout menus all use `Menu`. `PopupMenu` is no longer used in our code. Context menus (right-click on chart, future) also use `Menu`.

### 3.5 Tooltips

```rust
/// Text tooltip for elements that are not kit buttons (rows, canvas hit rects).
pub fn tip(label: impl Into<SharedString>, keys: Option<&str>) -> impl Fn(&mut Window, &mut App) -> AnyView;
pub fn tip_action(label: impl Into<SharedString>, action: &dyn Action) -> impl Fn(&mut Window, &mut App) -> AnyView;
```

Format: label in sentence case, no trailing period, shortcut as a `Kbd` chip (`Ctrl+T`, `Alt+Shift+R`, `/`). Never put the shortcut in parentheses. Every icon-only control must have a tooltip (lint in review: `IconButton` without `.tooltip*` is a review blocker).

Note: gpui-component's `ManagedTooltipExt` is `pub(crate)`. Raw elements therefore get gpui's native tooltip through `tip()`. Prefer kit buttons for anything that moves after a click.

### 3.6 Dialog scaffold

```rust
pub enum DialogSize { Sm, Md, Lg }    // 480/680/800 wide; body 280/460/480 high

/// Opens a dialog with the standard frame. `panel` renders only the body.
pub fn open_dialog<P: Render + Focusable>(
    window: &mut Window, cx: &mut App,
    title: impl Into<SharedString>, size: DialogSize, panel: Entity<P>,
    footer: Option<DialogFooter>,
);

#[derive(IntoElement)]
pub struct DialogFrame { /* nav, body */ }
impl DialogFrame {
    pub fn new(size: DialogSize) -> Self;
    /// Left nav (≥ 4 sections: chart Settings) or top tabs (≤ 3: indicator settings).
    pub fn nav(self, nav: NavTabs) -> Self;
    pub fn body(self, body: impl IntoElement) -> Self;          // scrolls inside the fixed body height
}

pub struct NavTabs { /* items: (id, label, Option<Glyph>), selected, on_change */ }
impl NavTabs {
    pub fn vertical() -> Self;    // W_NAV 200, rows 36, glyph 16, selected = selection bg
    pub fn horizontal() -> Self;  // gpui-component TabBar::underline().small()
    pub fn item(self, id: &'static str, label: impl Into<SharedString>, glyph: Option<Glyph>) -> Self;
    pub fn selected(self, id: &'static str) -> Self;
    pub fn on_change(self, f: impl Fn(&'static str, &mut Window, &mut App) + 'static) -> Self;
}

#[derive(IntoElement)]
pub struct DialogFooter { /* template, cancel, ok */ }
impl DialogFooter {
    pub fn new() -> Self;                                         // h 60, border-top, px 20
    pub fn template(self, menu: TemplateMenu) -> Self;            // left: "Template ▾"
    pub fn cancel(self) -> Self;                                  // outline, 32, min-w 72
    pub fn ok(self, label: impl Into<SharedString>) -> Self;      // primary, 32, min-w 72, rightmost
    pub fn on_ok(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self;
    pub fn ok_disabled(self, d: bool) -> Self;
}

pub struct TemplateMenu;  // ::defaults(on_apply) → rows "Apply defaults" (later "Save as default…")
```

Rules:

- Esc and × both mean Cancel. Settings dialogs restore the original state on Cancel, as they already do via `on_release`.
- Enter means Ok unless focus is in a multi-line field.
- The title is 16/600 in the header with the × at the right (gpui-component default).
- Picker dialogs (Symbol search, Indicators) have no footer, and Enter picks.

### 3.7 Forms

```rust
#[derive(IntoElement)]
pub struct FormRow { /* label, label_width, controls, check */ }
impl FormRow {
    pub fn new(label: impl Into<SharedString>) -> Self;          // h ≥ 36, label 14 text, one line + ellipsis
    /// Checkbox + label in the label column; extra controls align to the control column.
    pub fn check(id: impl Into<ElementId>, label: impl Into<SharedString>, checked: bool,
                 on: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self;
    pub fn label_width(self, w: Pixels) -> Self;                  // default from DialogSize
    pub fn control(self, c: impl IntoElement) -> Self;            // gap S3 between controls
    pub fn unit(self, u: impl Into<SharedString>) -> Self;        // trailing muted 14
    pub fn indent(self) -> Self;                                  // nested option, +24
}

pub struct NumberField;  // NumberField::new(&Entity<InputState>).width(W_FIELD_NUMBER) — NumberInput, h 28
pub struct Select<T>;    // Select::new(id, options: &[(T, &str)], current: T).on_change(..) — ToolbarButton-like outline trigger, h 28, Menu with ✓
pub struct ColorField;   // ColorField::new(&Entity<ColorPickerState>) — ColorPicker with featured_colors(shared palette); 24 trigger, 16 swatch R_SM, 1 px border
impl ColorField { pub fn compact(self) -> Self; }  // floating toolbar variant (same popover)
```

`Toggle` (binary option) is `FormRow::check` in dialogs, matching TradingView's checkboxes. Toolbar toggles are `IconButton::toggled`. `Switch` is not used, to keep one binary control per context.

### 3.8 Lists and overlays

```rust
pub struct ListRow;      // ListRow::new(id).height(H_LIST_ROW | H_LIST_ROW_TALL).leading(..).primary(..).secondary(..).trailing(..).selected(..)
pub struct ListNav;      // up/down/home/end/page over `len`, with ScrollHandle::scroll_to_item; used by search + indicators
pub struct EmptyState;   // centered muted 14, padding S6
pub struct Tag;          // 11 px, R_SM, panel_alt bg, px 4 ("SPOT")
pub struct Overlay;      // surface from §2.6; .floating() adds a drag handle (Glyph::GripVertical 14)
pub struct PaneControls; // PaneControls::width(buttons: usize) -> Pixels; row of IconButton::Sm
pub mod motion {
    pub fn fade_fast(id: impl Into<ElementId>, shown: bool, window: &mut Window, cx: &mut App) -> f32;
    pub fn snappy(ms: u64) -> Transition;   // moved from ui.rs
}
```

---

## 4. Migration plan

### Rules for every slice

- Use tokens and kit components only. No new `px(…)` literals for sizes, radii or text outside `tt-ui` (exception: layout geometry from the chart layout).
- No `Color32::from_rgb`, `gpui_kit::red()` or font names outside `tt-theme`/`tt-ui`.
- Each slice ends with dark and light screenshots of its surfaces (use the existing `shot.ps1` flow) and a check with Windows "Animation effects" off.
- Each slice is one PR. Slices 1–8 touch disjoint files after slice 0's file split, so they can run in parallel.

### Slice 0: build the kit (blocks all others, about 2–3 days)

Files:

- `crates/tt-theme/src/lib.rs`: new color tokens (§2.5), `badge_fg()`, mark `accent` deprecated in docs.
- New `crates/tt-theme/src/metrics.rs` (§2.1–2.4).
- New `crates/tt-ui/**` (§3), plus `examples/gallery.rs`.
- Root `Cargo.toml` workspace members; `crates/tradeterm/Cargo.toml` and `crates/tt-chart-gpui/Cargo.toml` gain `tt-ui`.
- `crates/tradeterm/src/ui.rs`: `apply_theme` calls `tt_ui::theme::sync`. `lucide`, `separator`, `snappy` and `chart_type_icon` become thin re-exports of kit items, so other slices compile before they migrate.
- `crates/tradeterm/src/main.rs`: `tt_ui::init(cx)`.
- **Pure-move file split** with no behavior change, to make the later slices disjoint:
  - `app.rs` → `app.rs` (state and actions), `title_bar.rs` (`title_bar`, `ui::tab_pill`), `top_toolbar.rs` (`toolbar`), `status_bar.rs` (`status_bar`, `perf_overlay`), `dialogs.rs` (`open_search`, `open_indicators`, `open_indicator_settings`, `open_chart_settings`, `open_custom_interval`).
  - `menus.rs` → `menus.rs` and `custom_interval.rs` (`CustomInterval`).
- Actions for drawing tools and "/" (A17) in `app.rs`, so tooltips can show bindings.

Done when the gallery renders every component in both themes, and the measured bounds match the token table (test in `tt-ui`).

### Slice 1: title bar, tabs, status bar

Files: `title_bar.rs`, `status_bar.rs`, `ui.rs` (`StatusDot` stays).
Fixes: A1–A4, A11–A15, T3.
Notes: the settings menu becomes a kit `Menu` (with the developer section), and the perf overlay uses `Overlay`.

### Slice 2: top toolbar and toolbar menus

Files: `top_toolbar.rs`, `menus.rs`.
Fixes: A5–A10, M1–M9, M11, M12.
Notes: the interval menu uses `Menu::collapsible_section` and `MenuRow::favorite/removable`; "Add custom interval…" moves to the top. The chart-type menu uses `MenuRow::icon(chart_type_glyph)`.

### Slice 3: left drawing toolbar and flyouts

Files: `drawing_toolbar.rs`.
Fixes: D1–D10.
Notes: use `IconButton::size(Lg).toggled()`, `with_flyout()` anchored TopLeft to the right of the trigger, the remove confirmation as a danger `MenuRow`, and `W_DRAWING_TOOLBAR`. Verify that the flyout never covers rows 1–2 (title bar and toolbar) at window heights from 400 to 1200.

### Slice 4: in-chart gpui controls

Files: `crates/tt-chart-gpui/src/chart.rs`; `crates/tt-chart-gpui/src/lib.rs` (drop `ICON_FONT` duplication; re-export from tt-ui).
Fixes: G1–G14.
Notes: floating toolbar becomes `Overlay::floating()` with top-center default, drag offset kept in `Chart` state, and `PaneControls::width()` collision avoidance. Pane controls fade out correctly. The price-scale menu becomes a kit `Menu`. Chrome colors come from `UiTheme`.

### Slice 5: canvas legend and tags

Files: `crates/tt-chart/src/draw.rs`; `crates/tt-chart/src/render.rs` (or wherever `Canvas` is declared; add `stroke_rounded_rect`); `crates/tt-chart-gpui/src/canvas.rs` (implement it); `RenderInput` (hovered legend button).
Fixes: L1–L5.
Notes: no gpui kit here. Use `tt_theme::metrics` only. Legend button tooltips are wired in the chart view: a small hook in `chart.rs` that reads `LegendHit` rects, coordinated with slice 4. Land slice 4 first, or agree on the `legend_tooltip` fn signature up front.

### Slice 6: symbol search dialog

Files: `search.rs`, the `open_search` part of `dialogs.rs`.
Fixes: S1–S7.
Notes: use `ListRow::tall`, `Chip`, `SectionHeader("RECENT")`, `ListNav`, `DialogSize::Md`, and the `active` highlight.

### Slice 7: indicator dialogs

Files: `indicators.rs`, the `open_indicators` / `open_indicator_settings` part of `dialogs.rs`.
Fixes: I1–I11, M1 (second gold).
Notes: Indicators uses `DialogSize::Lg` with `NavTabs::vertical()` (Favorites / Technicals) and `ListNav`. Indicator settings uses `DialogSize::Sm`, `NavTabs::horizontal()` (Inputs / Style / Visibility), `FormRow`, `NumberField`, `Select<Source>`, `ColorField`, and `DialogFooter` with `TemplateMenu`.

### Slice 8: chart settings and custom interval dialogs

Files: `chart_settings.rs`, `custom_interval.rs`, the `open_chart_settings` / `open_custom_interval` part of `dialogs.rs`.
Fixes: C1–C8, M10.
Notes: the C1 bug fix comes first (chrome from `UiTheme`). Then `DialogSize::Lg` + `NavTabs::vertical()`, `FormRow::check` with color fields, and `Select` for scale buttons and font size (integer sizes). Custom interval uses `DialogSize::Sm`, `ToggleGroup::segmented()` for units, and a footer with "Add".

### Slice 9: cross-cutting QA (after 1–8)

Files: none, or fixes only.

- Light-theme pass over every screenshot (T6).
- Reduce-motion pass.
- Keyboard pass: Tab order in dialogs, menu arrows, Esc/Enter.
- `rg 'px\((9|10|11\.5|12\.5|13\.5|14\.5)' crates/` and `rg 'from_rgb|IconName::' crates/tradeterm crates/tt-chart-gpui` must return nothing outside `tt-ui`/`tt-theme`.
- Update `docs/specs` screenshots.

### Parallelism summary

| Slice | Depends on | Touches |
|---|---|---|
| 0 Kit | n/a | tt-theme, tt-ui (new), Cargo manifests, ui.rs, main.rs, file split of app.rs/menus.rs |
| 1 Chrome | 0 | title_bar.rs, status_bar.rs, ui.rs |
| 2 Top toolbar | 0 | top_toolbar.rs, menus.rs |
| 3 Drawing toolbar | 0 | drawing_toolbar.rs |
| 4 In-chart controls | 0 | tt-chart-gpui/chart.rs, lib.rs |
| 5 Canvas legend | 0 (+ hook agreed with 4) | tt-chart/draw.rs, Canvas trait, tt-chart-gpui/canvas.rs |
| 6 Search | 0 | search.rs, dialogs.rs (open_search) |
| 7 Indicators | 0 | indicators.rs, dialogs.rs (two fns) |
| 8 Settings | 0 | chart_settings.rs, custom_interval.rs, dialogs.rs (two fns) |
| 9 QA | 1–8 | fixes only |

Slices 6–8 edit different functions in `dialogs.rs`. Each function is self-contained, so merges are trivial, or each slice can move its opener into its own module.

---

## Appendix: TradingView reference points used

- The drawing toolbar is about 52 px wide (TradingView's production CSS bundle). We use 48 to match our 36 px buttons.
- The top toolbar is about 38–40 px tall with about 32–34 px buttons, measured from screenshots. Treat this as a reference only, since TradingView's internal CSS is not a stable API.
- Conventions adopted from TradingView:
  - Active drawing tool and active A/L/% shown in blue.
  - Stars to pin intervals and chart types.
  - Collapsible interval sections.
  - Dialog footer with Template ▾ on the left and Cancel / Ok on the right.
  - Settings dialog with left navigation; indicator settings with top tabs.
  - Floating drawing toolbar that can be dragged.
  - Chart-pane buttons at the top right of the hovered pane.

Sources: [TradingView Charting Library: UI map](https://tradingview.com/charting-library-docs/ui-map/), [TradingView Charting Library: Toolbars](https://tradingview.com/charting-library-docs/v28/ui_elements/Toolbars), [TradingView static CSS bundle](https://static.tradingview.com/static/bundles/52955.82c44d0e97934be0a8c6.css).
