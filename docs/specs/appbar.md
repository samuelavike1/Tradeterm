# Top app bar and title bar

The bar under the tab strip (TradingView's "header toolbar") and the title bar
above it. Inventory taken from tradingview.com/chart (signed in, 1600 px wide,
November 2026) by reading every control's tooltip / aria label and opening its
menu, plus the TradingView help center.

Status legend: **done** = works in tradeterm; **new** = added by this slice;
**backlog** = listed in `BACKLOG.md` with the reason (no dead buttons).

## 1. Inventory, left to right

| # | Control | TradingView behaviour | Our status | Plan / notes |
|---|---------|----------------------|------------|--------------|
| 1 | Symbol search (ticker field) | Field-style button with the symbol ("BTCUSD"); opens symbol search. Typing on the chart opens it too. | done | `ToolbarButton::field`. |
| 2 | Compare or add symbol (+) | "Compare symbols" dialog: overlay another symbol as a % line on the price pane, or add it on a new price scale / pane. | backlog | Needs a second live series in a chart, percent-scale overlay and its own legend row; the series pipeline (`tt-chart`, `chart.rs`) has one main series. |
| 3 | Interval buttons | Favorite intervals as text buttons ("1m 3m 5m 15m 30m 1h 4h D W M"); tooltip is the long name ("1 minute"). | done | Starred in the interval menu. |
| 4 | Interval menu (chevron) | Tooltip "Chart interval". "Add custom interval…", collapsible SECONDS / MINUTES / HOURS / DAYS (+ TICKS, RANGES), star to pin, × on custom rows. | done | Ticks and ranges need tick data (backlog: data). |
| 5 | Chart type | Current type's icon + caret; tooltip is the type ("Candles"); grouped list with stars, favorites as quick buttons. | done | |
| 6 | Indicators | "Indicators, metrics, and strategies", `/`. Opens the indicators dialog. | done | |
| 7 | Indicator templates (caret next to Indicators) | "Favorite indicators and indicators templates": two tabs, **Indicators** (FAVORITE INDICATORS; a click adds it) and **Templates** (Save indicator template…, my templates, built-ins; apply replaces the indicators). | new | `indicator_templates.rs`: one kit menu with FAVORITE INDICATORS and TEMPLATES sections (the kit menu has no tabs; sections keep it one click), save dialog (name, remember symbol / interval), apply, delete. Spec: indicators.md §6 / slice I-8. |
| 8 | Create alert | Alarm-clock icon + "Alert", Alt + A. Alert dialog (condition, trigger, expiry, notifications). | done | Local alerts, see alerts.md. |
| 9 | Bar replay | Rewind icon + "Replay", Shift + ↓ to step. Replay toolbar at the bottom of the chart (select bar, play, speed, step). | backlog | Needs a replay data source (history cut at a bar, timed feed) in `tt-session` and a replay toolbar in the chart. |
| 10 | Undo | Ctrl + Z, tooltip "Undo <action>". Disabled when nothing to undo. | new | Drawing history (`Chart::undo_drawing`), disabled state from `can_undo`. Indicator / chart-type changes are not in the history yet (backlog). |
| 11 | Redo | Ctrl + Y, tooltip "Redo <action>". | new | `Chart::redo_drawing`. |
| 12 | Layout setup | Grid icon: 1–16 charts per layout in fixed arrangements; SYNC IN LAYOUT toggles (symbol, interval, crosshair, time, date range). | backlog | Needs several chart views in one tab and crosshair / time sync between them. Our tabs cover one-chart-per-tab today. |
| 13 | Layout name + Save | Shows the layout name; "All changes saved" / "Save" state; click saves (Ctrl + S). | new | Named layouts (chart-settings.md S12). |
| 14 | Manage layouts (caret) | Save layout (Ctrl + S), Autosave (toggle), Share layout, Make a copy…, Rename…, Download chart data…, Create new layout…, RECENTLY USED, Open layout… | new | All except **Share layout** (needs a server, backlog). Download chart data writes the active chart's bars as CSV. |
| 15 | Quick search | Magnifier, Ctrl + K. Command palette over actions, settings, drawing tools, indicators. | new | `quick_search.rs`: a kit dialog over app actions (intervals, chart types, indicators, drawing tools, tabs, theme, layouts, snapshot…). |
| 16 | Settings | Gear, opens the chart Settings dialog. | new | App bar gear opens chart settings. The title bar keeps the app menu (theme, developer). |
| 17 | Fullscreen mode | Shift + F; toggles browser fullscreen. | new | `Window::toggle_fullscreen`. |
| 18 | Take a snapshot | Camera. Menu CHART SNAPSHOT: Download image (Ctrl + Alt + S), Copy image (Ctrl + Shift + S), Copy link (Alt + S), Open in new tab, Post image. | new | Download / Copy image: the chart is rendered off-screen through `tt_chart::render::Canvas` on tiny-skia (cross-platform, no crosshair), plus a header line (symbol, interval, date). Copy link / Open in new tab / Post image need an image host: backlog. |
| 19 | Publish (Share your idea) | Publish an idea to the TradingView community. | backlog | No community backend. |
| 20 | Trade | Opens the trading panel (brokers). | backlog | No broker execution yet. |
| 21 | Upgrade | Subscription upsell. | not planned | Out of scope by request. |

TradingView's left-most avatar ("D") is the account / main menu (theme,
language, keyboard shortcuts, help). Our equivalent is the title bar's main
menu (theme, chart settings, developer tools).

## 2. Title bar (tradeterm only)

TradingView runs in a browser, so its tabs are browser tabs. Ours:

| Control | Behaviour | Status |
|---------|-----------|--------|
| Tab pill | Badge, symbol, change %, × on hover; middle-click closes; active fill eases in. | done |
| New tab (+) | Ctrl + T. | done |
| Theme toggle | Sun / moon, Ctrl + Shift + T. | done |
| Main menu | Theme (Dark / Light / System), Chart settings…, Developer (performance overlay F12, synthetic bars). | done (icon changed from gear to menu so the app bar gear is the only "Settings") |

## 3. Consistency rules (both bars)

- Heights: title bar `H_TITLE_BAR` 34, buttons `CONTROL_SM` 24 with `ICON_SM`
  icons; app bar `H_TOOLBAR` 40, buttons `CONTROL_MD` 32 with `ICON_MD` icons.
- Every control is a kit `IconButton` / `ToolbarButton`; groups are separated by
  `Divider::vertical()`; gap `S1` between groups, `S0_5` inside a group.
- Tooltips: sentence case, no period, shortcut as a Kbd chip, placed below.
- Menus: kit `Menu` (32 px rows, uppercase section headers, shortcut column with
  `MenuWidth::Lg`).
- Right group of the app bar (TradingView order): undo, redo | layout |
  quick search, settings, fullscreen, snapshot.
