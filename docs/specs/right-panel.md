# Right panel: watchlist, alerts, object tree and data window

TradingView reference, measured 2026-11-21 on tradingview.com/chart (light theme, signed in), in CSS px from computed styles (the browser ran at devicePixelRatio 1.22, so screenshots look 22 % bigger than these numbers). Screenshots: `img/tv-ref/tv-rightpanel-*.png`. Nothing was changed on the TradingView account: panels and menus were opened, measured and closed again (all three sidebar buttons were off before and after).

Scope here: the first three buttons of TradingView's right sidebar. The others (AI Copilot, screeners, Pine, calendar, news, …) are deliberately left out (no dead buttons).

## 1. Sidebar strip

| | TradingView | tradeterm token |
|---|---|---|
| Strip width | 45 (44 buttons + 1 px left border) | `W_SIDEBAR` 45 |
| Buttons | 44 × 44, no radius on the strip, pitch 46 (2 px gap) | `SIDEBAR_BUTTON` 44, `S0_5` gap |
| Icons | own 44-box icons with about 22 px of ink, 1 px strokes | our 28-box 1-stroke SVGs at 28 (≈ 20 px ink) |
| Order | Watchlist, details and news · Alerts · Object tree and data window | same |
| Tooltips | to the left: "Watchlist, details, and news", "Alerts", "Object tree and data window" | same |
| Toggle | click opens the panel to the left of the strip, click again closes it; one panel at a time | same |

Icons (TradingView's shapes, drawn again in our style by `scripts/icons/gen.py`):

- `panel-watchlist`: a bookmark outline with two list lines.
- `panel-alerts`: an alarm clock (circle, two bells, hands at 12 and 9).
- `panel-object-tree`: two stacked layers (a diamond and the edge of the one below).

## 2. Panel frame

- Panel width 300 + a 1 px border on its left (302 incl. the strip border); resizable by dragging its left edge in TradingView. Ours: default 300, min 240, max 600, persisted.
- Panel header row: 48 tall area, 38 px buttons (`Add symbol` +, `Advanced view`, `Settings` …), 8 px side padding.

## 3. Watchlist

- Header: the list name as a menu button ("Watchlist ⌄", 14 / 600), then `+` (Add symbol), advanced view, `…` (settings). 38 × 38 buttons.
- List menu (210 wide, 32 px rows, 14 px): Share list (toggle) · Add alert on the list… · Make a copy… · Rename · Add section · Clear list ── Create new list… · Upload list… ── Open list… (Shift+W). Ours: Create new list…, Rename…, Make a copy, Add section, Clear list, Delete list, then the other lists to switch to.
- Settings menu (`…`): Table view · GROUP BY · CUSTOMIZE COLUMNS: Last, Change, Change %, Volume, Extended hours (checkboxes) · SYMBOL DISPLAY: Logo (checkbox), Symbol / Name (radio). Ours: Last, Change, Change %, Volume, Logo, Symbol / Name.
- Column header row: 27 tall, 14 px `text_muted` (#707070); columns Symbol 78 · Last 73 · Chg 68 · Chg% 60 (right-aligned numbers) at a 300 px panel; clicking a header sorts (TradingView: asc, desc, off).
- Rows: 30 px pitch, 1 px `#f2f2f2` bottom rule; 18 px round logo 8 px in, ticker 14 / 400 at x + 24; values 14 px, Chg and Chg% in TradingView red / green (#F23645 / #089981 … ours: theme `down` / `up`); the active chart's symbol is highlighted. Hover `hover` fill.
- Row context menu: Flag/Unflag (colors), Add to watchlist ›, Add to compare, Add note, Add section, Add symbol. Ours: Open in new tab, Add section above, Remove from watchlist.
- Sections: a caption row (uppercase, collapsible with a caret) that groups the rows under it; dragging rows reorders them (also across sections).
- Details area under the list (split): symbol logo + ticker, full name · exchange, big last price with the smaller trailing digits, change and change %, then Volume and average volume. Ours: logo, ticker, name, last price (24 px), change / change % colored, 24 h high / low / volume / quote volume.

## 4. Object tree and data window

- Two segmented tabs at the top: "Object tree" / "Data window", 130 × 28 each, 14 / 500; the active one has a bordered white pill.
- Object tree toolbar (38 px buttons): create group, clone, move to … and at the right "Manage layout drawings". Ours: none of these exist yet, so no toolbar buttons (no dead buttons).
- Rows: 38 px pitch, 22 px icon box 8 px in, label 14 px at x + 58 from the panel edge. Order: the main series first ("BTCUSD · CRYPTO, 30", 14 / 600, chart-type icon), then indicators (the price pane ones, then panes; TradingView lists them latest first), then drawings, top of the z-order first.
- Hidden items are drawn in `text_disabled` (#B8B8B8) with their eye shown; hovered rows show 22 × 22 buttons at the right: lock (drawings), eye, trash.
- Click selects (drawings: selects on the chart), double-click renames a drawing, drag reorders drawings (z-order).
- Data window: Date / Time rows (26 px pitch, label left, value right), then one section per series and indicator: a 38 px header (collapse chevron, icon, bold title, eye) on a light `panel_alt` fill, then 26 px rows: Open / High / Low / Close / Change "−33.43 (−0.06%)" / Vol / Last day change, colored by the bar's direction (red / green). Indicator sections list each plot's value in the plot's color.
- With no crosshair the data window shows the last bar.

## 5. Alerts mount point

The alerts panel is built separately (`crates/tradeterm/src/alerts_panel.rs`, `pub struct AlertsPanel` with `pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self` and `impl Render`). The sidebar creates it lazily the first time the Alerts button opens and keeps the entity; it renders it full-size in the panel area. Until the alerts work lands, a stub with the same contract shows an empty state.
