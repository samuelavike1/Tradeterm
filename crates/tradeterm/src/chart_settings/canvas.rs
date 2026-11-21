//! The Canvas page: background, grid, separators, crosshair, watermark,
//! scales text, buttons and margins.

use super::*;
use tt_chart::style::{BackgroundKind, ButtonsVisibility, GridLines, resolve};

/// Scale text sizes (integers only, ui-guidelines C4).
const TEXT_SIZES: [f32; 7] = [10.0, 11.0, 12.0, 13.0, 14.0, 16.0, 18.0];

impl ChartSettingsPanel {
    pub(super) fn canvas_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Rows {
        let cv = self.draft.canvas;
        let mut rows = vec![section("Chart basic styles")];

        let kind = self
            .select(
                "cv.bg_kind",
                BackgroundKind::ALL.iter().map(|v| (*v, v.label())),
                cv.background_kind,
                |s, v| s.canvas.background_kind = v,
                cx,
            )
            .width(W_SELECT_SM);
        let bg = color!(self, window, cx, "cv.bg", canvas.background, |_, t| t.bg);
        let mut row = Self::row("Background").control(bg);
        if cv.background_kind == BackgroundKind::Gradient {
            let bottom = color!(
                self,
                window,
                cx,
                "cv.bg_bottom",
                canvas.background_bottom,
                |s, t| resolve(s.canvas.background, t.bg)
            );
            row = row.control(bottom);
        }
        let row = row.control(kind);
        rows.push(row.into_any_element());

        let grid = self.select(
            "cv.grid",
            GridLines::ALL.iter().map(|v| (*v, v.label())),
            cv.grid_lines,
            |s, v| s.canvas.grid_lines = v,
            cx,
        );
        rows.push(Self::row("Grid lines").control(grid).into_any_element());
        if cv.grid_lines.vertical() {
            let color = color!(
                self,
                window,
                cx,
                "cv.grid_v",
                canvas.grid_v_color,
                |_, t| t.grid
            );
            let kind = self.line_kind(
                "cv.grid_v_kind",
                cv.grid_v_kind,
                |s, v| s.canvas.grid_v_kind = v,
                cx,
            );
            rows.push(
                Self::row("Vert grid lines")
                    .indent()
                    .control(color)
                    .control(kind)
                    .into_any_element(),
            );
        }
        if cv.grid_lines.horizontal() {
            let color = color!(
                self,
                window,
                cx,
                "cv.grid_h",
                canvas.grid_h_color,
                |_, t| t.grid
            );
            let kind = self.line_kind(
                "cv.grid_h_kind",
                cv.grid_h_kind,
                |s, v| s.canvas.grid_h_kind = v,
                cx,
            );
            rows.push(
                Self::row("Horz grid lines")
                    .indent()
                    .control(color)
                    .control(kind)
                    .into_any_element(),
            );
        }
        let sep = color!(
            self,
            window,
            cx,
            "cv.separators",
            canvas.separators,
            |_, t| t.border
        );
        rows.push(Self::row("Pane separators").control(sep).into_any_element());

        let color = color!(
            self,
            window,
            cx,
            "cv.crosshair",
            canvas.crosshair,
            |_, t| t.crosshair
        );
        let width = self.width(
            "cv.crosshair_w",
            cv.crosshair_width,
            |s, v| s.canvas.crosshair_width = v,
            cx,
        );
        let kind = self.line_kind(
            "cv.crosshair_kind",
            cv.crosshair_kind,
            |s, v| s.canvas.crosshair_kind = v,
            cx,
        );
        rows.push(
            Self::row("Crosshair")
                .control(color)
                .control(width)
                .control(kind)
                .into_any_element(),
        );

        let wm = color!(
            self,
            window,
            cx,
            "cv.watermark",
            canvas.watermark.color,
            |_, t| { alpha(t.text, 0.06) }
        );
        rows.push(
            check!(self, cx, "cv.wm", "Watermark", canvas.watermark.visible)
                .control(wm)
                .into_any_element(),
        );
        if cv.watermark.visible {
            rows.push(
                check!(
                    self,
                    cx,
                    "cv.wm_ticker",
                    "Ticker and interval",
                    canvas.watermark.ticker_interval
                )
                .indent()
                .into_any_element(),
            );
            rows.push(
                check!(
                    self,
                    cx,
                    "cv.wm_desc",
                    "Description",
                    canvas.watermark.description
                )
                .indent()
                .into_any_element(),
            );
        }

        rows.push(section("Scales"));
        let size = self.draft.scales.text_size;
        let mut sizes: Vec<f32> = TEXT_SIZES.to_vec();
        if !sizes.contains(&size) {
            // A size from an older version (12.5): keep it selectable.
            sizes.push(size);
            sizes.sort_by(f32::total_cmp);
        }
        let text = color!(
            self,
            window,
            cx,
            "cv.scale_text",
            canvas.scale_text,
            |_, t| t.text_muted
        );
        let size_sel = self
            .select(
                "cv.text_size",
                sizes.into_iter().map(|v| (v, format!("{v}"))),
                size,
                |s, v| s.scales.text_size = v,
                cx,
            )
            .width(W_SELECT_XS);
        rows.push(
            Self::row("Text")
                .control(text)
                .control(size_sel)
                .into_any_element(),
        );
        let lines = color!(
            self,
            window,
            cx,
            "cv.scale_lines",
            canvas.scale_lines,
            |_, t| t.border
        );
        rows.push(Self::row("Lines").control(lines).into_any_element());

        rows.push(section("Buttons"));
        let nav = self
            .select(
                "cv.nav_buttons",
                ButtonsVisibility::ALL.iter().map(|v| (*v, v.label())),
                cv.nav_buttons,
                |s, v| s.canvas.nav_buttons = v,
                cx,
            )
            .width(W_SELECT_WIDE);
        rows.push(Self::row("Navigation").control(nav).into_any_element());
        let pane = self
            .select(
                "cv.pane_buttons",
                ButtonsVisibility::ALL.iter().map(|v| (*v, v.label())),
                cv.pane_buttons,
                |s, v| s.canvas.pane_buttons = v,
                cx,
            )
            .width(W_SELECT_WIDE);
        rows.push(Self::row("Pane").control(pane).into_any_element());

        rows.push(section("Margins"));
        let top = self.number(
            "cv.margin_top",
            |s| s.canvas.margins.top as f64,
            |s, v| s.canvas.margins.top = v as f32,
            0.0,
            40.0,
            1.0,
            window,
            cx,
        );
        rows.push(Self::row("Top").control(top).unit("%").into_any_element());
        let bottom = self.number(
            "cv.margin_bottom",
            |s| s.canvas.margins.bottom as f64,
            |s, v| s.canvas.margins.bottom = v as f32,
            0.0,
            40.0,
            1.0,
            window,
            cx,
        );
        rows.push(
            Self::row("Bottom")
                .control(bottom)
                .unit("%")
                .into_any_element(),
        );
        let right = self.number(
            "cv.margin_right",
            |s| s.canvas.margins.right as f64,
            |s, v| s.canvas.margins.right = v as f32,
            0.0,
            100.0,
            1.0,
            window,
            cx,
        );
        rows.push(
            Self::row("Right")
                .control(right)
                .unit("bars")
                .into_any_element(),
        );
        rows
    }
}
