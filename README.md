# tradeterm

A native desktop trading/charting app written in Rust. Rendering is
[egui](https://github.com/emilk/egui)/eframe on wgpu. It uses no web technology
(no Tauri, Electron or WebView).

It streams live Binance spot candles from public, key-less endpoints and draws them
with a custom chart widget built to handle a million bars.

## Features

- **Custom window chrome:** the OS title bar is replaced by our own. It has a tab
  strip (symbol badge, symbol, 24h change %, close button, "+" for a new tab), a
  settings menu, and minimize/maximize/close buttons. Drag empty space in it to move
  the window (OS snapping works), double-click it to maximize or restore, and drag
  any window edge to resize.
- **Tabs:** each tab is an independent chart with its own live stream, so background
  tabs stay current. Open tabs, their order and the active tab are restored on the
  next launch.
- **Chart types:** Candles, Hollow candles, OHLC bars, Line, Area and Heikin Ashi.
  Heikin Ashi bars are computed in `tt-core` and updated incrementally.
- **Price scales:** linear, logarithmic, and percent (relative to the first visible
  close). The price axis auto-fits the visible bars with 8% padding. Drag the axis to
  scale it manually; double-click it to return to auto-fit.
- **Navigation:** drag the plot to pan (it also pans vertically when the scale is
  manual). Mouse wheel or pinch zooms around the cursor. Drag the time axis to zoom
  horizontally. Double-click the plot to reset the view.
- **Time axis by bar index:** weekend and closed-session gaps take no space.
  Labels step through year → quarter → month → 5 days → day → 12h → … → 1m depending
  on zoom, and never overlap.
- **Price axis:** "nice" 1-2-5 steps. Label precision comes from the symbol's tick
  size (from Binance `exchangeInfo`).
- **Volume pane:** the bottom 20% of the chart, with its own scale and up/down
  colors. Toggle it with **Vol**.
- **Crosshair:** snaps to bar centers, shows labels on both axes (volume inside the
  volume pane), and an OHLCV + change % readout at the top left.
- **Symbol search:** type in the symbol box for live search over all Binance trading
  pairs. Use ↑/↓ and Enter, or click a result. Esc cancels.
- **Backfill:** scrolling near the oldest loaded bar fetches the previous 1000 bars in
  the background and prepends them without moving the view. It stops at 250k bars.
- **Live data:** the WebSocket connects before history is fetched, so no bars are
  missed. Reconnects use exponential backoff (500 ms → 30 s) and reload history while
  keeping backfilled bars.
- **Large data:** below about 2 px per bar, bars are drawn decimated as one min/max
  column per pixel, built from a precomputed pyramid. 1,000,000 visible bars draw in
  well under a millisecond (see [Performance](#performance)).
- **Resampling:** `tt-core` builds higher timeframes from lower ones (1m → 5m, 15m,
  1h, 4h, 1D) with buckets aligned to the UTC clock. It works in batch and
  incrementally.
- **Themes:** semantic color tokens with neutral Dark and Light palettes (pure grays in
  the style of shadcn's "neutral" scale, with TradingView's current up/down colors
  #089981 / #f23645),
  and a System mode that follows the OS. Up/down colors can be overridden. The whole
  UI (panels, widgets, rounding, spacing) uses the tokens.
- **Native fonts:** the UI and chart use the OS UI font (Segoe UI, SF/Helvetica, Noto/DejaVu
  Sans) instead of egui's thin built-in font, with egui's fonts as fallbacks.
- **Settings persistence:** theme, colors, and every tab (symbol, timeframe, chart type,
  scale mode, volume) plus the active tab are saved as JSON, 750 ms after the last
  change and again on exit.

### Hotkeys

| Key | Action |
|---|---|
| Ctrl+T / Ctrl+W (Cmd on macOS) | New tab / close tab |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / previous tab |
| Ctrl+Shift+T (Cmd+Shift+T on macOS) | Toggle dark/light theme |
| F12 | Performance overlay |
| Ctrl+Shift+D (Cmd+Shift+D on macOS) | Load 1,000,000 synthetic bars (stress test) |
| ↑ / ↓, Enter, Esc | Navigate, choose or cancel symbol search results |

| Mouse | Action |
|---|---|
| Drag the plot | Pan (also vertically when the price scale is manual) |
| Wheel / pinch / Ctrl+wheel | Zoom around the cursor |
| Horizontal wheel / trackpad swipe | Pan |
| Drag the price axis | Scale prices manually |
| Double-click the price axis | Return to auto-fit |
| Drag the time axis | Zoom horizontally |
| Double-click the plot | Reset the view |

The **⚙** menu in the title bar has the theme selector, up/down color pickers, the performance
overlay toggle and the synthetic-data command. You can also start with synthetic data
from the command line: `tradeterm --synthetic [N]`.

## Architecture

```
crates/
  tt-core/     Domain types, no UI or network. Symbol, Timeframe, Candle,
               PriceTicks, SymbolInfo, CandleSeries (structure of arrays, with a
               structural version counter), MarketEvent, EventSink, Heikin Ashi,
               resampling.
  tt-theme/    Semantic color tokens (Theme), Dark/Light palettes, ThemeMode, and
               applying the tokens to egui styles.
  tt-brokers/  MarketDataSource trait + Binance adapter (tokio, tokio-tungstenite
               with rustls, reqwest).
  tt-chart/    The chart widget.
     viewport.rs   Pure coordinate math: index_to_x, x_to_index, price_to_y,
                   y_to_price, visible_range, in linear/log/percent modes.
     scale.rs      Nice price ticks, precision, formatting, UTC dates.
     time_axis.rs  Index-based time ticks with weighted, non-overlapping labels.
     bars.rs       The per-frame DrawBar list (one per bar, or one per pixel column).
     lod.rs        Min/max pyramid used for decimation.
     draw.rs       Builds the picture from Canvas primitives only.
     render/       Canvas trait (the backend boundary) + EguiCanvas.
     view.rs       ChartView: input, layout, caches, per-frame orchestration.
  tradeterm/   eframe app. chrome.rs: custom title bar, tabs, window buttons,
               resize edges. tab.rs: one chart session per tab. Also the toolbar,
               symbol search, status bar, settings, perf overlay and synthetic data.
```

**Why a separate `tt-theme` crate:** the tokens style both the chart and the rest of
the app's widgets. If they lived in `tt-chart`, the app's styling would depend on the
chart crate. As their own crate, other panes or a future wgpu renderer can use them
without pulling in chart code.

**Data flow:**

```
tokio thread (background)                 UI thread (eframe)
BinanceSource::run ── EventSink ──► crossbeam channel ──► drained once per frame
  1. connect WebSocket                    │                  └─► CandleSeries ─► ChartView
  2. GET klines (1000) → History          └─ wake() = ctx.request_repaint()
  3. live klines → CandleUpdate
symbol_info / fetch_history(before) / search_symbols ── side requests on the same runtime
```

The UI thread never blocks on network I/O. It repaints only on input or when an
adapter sends an event (or continuously while the perf overlay is open). Each stream
gets its own channel, so after a symbol or timeframe switch, stale events from the old
stream are dropped.

**Rendering boundary:** chart code draws only through `render::Canvas`: rects,
lines, dashed lines, polylines, gradient area fills and text, all in screen points.
`EguiCanvas` implements it on `egui::Painter`, merging consecutive rects and
axis-aligned lines into a single mesh. A custom wgpu renderer would implement the same
trait (for example inside an `egui_wgpu::Callback`); viewport math, ticks, decimation
and interaction stay as they are.

**Caches stay incremental:** `CandleSeries::version()` changes only on structural
edits (history load, prepend, clear). The Heikin Ashi cache and the decimation pyramid
recompute just the last bar or block on live updates, and rebuild only when the
version changes.

## Performance

These numbers come from `cargo run --release -p tt-chart --example bench_draw`, on a
2019 laptop with a Core i7-9750H and Quadro T2000, Windows 11. The setup: a 1,600 × 900
plot, the pointer over the chart (so the crosshair is drawn), and a live update to the
forming bar every frame.

| Bars visible (of 1,000,000) | Chart draw avg / max | egui tessellation avg / max |
|---|---|---|
| 1,000,000 (fully zoomed out) | 0.42 / 0.96 ms | 0.15 / 1.5 ms |
| 200,000 | 0.36 / 1.05 ms | 0.16 / 0.32 ms |
| 20,000 | 0.64 / 1.12 ms | 0.22 / 0.72 ms |
| 1,000 | 0.25 / 0.48 ms | 0.06 / 0.10 ms |
| 150 | 0.12 / 0.40 ms | 0.05 / 0.16 ms |
| Heikin Ashi, 1,000,000 | 0.48 / 0.92 ms | 0.17 / 0.53 ms |
| Area, 1,000,000 | 0.39 / 0.80 ms | 0.35 / 0.85 ms |

These are the in-app F12 overlay readings with all 1M synthetic bars visible:

| Build | Chart draw avg / max | Frame CPU avg / max (whole UI + tessellation) |
|---|---|---|
| release | 0.36 / 1.31 ms | 1.82 / 4.37 ms |
| debug (deps at opt-level 2) | 2.95 / 3.83 ms | 6.51 / 8.72 ms |

"Chart draw" covers the chart's layout and command recording. GPU time isn't included.

## Setup

You need a recent stable Rust toolchain from <https://rustup.rs>. This project was
built with Rust 1.99 on edition 2024.

### Windows

1. Install the **Visual Studio Build Tools** with the *Desktop development with C++*
   workload (this provides the MSVC linker and Windows SDK).
2. Install Rust: download and run `rustup-init.exe` from <https://rustup.rs>.
3. Use an up-to-date GPU driver. wgpu uses DirectX 12 or Vulkan.

### macOS

```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

wgpu renders through Metal, so you don't need anything else.

### Linux

Install a C toolchain, the X11/Wayland/keyboard development headers that winit and
eframe need, and a Vulkan driver for wgpu (wgpu falls back to OpenGL ES if Vulkan isn't
available).

**Debian / Ubuntu**

```bash
sudo apt install build-essential pkg-config cmake \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
  mesa-vulkan-drivers libvulkan1
```

**Fedora**

```bash
sudo dnf install gcc gcc-c++ pkgconf-pkg-config cmake \
  libxcb-devel libxkbcommon-devel wayland-devel libX11-devel libXcursor-devel \
  libXrandr-devel libXi-devel mesa-vulkan-drivers vulkan-loader
```

**Arch**

```bash
sudo pacman -S --needed base-devel cmake libxcb libxkbcommon wayland libx11 \
  libxcursor libxrandr libxi vulkan-icd-loader
```

Then install Rust:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

TLS uses rustls with the aws-lc-rs backend, so OpenSSL isn't required. If the
aws-lc-sys build complains, make sure `cmake` and a C compiler are installed.

## Build, test, run

```bash
cargo run -p tradeterm --release                  # the app
cargo run -p tradeterm --release -- --synthetic   # start with 1,000,000 synthetic bars
cargo build                                       # debug build (dependencies at opt-level 2)
cargo test                                        # unit tests
cargo clippy --all-targets -- -D warnings         # lints
cargo run --release -p tt-chart --example bench_draw     # headless chart benchmark
cargo run -p tt-brokers --example stream -- ETHUSDT 5m 20  # live API smoke test
```

The release profile uses thin LTO with `codegen-units = 1`. Logging is controlled by
`RUST_LOG`, e.g. `RUST_LOG=debug,wgpu=warn`.

Settings are stored at:

- Windows: `%APPDATA%\tradeterm\config\settings.json`
- macOS: `~/Library/Application Support/tradeterm/settings.json`
- Linux: `~/.config/tradeterm/settings.json`

Delete the file to reset to defaults. Set `TRADETERM_SETTINGS=/path/to/settings.json` to
use a different file (portable installs, test runs).

### Window chrome notes

The window is created without OS decorations. On Windows a maximized borderless
window extends about 8 px past each screen edge, so the app insets its content by
that amount while maximized. macOS and Linux get the same Windows-style buttons on the
right (native traffic lights aren't used).
