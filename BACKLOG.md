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
- Remaining tools: info line, trend angle; channels (parallel, regression, flat
  top/bottom, disjoint); pitchforks; Fib extension / channel / time zone / fans /
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

## Chart types
- Kagi, Point & figure.
- Volume footprint, TPO, volume candles and session volume profile (need tick data).

## Data / connection
- First chart at startup still takes ~3–4 s to show history (cold REST fetch).
- Live stream attaches ~1 s after history appears.
