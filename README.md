# tradeterm

A native desktop trading/charting app written in Rust. Rendering is
[egui](https://github.com/emilk/egui)/eframe on wgpu. It uses no web technology
(no Tauri, Electron or WebView).

It streams live Binance spot candles from public, key-less endpoints and draws them
with a custom candlestick widget: pan, zoom, auto-fit Y axis, a crosshair with an OHLC
readout, and a last-price line.

## Workspace layout

```
crates/
  tt-core/     Types only: Symbol, Timeframe, Candle, PriceTicks, CandleSeries (SoA),
               MarketEvent, EventSink. No UI or network dependencies.
  tt-brokers/  MarketDataSource trait + Binance adapter (tokio, tokio-tungstenite/rustls,
               reqwest). Auto-reconnects with exponential backoff (500 ms → 30 s).
  tt-chart/    ChartView egui widget. Math (viewport.rs, scale.rs) is separate from
               drawing (paint.rs), so the painter can later be swapped for a custom
               wgpu renderer.
  tradeterm/   The eframe binary: top bar, chart, status bar, dark theme (theme.rs).
```

### Data flow

```
tokio thread                                   UI thread (eframe)
BinanceSource::run ──EventSink──► crossbeam channel ──► drain once per frame
   │                   │                                 └─► CandleSeries ─► ChartView
   │                   └─ wake() = ctx.request_repaint()
   ├─ 1. connect WebSocket  <symbol>@kline_<interval>
   ├─ 2. GET /api/v3/klines (1000 bars) → History
   └─ 3. live kline frames → CandleUpdate (on reconnect: back to 1)
```

The WebSocket connects before the REST call runs, so no bars are lost while history
downloads. Live bars that are older than the newest bar are ignored by `CandleSeries::upsert`.
The UI thread never blocks on network I/O, and it repaints only on input or when an
adapter sends an event.

## Setup

You need a recent stable Rust toolchain from <https://rustup.rs>. This project was built
with Rust 1.99 on edition 2024.

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

Install a C toolchain, the X11/Wayland/keyboard development headers that winit and eframe
need, and a Vulkan driver for wgpu (wgpu falls back to OpenGL ES if Vulkan isn't available).

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

TLS uses rustls with the aws-lc-rs backend, so OpenSSL isn't required. If the aws-lc-sys
build complains, make sure `cmake` and a C compiler are installed.

## Build, test, run

```bash
cargo run -p tradeterm --release   # the app
cargo build                         # debug build (dependencies built at opt-level 2)
cargo test                          # unit tests
cargo clippy -- -D warnings         # lints
```

The release profile uses thin LTO with `codegen-units = 1`. In dev builds, all
dependencies compile at `opt-level = 2` so egui/wgpu stay fast while your own crates
still build quickly.

To print raw events without the GUI (useful for checking network access):

```bash
cargo run -p tt-brokers --example stream -- ETHUSDT 5m 20
```

Logging is controlled by `RUST_LOG`, e.g. `RUST_LOG=debug,wgpu=warn cargo run -p tradeterm`.

## Using the app

| Action | Effect |
|---|---|
| Type a symbol (e.g. `ETHUSDT`) and press Enter | Switch instrument |
| Click `1m` … `1D` | Switch timeframe |
| Drag | Pan through history |
| Mouse wheel / pinch / Ctrl+wheel | Zoom around the cursor |
| Horizontal wheel / trackpad swipe | Pan |
| Double-click | Reset the view to the latest bar |

The status bar shows the connection state (with the error reason while it's retrying),
the last price and the number of bars. All times are UTC.
