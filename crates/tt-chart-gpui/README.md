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
Windows 11, 60 Hz. "Our paint" is time in our paint code; "process CPU" is
the whole process per frame (gpui frame building, scene submission,
DirectX, driver).

| Scenario | fps | base repaints | overlay paint | process CPU / frame |
|---|---|---|---|---|
| Empty window (floor) | 60 | – | – | 2.2 ms |
| Static chart, redrawn every frame | 60 | 0 | 0.56 ms | 5.9 ms |
| Hover (crosshair moves) | 60 | 0 | 0.83 ms | 6.3 ms |
| Pan (half the time over all 1M bars) | 58.9 | every frame, 5.2 ms | 0.81 ms | 11.5 ms |

## Profiling notes (gpui-kit 0.10 / gpui-pre 0.3.12, Windows)

Measured with timers in a scratch copy of gpui's window and DirectX renderer:

- gpui gives every primitive a draw order by inserting it into a bounds
  tree, also when a cached view is replayed. With ~3,000 primitives that was
  4–5 ms per frame. `GpuiCanvas::finish` now paints quads and text inside
  `paint_layer`s, where primitives share the layer's order and skip the
  tree: frame building went from 5.8 to ~1.4 ms.
- Every batch of paths costs a full-window 4× MSAA clear + resolve. Paths
  are painted between the quad and text layers so they form one batch.
  That batch still costs ~1.5 ms of renderer CPU; reducing it needs a gpui
  change (clear/resolve only the paths' bounds) or fewer path pixels.
- A quad exactly as deep as its dashed border renders nothing; grid lines
  are 3 px deep quads with only the leading edge dashed.
