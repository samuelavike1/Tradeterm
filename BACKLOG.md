# Backlog

Features seen in TradingView (or planned) that were deliberately left out so far.
Add to this list whenever something is skipped; remove an entry when it lands.

## Price scale
- Move scale to the left (and both sides).
- Lock price to bar ratio.
- Scale price chart only (indicators keep their own range).
- Labels › / Lines › submenus in full (high/low labels, bid/ask, pre/post market,
  previous day close, symbol name label).
- Plus button on the price scale (quick alert / order at a price).

## Chart settings dialog
- Trading tab (order/position lines, execution marks, buy/sell buttons).
- Alerts tab.
- Events tab (dividends, splits, earnings, economic events).
- Template menu (save / load / apply default settings) in the dialog footer.
- Timezone setting (time axis is UTC today).
- Watermark (symbol + timeframe behind the bars).
- Session breaks and session background on intraday charts.
- Status line: bar change in values, symbol logo, legend background / opacity.
- Canvas: background gradient, pane separator color, crosshair style
  (solid / dashed / dotted) and width, navigation buttons, pane buttons visibility.

## Panes and indicators
- Move the price pane below indicator panes (we keep it on top).
- "Manage panes" menu (shown when the chart is too narrow for the buttons).
- Several indicators in one pane; "Move to" another / new pane / the price pane.
- Indicator legend "More" (…) menu and source-code button.
- Highlight the pane divider while hovering / dragging it.
- Fade-out of the pane buttons (they fade in, but vanish instantly).
- Remember a maximized pane across restarts.

## Drawing tools
- Remaining tools: channels (regression, flat top/bottom, disjoint); pitchforks;
  Fib channel / time zone / fans /
  circles / spiral / arcs / wedge, pitchfan; Gann box / square / fan; chart patterns
  (XABCD, Cypher, head and shoulders, ABCD, triangle, three drives); Elliott waves;
  cycles (cyclic lines, time cycles, sine line); long / short position, position
  forecast, bars pattern, ghost feed, sector; anchored VWAP, fixed range / anchored
  volume profile; brush, highlighter, arrows, rotated rectangle, path, circle,
  ellipse, polyline, triangle, arc, curves; text, note, price note, pin, table,
  callout, comment, price label, signpost, flag mark, image; icons / emojis.
- Cursors: dot, arrow, demonstration, magic, eraser.
- Measure (Shift + click) and zoom-in tools on the toolbar; strong magnet mode.
- Drawing settings dialog (double-click a drawing): style, text, coordinates,
  visibility per timeframe; Fib level list editing; templates.
- Undo / redo (Ctrl + Z / Ctrl + Y); copy / paste / clone drawings.
- Rectangle: side and middle handles; extend left / right.
- Drawings on indicator panes (today they live on the price pane).
- Favorite drawing tools toolbar (floating, starred tools).
- Live sync of drawings between two tabs showing the same symbol (synced when
  switching tabs today).
- Remember magnet / keep drawing / lock / hide across restarts.
- Flyout menus open centered on their button (TradingView aligns their top).
- Object tree (list of drawings and indicators).
- Floating drawing toolbar "Add alert" button (TradingView's clock-plus
  between settings and lock): needs the alert engine (see the app bar's
  "Create alert").
- Ticks and Ranges visibility rows only persist: tradeterm has no tick or
  range-based charts yet.
- TradingView-measured settings dialog layout (docs/specs/ui-guidelines.md
  §2.10): only the horizontal ray and horizontal line use it so far; the
  other tools opt in through `form::dialog_layout`.
- Palette popover "+" (custom color) and an editable opacity percent: the
  palette has swatches and an opacity slider only.

## Chart types
- Kagi, Point & figure.
- Volume footprint, TPO, volume candles and session volume profile (need tick data).

## Data / connection
- First chart at startup still takes ~3–4 s to show history (cold REST fetch).
- Live stream attaches ~1 s after history appears.

## Top app bar (docs/specs/appbar.md)
- Compare / add symbol (+): needs a second live series per chart, a percent
  overlay on the price pane (or its own scale / pane) and a legend row for it.
- Create alert (Alt + A): needs an alert engine evaluating conditions on live
  data, an alert list and notifications.
- Bar replay: needs a replay source (history cut at a bar, timed feed, step /
  speed) and the replay toolbar on the chart.
- Layout setup: several charts per layout (1–16 grids) with symbol / interval /
  crosshair / time / date-range sync. Today one chart per tab.
- Share layout, snapshot "Copy link" / "Open in new tab" / "Post image",
  Publish idea: need a server to host layouts and images.
- Trade button / trading panel: no broker execution yet.
- Undo / redo covers drawings only; indicator, chart type, symbol and interval
  changes are not in the history. TradingView's tooltip names the action
  ("Undo create trend line").
- Layouts carry tabs only; drawings stay per symbol across layouts (TradingView
  can keep drawings per layout). Unsaved-changes prompt when switching with
  autosave off (changes are dropped today).
- Download chart data: OHLCV only (TradingView adds indicator columns).
- Indicator templates: TradingView's tabbed menu (Indicators / Templates) is one
  menu with sections here; "Technicals" built-ins are limited to the indicators
  we have (no Bill Williams 3 Lines, Displaced EMA, MA Exp Ribbon, Swing
  Trading, Volume Based).

## Chart settings (hidden until they work)
- Bid and ask labels/lines: needs a Binance `bookTicker` stream (new MarketEvent,
  throttled repaints). The model fields exist; the dialog rows are hidden.
- Navigation buttons on the chart (zoom / scroll / reset at the bottom): setting
  exists in the model, dialog row hidden.
- Pre/post market and Plus button on the price scale (n/a for crypto / blocked).

## Drawing settings (TradingView parity follow-ups)
- Rotated text on the chart canvas (vertical line text is stacked upright today).
- Fib: per-level text row and the rainbow line-color button on the floating toolbar.
- Trend angle: "Angle" field on the Coordinates tab.
- Parallel channel: dim unchecked level rows like TradingView.

