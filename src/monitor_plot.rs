use super::*;
use plotters::prelude::*;
use plotters_backend::{
    text_anchor::{HPos, VPos},
    BackendColor, BackendCoord, BackendStyle, BackendTextStyle, DrawingBackend, DrawingErrorKind,
};
use std::convert::Infallible;

type Result<T> = std::result::Result<T, DrawingErrorKind<Infallible>>;
struct Backend<'a> {
    renderer: &'a Renderer,
    x: f32,
    y: f32,
    width: u32,
    height: u32,
}
fn ink(c: BackendColor) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: c.rgb.0 as f32 / 255.,
        g: c.rgb.1 as f32 / 255.,
        b: c.rgb.2 as f32 / 255.,
        a: c.alpha as f32,
    }
}
impl Backend<'_> {
    fn point(&self, p: BackendCoord) -> D2D1_POINT_2F {
        D2D1_POINT_2F {
            x: self.x + p.0 as f32,
            y: self.y + p.1 as f32,
        }
    }
    fn geometry(
        &self,
        points: impl IntoIterator<Item = BackendCoord>,
        c: BackendColor,
        width: u32,
        fill: bool,
    ) {
        unsafe {
            let mut points = points.into_iter();
            let Some(first) = points.next() else { return };
            let mut path = Com::empty();
            if (*self.renderer.factory.0).CreatePathGeometry(path.out()) < 0 {
                return;
            }
            let mut sink = Com::empty();
            if (*path.0).Open(sink.out()) < 0 {
                return;
            }
            (*sink.0).BeginFigure(
                self.point(first),
                if fill {
                    D2D1_FIGURE_BEGIN_FILLED
                } else {
                    D2D1_FIGURE_BEGIN_HOLLOW
                },
            );
            for p in points {
                (*sink.0).AddLine(self.point(p));
            }
            (*sink.0).EndFigure(if fill {
                D2D1_FIGURE_END_CLOSED
            } else {
                D2D1_FIGURE_END_OPEN
            });
            if (*sink.0).Close() < 0 {
                return;
            }
            (*self.renderer.brush.0).SetColor(&ink(c));
            if fill {
                (*self.renderer.target.0).FillGeometry(
                    path.0 as _,
                    self.renderer.brush.0 as _,
                    null_mut(),
                );
            } else {
                (*self.renderer.target.0).DrawGeometry(
                    path.0 as _,
                    self.renderer.brush.0 as _,
                    width as f32,
                    null_mut(),
                );
            }
        }
    }
    fn text_size(&self, text: &str) -> (u32, u32) {
        if let Some(size) = self.renderer.text_sizes.borrow().get(text) {
            return *size;
        }
        unsafe {
            let utf16 = wide(text);
            let mut layout = Com::empty();
            let mut metrics: DWRITE_TEXT_METRICS = std::mem::zeroed();
            let size = if (*self.renderer._dw.0).CreateTextLayout(
                utf16.as_ptr(),
                (utf16.len() - 1) as u32,
                self.renderer.small.0,
                1000.,
                100.,
                layout.out(),
            ) >= 0
                && (*layout.0).GetMetrics(&mut metrics) >= 0
            {
                (
                    metrics.widthIncludingTrailingWhitespace.ceil() as u32,
                    metrics.height.ceil() as u32,
                )
            } else {
                (text.chars().count() as u32 * 7, 15)
            };
            self.renderer
                .text_sizes
                .borrow_mut()
                .insert(text.to_owned(), size);
            size
        }
    }
}
impl DrawingBackend for Backend<'_> {
    type ErrorType = Infallible;
    fn get_size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    fn ensure_prepared(&mut self) -> Result<()> {
        Ok(())
    }
    fn present(&mut self) -> Result<()> {
        Ok(())
    }
    fn draw_pixel(&mut self, p: BackendCoord, c: BackendColor) -> Result<()> {
        unsafe {
            self.renderer
                .rect(self.x + p.0 as f32, self.y + p.1 as f32, 1., 1., 0., ink(c));
        }
        Ok(())
    }
    fn draw_line<S: BackendStyle>(
        &mut self,
        a: BackendCoord,
        b: BackendCoord,
        s: &S,
    ) -> Result<()> {
        unsafe {
            self.renderer.line(
                (self.x + a.0 as f32, self.y + a.1 as f32),
                (self.x + b.0 as f32, self.y + b.1 as f32),
                s.stroke_width() as f32,
                ink(s.color()),
            );
        }
        Ok(())
    }
    fn draw_rect<S: BackendStyle>(
        &mut self,
        a: BackendCoord,
        b: BackendCoord,
        s: &S,
        fill: bool,
    ) -> Result<()> {
        if fill {
            unsafe {
                self.renderer.rect(
                    self.x + a.0 as f32,
                    self.y + a.1 as f32,
                    (b.0 - a.0) as f32,
                    (b.1 - a.1) as f32,
                    0.,
                    ink(s.color()),
                );
            }
        } else {
            self.geometry(
                [a, (b.0, a.1), b, (a.0, b.1), a],
                s.color(),
                s.stroke_width(),
                false,
            );
        }
        Ok(())
    }
    fn draw_path<S: BackendStyle, I: IntoIterator<Item = BackendCoord>>(
        &mut self,
        p: I,
        s: &S,
    ) -> Result<()> {
        self.geometry(p, s.color(), s.stroke_width(), false);
        Ok(())
    }
    fn fill_polygon<S: BackendStyle, I: IntoIterator<Item = BackendCoord>>(
        &mut self,
        p: I,
        s: &S,
    ) -> Result<()> {
        self.geometry(p, s.color(), 0, true);
        Ok(())
    }
    fn estimate_text_size<T: BackendTextStyle>(&self, text: &str, _: &T) -> Result<(u32, u32)> {
        Ok(self.text_size(text))
    }
    fn draw_text<T: BackendTextStyle>(
        &mut self,
        text: &str,
        style: &T,
        p: BackendCoord,
    ) -> Result<()> {
        let (w, h) = self.text_size(text);
        let x = p.0
            - match style.anchor().h_pos {
                HPos::Left => 0,
                HPos::Center => w as i32 / 2,
                HPos::Right => w as i32,
            };
        let y = p.1
            - match style.anchor().v_pos {
                VPos::Top => 0,
                VPos::Center => h as i32 / 2,
                VPos::Bottom => h as i32,
            };
        unsafe {
            self.renderer.text(
                self.x + x as f32,
                self.y + y as f32,
                w as f32 + 2.,
                h as f32 + 2.,
                text,
                0,
                ink(style.color()),
            );
        }
        Ok(())
    }
}

