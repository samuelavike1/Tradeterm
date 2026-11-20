use crate::paint::{self, FrameData};
use crate::scale::price_decimals;
use crate::viewport::{PlotRect, Viewport, padded_price_range, visible_range};
use crate::{ChartStyle, ChartType, ScaleMode};
use egui::{CursorIcon, Rect, Response, Sense, Ui, pos2};
use tt_core::CandleSeries;
use tt_theme::Theme;

const DEFAULT_SPACING: f32 = 8.0;
const DEFAULT_RIGHT_OFFSET: f32 = 5.0;
const MIN_SPACING: f32 = 1.0;
const MAX_SPACING: f32 = 80.0;
/// Fraction of the visible price span added above and below.
const Y_PADDING: f64 = 0.08;

/// Interactive candlestick chart.
///
/// Holds only view state (zoom and scroll position); the data is borrowed
/// each frame so the caller stays the single owner of the series.
///
/// - drag: pan · wheel / pinch: zoom around the pointer · horizontal wheel: pan
/// - double-click: reset view
#[derive(Debug, Clone)]
pub struct ChartView {
    pub style: ChartStyle,
    pub chart_type: ChartType,
    pub scale_mode: ScaleMode,
    pub show_volume: bool,
    bar_spacing: f32,
    right_offset: f32,
}

impl Default for ChartView {
    fn default() -> Self {
        Self::new(ChartStyle::default())
    }
}

impl ChartView {
    pub fn new(style: ChartStyle) -> Self {
        Self {
            style,
            chart_type: ChartType::default(),
            scale_mode: ScaleMode::default(),
            show_volume: true,
            bar_spacing: DEFAULT_SPACING,
            right_offset: DEFAULT_RIGHT_OFFSET,
        }
    }

    /// Returns to the default zoom, scrolled to the latest bar.
    pub fn reset_view(&mut self) {
        self.bar_spacing = DEFAULT_SPACING;
        self.right_offset = DEFAULT_RIGHT_OFFSET;
    }

    /// Draws the chart filling all available space. `title` is shown top-left.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        series: &CandleSeries,
        theme: &Theme,
        title: &str,
    ) -> Response {
        let (full, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());

        let decimals = series.last().map_or(2, |c| price_decimals(c.close));
        let plot = plot_rect(full, &self.style, series, decimals);

        self.handle_input(ui, &response, plot, series.len());

        let n = series.len();
        let range = visible_range(n, self.bar_spacing, self.right_offset, plot.width());
        let (lo, hi) = series
            .price_range(range)
            .map_or((0.0, 1.0), |(lo, hi)| padded_price_range(lo, hi, Y_PADDING));

        let vp = Viewport {
            plot: PlotRect {
                left: plot.left(),
                top: plot.top(),
                right: plot.right(),
                bottom: plot.bottom(),
            },
            bar_count: n,
            bar_spacing: self.bar_spacing,
            right_offset: self.right_offset,
            price_lo: lo,
            price_hi: hi,
        };

        let crosshair = response
            .hover_pos()
            .filter(|p| plot.contains(*p) && !response.dragged());

        let frame = FrameData {
            vp,
            series,
            style: &self.style,
            theme,
            full,
            plot,
            crosshair,
            decimals,
            bar_interval_ms: bar_interval_ms(series),
            title,
        };
        paint::paint(&ui.painter_at(full), &frame);

        if response.dragged() {
            response.on_hover_cursor(CursorIcon::Grabbing)
        } else {
            response.on_hover_cursor(CursorIcon::Crosshair)
        }
    }

    fn handle_input(&mut self, ui: &Ui, response: &Response, plot: Rect, n: usize) {
        if response.double_clicked() {
            self.reset_view();
        }

        if response.dragged() {
            // Dragging right reveals older bars.
            self.right_offset -= response.drag_delta().x / self.bar_spacing;
        }

        if response.hovered() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta(), i.zoom_delta()));
            if scroll.x != 0.0 {
                self.right_offset += scroll.x / self.bar_spacing;
            }
            let factor = zoom * (scroll.y * 0.002).exp();
            if factor != 1.0 {
                let anchor_x = response.hover_pos().map_or(plot.right(), |p| p.x);
                self.zoom_at(factor, anchor_x, plot.right());
            }
        }

        // Keep at least a few bars on screen.
        let visible_bars = plot.width() / self.bar_spacing;
        let max_offset = (visible_bars - 3.0).max(0.0);
        let min_offset = -((n as f32) - 3.0).max(0.0);
        self.right_offset = self.right_offset.clamp(min_offset, max_offset);
    }

    /// Zooms by `factor`, keeping the bar under `anchor_x` fixed on screen.
    fn zoom_at(&mut self, factor: f32, anchor_x: f32, plot_right: f32) {
        let new_spacing = (self.bar_spacing * factor).clamp(MIN_SPACING, MAX_SPACING);
        let dist = plot_right - anchor_x;
        let bars_before = dist / self.bar_spacing;
        let bars_after = dist / new_spacing;
        self.right_offset += bars_after - bars_before;
        self.bar_spacing = new_spacing;
    }
}

/// Splits the widget into plot area and axis strips.
fn plot_rect(full: Rect, style: &ChartStyle, series: &CandleSeries, decimals: usize) -> Rect {
    // Estimate the widest price label from the largest price; monospace digits
    // are ~0.62em wide, so this avoids measuring text before painting.
    let max_price = series.highs().iter().copied().fold(0.0_f64, f64::max);
    let int_digits = (max_price.abs().max(1.0).log10().floor() as usize) + 1;
    let chars = int_digits + decimals + 1;
    let axis_w = (chars as f32 * style.font_size * 0.62 + 22.0).max(64.0);
    Rect::from_min_max(
        full.min,
        pos2(
            full.right() - axis_w,
            full.bottom() - style.time_axis_height,
        ),
    )
}

/// Bar interval inferred from the data (smallest gap among the last bars).
fn bar_interval_ms(series: &CandleSeries) -> i64 {
    let t = series.open_times();
    let tail = &t[t.len().saturating_sub(50)..];
    tail.windows(2)
        .map(|w| w[1] - w[0])
        .filter(|d| *d > 0)
        .min()
        .unwrap_or(60_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_keeps_anchor_bar_fixed() {
        let mut v = ChartView::default();
        let right = 1000.0;
        let anchor = 600.0;
        // Continuous index under the anchor: n-1+off+0.5 - (right-x)/spacing, n cancels out.
        let idx = |v: &ChartView| v.right_offset - (right - anchor) / v.bar_spacing;
        let before = idx(&v);
        v.zoom_at(1.7, anchor, right);
        assert!((idx(&v) - before).abs() < 1e-4);
        v.zoom_at(1000.0, anchor, right);
        assert_eq!(v.bar_spacing, MAX_SPACING);
        assert!((idx(&v) - before).abs() < 1e-3);
    }
}
