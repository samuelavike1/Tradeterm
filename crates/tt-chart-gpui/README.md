# tt-chart-gpui

gpui backend for the tradeterm chart.

- `GpuiCanvas`: `tt_chart::render::Canvas` on gpui. Rects → instanced quads;
  lines/fills → triangle paths (consecutive same-color triangles merged, draw
  order preserved); grid → one dashed-border quad per line; text → `shape_line`.
- `Chart`: the chart as two gpui views, like TradingView's paired canvases.
  The **base** (grid, bars, indicators, axes, value tags) is a cached view,
  repainted only after `Chart::invalidate`; the **overlay** (crosshair, axis
  tags, readout, legends) repaints on pointer moves.

## Benchmark

`cargo run -p tt-chart-gpui --release --example bench -- <hover|pan|static|empty> [secs] [noind]`

1M synthetic 1m bars + Volume, EMA 21, BB 20, RSI 14, MACD; 1920×1040,
Windows 11, 60 Hz. "Our CPU" is time in our paint code; "process CPU" is the
whole process (gpui scene submission, DirectX, driver) per frame.

| Scenario | fps | base repaints | overlay | process CPU / frame |
|---|---|---|---|---|
| Empty window (baseline) | 60 | – | – | 2.2 ms |
| Static chart, redrawn every frame | 60 | 0 | 0.78 ms | 8.9 ms (5.0 ms without indicators) |
| Hover (crosshair moves) | 60 | 0 | 0.78 ms | 8.5 ms |
| Pan, zoomed in (300 bars) | 60 | every frame, ~4.7 ms | ~0.9 ms | – |
| Pan, zoomed out (1M bars) | 49–60 | every frame, 5.5–14.6 ms | ~1.3 ms | – |

Takeaways: the layer split works (hovering never repaints the base, and our
per-move work is under 1 ms). What remains on a hover frame is gpui
re-presenting the cached scene: about 2.8 ms for ~1,300 quads/glyphs and
3.9 ms for the indicator line/fill paths. That is the next thing to profile.