pub(super) unsafe fn wave(
    r: &Renderer,
    bins: &[Bucket],
    lane: usize,
    seconds: u32,
    stages: [bool; 2],
    bounds: (f32, f32, f32, f32),
    time_labels: bool,
    phase: f32,
) {
    let (x, y, w, h) = bounds;
    if w < 60. || h < 22. {
        return;
    }
    let area = Backend {
        renderer: r,
        x,
        y,
        width: w as u32,
        height: h as u32,
    }
    .into_drawing_area();
    let Ok(mut chart) = ChartBuilder::on(&area)
        .margin_top(9)
        .margin_bottom(9)
        .margin_right(14)
        .x_label_area_size(18)
        .y_label_area_size(38)
        .build_cartesian_2d(-(seconds as f32)..0f32, -100f32..100f32)
    else {
        return;
    };
    let _ = chart
        .configure_mesh()
        .disable_x_mesh()
        .x_labels(if time_labels { 4 } else { 0 })
        .y_labels(3)
        .max_light_lines(0)
        .axis_style(RGBColor(213, 220, 230))
        .bold_line_style(RGBColor(236, 240, 245))
        .label_style(
            ("sans-serif", 11)
                .into_font()
                .color(&RGBColor(117, 129, 146)),
        )
        .x_label_formatter(&|v| {
            if v.abs() < 0.01 {
                "0s".into()
            } else {
                format!("{v:.0}s")
            }
        })
        .y_label_formatter(&|v| format!("{v:.0}%"))
        .draw();
    for (stage, enabled) in stages.into_iter().enumerate() {
        if !enabled {
            continue;
        }
        let bit = 1 << (lane + stage * 5);
        let color = if stage == 0 {
            RGBColor(46, 115, 235)
        } else {
            RGBColor(229, 137, 52)
        };
        let time = |i: usize| {
            -(seconds as f32) + (i as f32 - phase) * seconds as f32 / bins.len().max(1) as f32
        };
        let mut begin = 0;
        while begin < bins.len() {
            if bins[begin].gap & bit != 0 {
                let _ = chart.draw_series(std::iter::once(PathElement::new(
                    vec![(time(begin), -100.), (time(begin), 100.)],
                    RGBColor(224, 139, 133).mix(0.5),
                )));
            }
            if bins[begin].valid & bit == 0 || bins[begin].gap & bit != 0 {
                begin += 1;
                continue;
            }
            let mut end = begin + 1;
            while end < bins.len() && bins[end].valid & bit != 0 && bins[end].gap & bit == 0 {
                end += 1;
            }
            let top: Vec<_> = (begin..end)
                .map(|i| {
                    (
                        time(i),
                        bins[i].max[lane + stage * 5] as f32 * 100. / 32768.,
                    )
                })
                .collect();
            let bottom: Vec<_> = (begin..end)
                .map(|i| {
                    (
                        time(i),
                        bins[i].min[lane + stage * 5] as f32 * 100. / 32768.,
                    )
                })
                .collect();
            if stage == 0 {
                let envelope: Vec<_> = top.iter().chain(bottom.iter().rev()).copied().collect();
                let _ = chart.draw_series(std::iter::once(Polygon::new(
                    envelope,
                    color.mix(0.16).filled(),
                )));
                let _ = chart.draw_series(LineSeries::new(
                    top,
                    plotters::style::Color::stroke_width(&color, 1),
                ));
                let _ = chart.draw_series(LineSeries::new(
                    bottom,
                    plotters::style::Color::stroke_width(&color, 1),
                ));
            } else {
                let _ = chart.draw_series(DashedLineSeries::new(
                    top,
                    4,
                    3,
                    plotters::style::Color::stroke_width(&color, 1),
                ));
                let _ = chart.draw_series(DashedLineSeries::new(
                    bottom,
                    4,
                    3,
                    plotters::style::Color::stroke_width(&color, 1),
                ));
            }
            begin = end;
        }
    }
}
