//! The Canvas page: background, grid, separators, crosshair, watermark,
//! scales text, buttons and margins (TradingView's order).

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
            .width(W_SELECT);
        let bg = color!(self, cx, "cv.bg", canvas.background, |_, t| t.bg);
        let mut row = Self::row("Background").control(kind).control(bg);
        if cv.background_kind == BackgroundKind::Gradient {
            let bottom = color!(
                self,
                cx,
                "cv.bg_bottom",
                canvas.background_bottom,
                |s, t| resolve(s.canvas.background, t.bg)
            );
            row = row.control(bottom);
        }
        rows.push(row.into());

        // Grid: one checkbox per direction (GridLines), each with its line.
        let v_line = self.line(
            "cv.grid_v",
            |s| s.canvas.grid_v_color,
            |s, v| s.canvas.grid_v_color = v,
            |_, t| t.grid,
            None,
            Some((
                cv.grid_v_kind,
                Rc::new(|s: &mut ChartStyle, v| s.canvas.grid_v_kind = v),
            )),
            cx,
        );
        rows.push(
            self.check(
                "cv.grid_v_on",
                "Vertical grid lines",
                cv.grid_lines.vertical(),
                |s, v| {
                    s.canvas.grid_lines = GridLines::from_flags(v, s.canvas.grid_lines.horizontal())
                },
                cx,
            )
            .control(v_line)
            .into(),
        );
        let h_line = self.line(
            "cv.grid_h",
            |s| s.canvas.grid_h_color,
            |s, v| s.canvas.grid_h_color = v,
            |_, t| t.grid,
            None,
            Some((
                cv.grid_h_kind,
                Rc::new(|s: &mut ChartStyle, v| s.canvas.grid_h_kind = v),
            )),
            cx,
        );
        rows.push(
            self.check(
                "cv.grid_h_on",
                "Horizontal grid lines",
                cv.grid_lines.horizontal(),
                |s, v| s.canvas.grid_lines = GridLines::from_flags(s.canvas.grid_lines.vertical(), v),
                cx,
            )
            .control(h_line)
            .into(),
        );
        let sep = color!(self, cx, "cv.separators", canvas.separators, |_, t| t
            .border);
        rows.push(Self::row("Pane separators").control(sep).into());

        let crosshair = self.line(
            "cv.crosshair",
            |s| s.canvas.crosshair,
            |s, v| s.canvas.crosshair = v,
            |_, t| t.crosshair,
            Some((
                cv.crosshair_width,
                Rc::new(|s: &mut ChartStyle, v| s.canvas.crosshair_width = v),
            )),
            Some((
                cv.crosshair_kind,
                Rc::new(|s: &mut ChartStyle, v| s.canvas.crosshair_kind = v),
            )),
            cx,
        );
        rows.push(Self::row("Crosshair").control(crosshair).into());

        let wm = color!(self, cx, "cv.watermark", canvas.watermark.color, |_, t| {
            alpha(t.text, 0.06)
        });
        rows.push(
            check!(self, cx, "cv.wm", "Watermark", canvas.watermark.visible)
                .control(wm)
                .into(),
        );
        if cv.watermark.visible {
            rows.push(self.sub_check(
                "cv.wm_ticker",
                "Ticker and interval",
                cv.watermark.ticker_interval,
                |s, v| s.canvas.watermark.ticker_interval = v,
                cx,
            ));
            rows.push(self.sub_check(
                "cv.wm_desc",
                "Description",
                cv.watermark.description,
                |s, v| s.canvas.watermark.description = v,
                cx,
            ));
        }

        rows.push(section("Scales"));
        let size = self.draft.scales.text_size;
        let mut sizes: Vec<f32> = TEXT_SIZES.to_vec();
        if !sizes.contains(&size) {
            // A size from an older version (12.5): keep it selectable.
            sizes.push(size);
            sizes.sort_by(f32::total_cmp);
        }
        let text = color!(self, cx, "cv.scale_text", canvas.scale_text, |_, t| t
            .text_muted);
        let size_sel = self
            .select(
                "cv.text_size",
                sizes.into_iter().map(|v| (v, format!("{v}"))),
                size,
                |s, v| s.scales.text_size = v,
                cx,
            )
            .width(W_SELECT_SM);
        rows.push(Self::row("Text").control(text).control(size_sel).into());
        let lines = color!(self, cx, "cv.scale_lines", canvas.scale_lines, |_, t| t
            .border);
        rows.push(Self::row("Lines").control(lines).into());

        rows.push(section("Buttons"));
        // Navigation buttons: hidden until the chart has them (BACKLOG.md).
        let pane = self
            .select(
                "cv.pane_buttons",
                ButtonsVisibility::ALL.iter().map(|v| (*v, v.label())),
                cv.pane_buttons,
                |s, v| s.canvas.pane_buttons = v,
                cx,
            )
            .width(W_SELECT_LG);
        rows.push(Self::row("Pane").control(pane).into());

        rows.push(section("Margins"));
        let top = self.number(
            "cv.margin_top",
            |s| s.canvas.margins.top as f64,
            |s, v| s.canvas.margins.top = v as f32,
            0.0,
            40.0,
            W_FIELD_SM,
            window,
            cx,
        );
        rows.push(Self::row("Top").control(top).unit("%").into());
        let bottom = self.number(
            "cv.margin_bottom",
            |s| s.canvas.margins.bottom as f64,
            |s, v| s.canvas.margins.bottom = v as f32,
            0.0,
            40.0,
            W_FIELD_SM,
            window,
            cx,
        );
        rows.push(Self::row("Bottom").control(bottom).unit("%").into());
        let right = self.number(
            "cv.margin_right",
            |s| s.canvas.margins.right as f64,
            |s, v| s.canvas.margins.right = v as f32,
            0.0,
            100.0,
            W_FIELD_SM,
            window,
            cx,
        );
        rows.push(Self::row("Right").control(right).unit("bars").into());
        rows
    }
}
