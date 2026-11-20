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

## Chart types
- Kagi, Point & figure.
- Volume footprint, TPO, volume candles and session volume profile (need tick data).

## Data / connection
- First chart at startup still takes ~3–4 s to show history (cold REST fetch).
- Live stream attaches ~1 s after history appears.
