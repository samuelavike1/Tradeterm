# gpui spike (branch `spike/gpui`)

Question: move tradeterm from egui to gpui before the app grows?
Stack tested: `gpui-kit 0.10.0` = `gpui-pre 0.3.12` (snapshot of zed@d2a4fb6)
+ `gpui-component 0.10.0` (Longbridge; powers Longbridge Pro).
Windows 11, 1936×1048, scale factor 1.0, release builds.

## What was built

- `tt-chart`: `ChartView::render(&mut dyn Canvas, …)`, a backend-neutral
  frame (layout, caches, drawing); `show()` is now the egui front end on top.
  Public `pan`/`zoom`/`last_layout` for backends that do their own input.
- `src/canvas.rs`: `Canvas` on gpui. Quads → `paint_quad`; lines/fills →
  triangle `Path`s merged per color and clip; text via `shape_line`;
  axis-aligned dotted grid → one dashed-border quad per line.
- `src/main.rs`: the chart as a gpui view (drag pan, wheel zoom, crosshair).
- `src/live.rs`: 3 charts + 200-row watchlist ticking at 10 Hz.
- `src/ui.rs`: title bar, toolbar, chart-type dropdown and symbol search
  dialog with gpui-component.
- `src/bin/egui_bench.rs`, `src/bin/egui_live.rs`: the same scenes on egui.

Run: `tt-gpui-spike [--bench 24 | --live 3 | --ui]`, `egui_bench 24`, `egui_live`.

## Results

Chart: 1M synthetic 1m bars + Volume, EMA 21, BB 20, RSI 14, MACD; pans every
frame for 24 s, alternating zoomed out (all bars, decimated) and 300 bars.

| | egui | gpui |
|---|---|---|
| Avg fps (vsync 60) | 60.1 | 58.6 |
| Zoomed in, CPU per frame | ~2.2 ms | ~5.5 ms (record 2 + replay 3.6) |
| Zoomed out, CPU per frame | ~9.6 ms | ~13 ms (record 5 + replay 8), 52–60 fps |
| Process CPU over 24 s | 12.8 s | 18.4 s |

Live (3 charts + 200-row watchlist, 10 Hz):

| | egui | gpui |
|---|---|---|
| CPU (one core) | 13 % | 25.5 % |
| Working set | 211 MB | 127 MB |

Other:

| | egui | gpui |
|---|---|---|
| Release binary | 15.6 MB | 23.0 MB |
| Incremental release build (thin LTO) | 58 s | 102 s |
| Incremental dev build | 11 s | 7 s |
| Cold dev build of the UI stack | (already cached) | ~15 min, one-time |
| Title bar + toolbar + dropdown + search dialog | ~2,000 lines hand-painted (more features) | ~290 lines with components |

## Notes

- The first gpui canvas ran at 13 fps; fixes were in our code: a `Prim` enum
  bloated to 1.4 KB by `ShapedLine`'s inline SmallVec, per-dot meshes for the
  dotted grid. gpui has no tiling-texture or custom-shader hook, so the grid
  is a dashed-border quad: gpui's dash pattern is fixed (2 on / 1 off), not
  TradingView's 1 on / 3 off.
- Also found two chart-core bugs that hit egui too (fixed here, worth keeping
  on main): RSI/MACD/Stochastic recomputed over the whole series every frame
  even without new data; auto-fit scanned every indicator value of the visible
  range (8M reads at 1M bars) instead of the drawn samples.
- gpui-component needs `application().with_assets(gpui_kit::assets::Assets)`
  or every icon (and the window buttons) is blank.
- Not tested: DPI other than 100 %, IME, multiple monitors, detached windows.
