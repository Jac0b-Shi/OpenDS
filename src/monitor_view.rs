use crate::{
    config::Config,
    monitor::{neutral, wave_state, Bucket, Monitor, WaveState},
};
#[path = "command_view.rs"]
mod command_view;
#[path = "monitor_plot.rs"]
mod plotting;
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    ptr::{null, null_mut},
    rc::Rc,
    sync::{atomic::Ordering::*, Arc},
    time::Instant,
};
use winapi::{
    shared::{dxgiformat::DXGI_FORMAT_UNKNOWN, minwindef::*, windef::*},
    um::{
        commdlg::{
            CommDlgExtendedError, GetSaveFileNameW, OFN_EXPLORER, OFN_NOCHANGEDIR,
            OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
        },
        d2d1::*,
        dcommon::{D2D1_ALPHA_MODE_UNKNOWN, D2D1_PIXEL_FORMAT},
        dwrite::*,
        libloaderapi::GetModuleHandleW,
        unknwnbase::IUnknown,
        wingdi::*,
        winuser::*,
    },
    Interface,
};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
unsafe fn choose_recording_path(owner: HWND) -> Result<Option<PathBuf>, String> {
    let filter = wide(&format!(
        "{}\0*.csv\0",
        crate::tr!(monitor_view_csv_files_csv_u0000_csv_u0000)
    ));
    let mut path = vec![0u16; 32768];
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let name = wide(&format!("OpenDS-monitor-{stamp}.csv"));
    path[..name.len()].copy_from_slice(&name);
    let mut dialog: OPENFILENAMEW = std::mem::zeroed();
    dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    dialog.hwndOwner = owner;
    dialog.lpstrFilter = filter.as_ptr();
    dialog.lpstrFile = path.as_mut_ptr();
    dialog.nMaxFile = path.len() as u32;
    let title = wide(crate::tr!(
        monitor_view_choose_where_to_save_the_csv_recording
    ));
    let extension = wide("csv");
    dialog.lpstrTitle = title.as_ptr();
    dialog.lpstrDefExt = extension.as_ptr();
    dialog.Flags = OFN_EXPLORER | OFN_NOCHANGEDIR | OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST;
    if GetSaveFileNameW(&mut dialog) != 0 {
        let end = path.iter().position(|c| *c == 0).unwrap_or(path.len());
        Ok(Some(PathBuf::from(String::from_utf16_lossy(&path[..end]))))
    } else {
        let error = CommDlgExtendedError();
        if error == 0 {
            Ok(None)
        } else {
            Err(crate::tr!(
                monitor_view_could_not_choose_save_location_0x,
                code = format!("{:08X}", error)
            ))
        }
    }
}
fn model_bottom(width: f32) -> f32 {
    86. + ((width - 8.).max(1.) / 440.).min(1.65) * 300.
}
struct Com<T>(*mut T);
impl<T> Com<T> {
    fn empty() -> Self {
        Self(null_mut())
    }
    fn out(&mut self) -> *mut *mut T {
        &mut self.0
    }
}
impl<T> Drop for Com<T> {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                (*(self.0 as *mut IUnknown)).Release();
            }
        }
    }
}
fn hr(v: i32) -> Result<(), String> {
    if v < 0 {
        Err(crate::tr!(
            monitor_view_drawing_failed_0x,
            code = format!("{:08X}", v as u32)
        ))
    } else {
        Ok(())
    }
}
fn color(r: f32, g: f32, b: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r, g, b, a: 1.0 }
}
fn palette_color(value: u32) -> D2D1_COLOR_F {
    color(
        (value & 255) as f32 / 255.,
        ((value >> 8) & 255) as f32 / 255.,
        ((value >> 16) & 255) as f32 / 255.,
    )
}
fn ink_color() -> D2D1_COLOR_F {
    palette_color(crate::appearance::palette().text)
}
fn muted_color() -> D2D1_COLOR_F {
    palette_color(crate::appearance::palette().muted)
}
fn blue_color() -> D2D1_COLOR_F {
    palette_color(crate::appearance::palette().source)
}
fn orange_color() -> D2D1_COLOR_F {
    palette_color(crate::appearance::palette().forwarded)
}

struct Renderer {
    factory: Com<ID2D1Factory>,
    target: Com<ID2D1RenderTarget>,
    _dw: Com<IDWriteFactory>,
    brush: Com<ID2D1SolidColorBrush>,
    body: Com<IDWriteTextFormat>,
    small: Com<IDWriteTextFormat>,
    title: Com<IDWriteTextFormat>,
    paths: RefCell<Vec<((u32, u32), Com<ID2D1PathGeometry>)>>,
    text_sizes: RefCell<std::collections::HashMap<String, (u32, u32)>>,
    layouts: RefCell<std::collections::HashMap<(String, u8, u32, u32), Com<IDWriteTextLayout>>>,
}
impl Renderer {
    unsafe fn new(hwnd: HWND) -> Result<Self, String> {
        let mut factory: Com<ID2D1Factory> = Com::empty();
        hr(D2D1CreateFactory(
            D2D1_FACTORY_TYPE_SINGLE_THREADED,
            &ID2D1Factory::uuidof(),
            null(),
            factory.out() as _,
        ))?;
        let mut dw: Com<IDWriteFactory> = Com::empty();
        hr(DWriteCreateFactory(
            DWRITE_FACTORY_TYPE_SHARED,
            &IDWriteFactory::uuidof(),
            dw.out() as _,
        ))?;
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut r);
        let dpi = crate::gui_theme::window_dpi(hwnd).max(96) as f32;
        let props = D2D1_RENDER_TARGET_PROPERTIES {
            _type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_UNKNOWN,
                alphaMode: D2D1_ALPHA_MODE_UNKNOWN,
            },
            dpiX: dpi,
            dpiY: dpi,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let hp = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd,
            pixelSize: D2D1_SIZE_U {
                width: r.right.max(1) as u32,
                height: r.bottom.max(1) as u32,
            },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };
        let mut target: Com<ID2D1HwndRenderTarget> = Com::empty();
        hr((*factory.0).CreateHwndRenderTarget(&props, &hp, target.out()))?;
        let ptr = target.0;
        target.0 = null_mut();
        let target = Com(ptr as *mut ID2D1RenderTarget);
        Self::from_target(factory, dw, target)
    }
    unsafe fn from_target(
        factory: Com<ID2D1Factory>,
        dw: Com<IDWriteFactory>,
        target: Com<ID2D1RenderTarget>,
    ) -> Result<Self, String> {
        let mut brush = Com::empty();
        hr((*target.0).CreateSolidColorBrush(&ink_color(), null(), brush.out()))?;
        let make = |size| -> Result<Com<IDWriteTextFormat>, String> {
            let mut f = Com::empty();
            hr((*dw.0).CreateTextFormat(
                wide(crate::gui_theme::font_family()).as_ptr(),
                null_mut(),
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                wide(&crate::i18n::current().code).as_ptr(),
                f.out(),
            ))?;
            Ok(f)
        };
        let body = make(13.0)?;
        let small = make(11.0)?;
        let title = make(16.0)?;
        Ok(Self {
            factory,
            target,
            _dw: dw,
            brush,
            body,
            small,
            title,
            paths: RefCell::new(Vec::new()),
            text_sizes: RefCell::new(std::collections::HashMap::new()),
            layouts: RefCell::new(std::collections::HashMap::new()),
        })
    }
    unsafe fn text(&self, x: f32, y: f32, w: f32, h: f32, s: &str, size: u8, c: D2D1_COLOR_F) {
        if s.is_empty() || w <= 0. || h <= 0. {
            return;
        }
        (*self.brush.0).SetColor(&c);
        let key = (s.to_owned(), size, w.to_bits(), h.to_bits());
        if let Some(layout) = self.layouts.borrow().get(&key) {
            (*self.target.0).DrawTextLayout(
                D2D1_POINT_2F { x, y },
                layout.0,
                self.brush.0 as _,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
            );
            return;
        }
        let t = wide(s);
        let f = match size {
            0 => self.small.0,
            2 => self.title.0,
            _ => self.body.0,
        };
        let mut layout = Com::empty();
        if (*self._dw.0).CreateTextLayout(t.as_ptr(), (t.len() - 1) as u32, f, w, h, layout.out())
            < 0
        {
            return;
        }
        let mut font_size = (*f).GetFontSize();
        for _ in 0..4 {
            let mut metrics: DWRITE_TEXT_METRICS = std::mem::zeroed();
            if (*layout.0).GetMetrics(&mut metrics) < 0
                || metrics.height <= h + 0.5
                || font_size <= 8.
            {
                break;
            }
            font_size = (font_size * (h / metrics.height).min(0.9)).max(8.);
            (*layout.0).SetFontSize(
                font_size,
                DWRITE_TEXT_RANGE {
                    startPosition: 0,
                    length: (t.len() - 1) as u32,
                },
            );
        }
        (*self.target.0).DrawTextLayout(
            D2D1_POINT_2F { x, y },
            layout.0,
            self.brush.0 as _,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
        );
        let mut cache = self.layouts.borrow_mut();
        if cache.len() >= 256 {
            cache.clear();
        }
        cache.insert(key, layout);
    }
    unsafe fn rect(&self, x: f32, y: f32, w: f32, h: f32, r: f32, c: D2D1_COLOR_F) {
        (*self.brush.0).SetColor(&c);
        (*self.target.0).FillRoundedRectangle(
            &D2D1_ROUNDED_RECT {
                rect: D2D1_RECT_F {
                    left: x,
                    top: y,
                    right: x + w,
                    bottom: y + h,
                },
                radiusX: r,
                radiusY: r,
            },
            self.brush.0 as _,
        );
    }
    unsafe fn battery(&self, x: f32, y: f32, view: crate::battery::View) {
        let p = crate::appearance::palette();
        let color = palette_color(crate::battery_ui::color(view));
        self.rect(
            x,
            y + 5.,
            35.,
            19.,
            3.,
            if view.percent.is_some() || matches!(view.status, crate::battery::Status::Fault(_)) {
                color
            } else {
                muted_color()
            },
        );
        self.rect(x + 1., y + 6., 33., 17., 2., palette_color(p.surface));
        self.rect(x + 35., y + 11., 3., 7., 1., muted_color());
        let fill = 29. * crate::battery_ui::fill_fraction(view);
        if fill > 0. {
            self.rect(x + 3., y + 8., fill, 13., 1., color);
        }
        if view.status == crate::battery::Status::Charging {
            let ink = palette_color(if p.kind == crate::appearance::Kind::Contrast {
                p.surface
            } else {
                p.text
            });
            for pair in crate::battery_ui::BOLT.windows(2) {
                self.line(
                    (x + pair[0].0 * 35., y + 5. + pair[0].1 * 19.),
                    (x + pair[1].0 * 35., y + 5. + pair[1].1 * 19.),
                    1.5,
                    ink,
                );
            }
        }
        if matches!(view.status, crate::battery::Status::Fault(_)) {
            self.text(x + 11., y, 16., 28., "!", 1, ink_color());
        }
        self.text(x + 45., y, 49., 28., &view.number(), 1, ink_color());
    }
    unsafe fn line(&self, a: (f32, f32), b: (f32, f32), width: f32, c: D2D1_COLOR_F) {
        (*self.brush.0).SetColor(&c);
        (*self.target.0).DrawLine(
            D2D1_POINT_2F { x: a.0, y: a.1 },
            D2D1_POINT_2F { x: b.0, y: b.1 },
            self.brush.0 as _,
            width,
            null_mut(),
        );
    }
    unsafe fn ellipse(&self, x: f32, y: f32, r: f32, c: D2D1_COLOR_F) {
        (*self.brush.0).SetColor(&c);
        (*self.target.0).FillEllipse(
            &D2D1_ELLIPSE {
                point: D2D1_POINT_2F { x, y },
                radiusX: r,
                radiusY: r,
            },
            self.brush.0 as _,
        );
    }
    unsafe fn path(&self, origin: (f32, f32), segments: &[[f32; 6]], c: D2D1_COLOR_F) {
        let key = (origin.0.to_bits(), origin.1.to_bits());
        let mut paths = self.paths.borrow_mut();
        if let Some((_, p)) = paths.iter().find(|(k, _)| *k == key) {
            (*self.brush.0).SetColor(&c);
            (*self.target.0).FillGeometry(p.0 as _, self.brush.0 as _, null_mut());
            return;
        }
        let mut p: Com<ID2D1PathGeometry> = Com::empty();
        if (*self.factory.0).CreatePathGeometry(p.out()) < 0 {
            return;
        }
        let mut sink: Com<ID2D1GeometrySink> = Com::empty();
        if (*p.0).Open(sink.out()) < 0 {
            return;
        }
        (*sink.0).BeginFigure(
            D2D1_POINT_2F {
                x: origin.0,
                y: origin.1,
            },
            D2D1_FIGURE_BEGIN_FILLED,
        );
        for s in segments {
            (*sink.0).AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: D2D1_POINT_2F { x: s[0], y: s[1] },
                point2: D2D1_POINT_2F { x: s[2], y: s[3] },
                point3: D2D1_POINT_2F { x: s[4], y: s[5] },
            });
        }
        (*sink.0).EndFigure(D2D1_FIGURE_END_CLOSED);
        if (*sink.0).Close() >= 0 {
            (*self.brush.0).SetColor(&c);
            (*self.target.0).FillGeometry(p.0 as _, self.brush.0 as _, null_mut());
            paths.push((key, p));
        }
    }
    unsafe fn controller(
        &self,
        x: f32,
        y: f32,
        w: f32,
        s: &crate::report::ParsedState,
        press: [u8; 3],
        edge: bool,
    ) {
        let scale = (w / 440.0).min(1.65);
        let mut mat: D2D1_MATRIX_3X2_F = std::mem::zeroed();
        mat.matrix = [[scale, 0.0], [0.0, scale], [x, y]];
        (*self.target.0).SetTransform(&mat);
        self.path(
            (80., 80.),
            &[
                [45., 85., 25., 180., 42., 240.],
                [60., 282., 92., 220., 125., 200.],
                [155., 187., 285., 187., 315., 200.],
                [348., 220., 380., 282., 398., 240.],
                [415., 180., 395., 85., 360., 80.],
                [340., 45., 300., 35., 270., 44.],
                [245., 48., 195., 48., 170., 44.],
                [140., 35., 100., 45., 80., 80.],
            ],
            color(0.12, 0.15, 0.21),
        );
        self.path(
            (75., 86.),
            &[
                [74., 55., 135., 42., 169., 52.],
                [198., 58., 242., 58., 271., 52.],
                [305., 42., 366., 55., 365., 86.],
                [389., 114., 400., 207., 384., 228.],
                [375., 242., 347., 195., 325., 180.],
                [285., 159., 155., 159., 115., 180.],
                [93., 195., 65., 242., 56., 228.],
                [40., 207., 51., 114., 75., 86.],
            ],
            color(0.89, 0.92, 0.96),
        );
        self.rect(151., 64., 138., 73., 12., color(0.14, 0.17, 0.23));
        self.rect(
            157.,
            69.,
            126.,
            58.,
            9.,
            if s.buttons2 & 2 != 0 {
                blue_color()
            } else {
                color(0.21, 0.25, 0.32)
            },
        );
        self.line((155., 63.), (285., 63.), 3., color(0.23, 0.56, 1.0));
        for t in [s.touch1, s.touch2] {
            if t.active {
                self.ellipse(
                    160. + t.x.min(1919) as f32 / 1919. * 120.,
                    72. + t.y.min(1079) as f32 / 1079. * 50.,
                    5.,
                    color(0.43, 0.84, 1.0),
                );
            }
        }
        let b0 = s.buttons0 | press[0];
        let b1 = s.buttons1 | press[1];
        let b2 = s.buttons2 | press[2];
        let button = |x: f32, y: f32, label: &str, on: bool, r: f32| {
            self.ellipse(x, y + 2., r, color(0.06, 0.08, 0.12));
            self.ellipse(
                x,
                y,
                r,
                if on {
                    blue_color()
                } else {
                    color(0.20, 0.24, 0.30)
                },
            );
            self.text(
                x - r + 5.,
                y - 10.,
                r * 2.,
                24.,
                label,
                1,
                color(0.96, 0.97, 1.),
            );
        };
        button(325., 91., "△", b0 & 128 != 0, 15.);
        button(348., 115., "○", b0 & 64 != 0, 15.);
        button(325., 139., "×", b0 & 32 != 0, 15.);
        button(302., 115., "□", b0 & 16 != 0, 15.);
        let d = crate::monitor::dpad_bits(s.buttons0) | (press[0] & 15);
        let up = d & 1 != 0;
        let right = d & 2 != 0;
        let down = d & 4 != 0;
        let left = d & 8 != 0;
        for (xx, yy, label, on) in [
            (100., 91., "↑", up),
            (123., 115., "→", right),
            (100., 139., "↓", down),
            (77., 115., "←", left),
        ] {
            button(xx, yy, label, on, 13.);
        }
        for (xx, lx, ly, mask) in [(165., s.lx, s.ly, 64), (275., s.rx, s.ry, 128)] {
            self.ellipse(xx, 170., 26., color(0.08, 0.10, 0.15));
            self.ellipse(xx, 170., 23., color(0.23, 0.27, 0.34));
            self.ellipse(
                xx + (lx as f32 - 128.) / 128. * 12.,
                170. + (ly as f32 - 128.) / 128. * 12.,
                17.,
                if b1 & mask != 0 {
                    blue_color()
                } else {
                    color(0.12, 0.15, 0.21)
                },
            );
        }
        button(220., 171., "PS", b2 & 1 != 0, 13.);
        self.rect(
            209.,
            196.,
            22.,
            7.,
            3.,
            if b2 & 4 != 0 {
                orange_color()
            } else {
                color(0.32, 0.38, 0.45)
            },
        );
        self.text(
            202.,
            207.,
            50.,
            20.,
            crate::tr!(monitor_view_muted),
            0,
            muted_color(),
        );
        button(137., 83., "C", b1 & 16 != 0, 9.);
        button(301., 75., "≡", b1 & 32 != 0, 9.);
        for (xx, label, val, mask) in [(66., "L", s.l2, 1u8), (301., "R", s.r2, 2u8)] {
            self.rect(
                xx,
                20.,
                72.,
                15.,
                5.,
                if b1 & mask != 0 {
                    blue_color()
                } else {
                    color(0.21, 0.25, 0.32)
                },
            );
            self.text(
                xx + 21.,
                18.,
                50.,
                20.,
                &format!("{label}1"),
                0,
                color(1., 1., 1.),
            );
            self.rect(xx, 0., 72., 13., 4., color(0.8, 0.83, 0.88));
            self.rect(xx, 0., 72. * val as f32 / 255., 13., 4., blue_color());
            (*self.target.0).SetTransform(&identity());
            self.text(
                x + xx * scale,
                y - 21. * scale,
                80. * scale,
                20.,
                &format!("{label}2 {:3}%", val as u32 * 100 / 255),
                0,
                muted_color(),
            );
            (*self.target.0).SetTransform(&mat);
        }
        if edge {
            for (i, (label, mask)) in [
                ("Fn L", 16),
                ("Fn R", 32),
                (crate::tr!(monitor_view_touch_l), 64),
                (crate::tr!(monitor_view_touch_r), 128),
            ]
            .iter()
            .enumerate()
            {
                self.rect(
                    40. + i as f32 * 95.,
                    263.,
                    88.,
                    27.,
                    6.,
                    if b2 & mask != 0 {
                        blue_color()
                    } else {
                        color(0.91, 0.93, 0.97)
                    },
                );
                (*self.target.0).SetTransform(&identity());
                self.text(
                    x + (47. + i as f32 * 95.) * scale,
                    y + 266. * scale,
                    85. * scale,
                    22.,
                    label,
                    0,
                    ink_color(),
                );
                (*self.target.0).SetTransform(&mat);
            }
        }
        (*self.target.0).SetTransform(&identity());
    }
}
fn identity() -> D2D1_MATRIX_3X2_F {
    let mut m: D2D1_MATRIX_3X2_F = unsafe { std::mem::zeroed() };
    m.matrix = [[1., 0.], [0., 1.], [0., 0.]];
    m
}

const FRAME_MESSAGE: u32 = WM_APP + 0x4d;
const RECORDING_ERROR_MESSAGE: u32 = WM_APP + 0x4e;
#[link(name = "kernel32")]
extern "system" {
    fn CreateWaitableTimerExW(
        attributes: *mut std::ffi::c_void,
        name: *const u16,
        flags: u32,
        access: u32,
    ) -> *mut std::ffi::c_void;
    fn SetWaitableTimer(
        timer: *mut std::ffi::c_void,
        due: *const i64,
        period: i32,
        callback: *mut std::ffi::c_void,
        arg: *mut std::ffi::c_void,
        resume: i32,
    ) -> i32;
    fn WaitForSingleObject(handle: *mut std::ffi::c_void, millis: u32) -> u32;
    fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
}
struct DrawClock {
    stop: Arc<std::sync::atomic::AtomicBool>,
    pending: Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    fps: u32,
}
impl DrawClock {
    fn new(hwnd: HWND, fps: u32) -> std::io::Result<Self> {
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let pending = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (end, posted) = (stop.clone(), pending.clone());
        let hwnd = hwnd as usize;
        let thread = std::thread::Builder::new()
            .name("opends-monitor-draw".into())
            .spawn(move || unsafe {
                let timer = CreateWaitableTimerExW(null_mut(), null(), 2, 0x1f0003);
                let period = std::time::Duration::from_secs_f64(1.0 / fps as f64);
                let mut deadline = Instant::now();
                while !end.load(Acquire) {
                    let h = hwnd as HWND;
                    if IsWindowVisible(h) == 0 || IsIconic(GetAncestor(h, GA_ROOT)) != 0 {
                        break;
                    }
                    if !posted.swap(true, AcqRel) && PostMessageW(h, FRAME_MESSAGE, 0, 0) == 0 {
                        posted.store(false, Release);
                        break;
                    }
                    deadline += period;
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if !remaining.is_zero() {
                        let due =
                            -((remaining.as_nanos() / 100).max(1).min(i64::MAX as u128) as i64);
                        if !timer.is_null()
                            && SetWaitableTimer(timer, &due, 0, null_mut(), null_mut(), 0) != 0
                        {
                            WaitForSingleObject(timer, 1000);
                        } else {
                            std::thread::sleep(remaining);
                        }
                    } else {
                        deadline = Instant::now();
                    }
                }
                if !timer.is_null() {
                    CloseHandle(timer);
                }
            })?;
        Ok(Self {
            stop,
            pending,
            thread: Some(thread),
            fps,
        })
    }
}
impl Drop for DrawClock {
    fn drop(&mut self) {
        self.stop.store(true, Release);
        if let Some(t) = self.thread.take() {
            let _ = crate::shutdown::join(t);
        }
    }
}

struct ViewState {
    commands: Option<command_view::View>,
    battery: Rc<Cell<crate::battery::Display>>,
    battery_observed: crate::battery::View,
    battery_tip: Option<crate::battery_ui::Tooltip>,
    clock: Option<DrawClock>,
    hub: Arc<Monitor>,
    root: HWND,
    canvas: HWND,
    parent: HWND,
    popup: HWND,
    controls: Vec<HWND>,
    font: HFONT,
    renderer: Option<Renderer>,
    path: PathBuf,
    hz: u32,
    fps: u32,
    seconds: u32,
    page_visible: bool,
    saved: (i32, i32, i32, i32),
    scroll: i32,
    scroll_max: i32,
    input: crate::report::ParsedState,
    pulse: [u64; 24],
    plot: Vec<Bucket>,
    last: [u64; 10],
    nonzero: [u64; 10],
    input_time: u64,
    error: String,
    recording_error_shown: bool,
    has_collected: bool,
    last_status: Instant,
}
pub struct MonitorView {
    state: Box<RefCell<ViewState>>,
}
impl MonitorView {
    pub fn new(
        parent: HWND,
        hub: Arc<Monitor>,
        cfg: &Config,
        path: PathBuf,
        battery: Rc<Cell<crate::battery::Display>>,
    ) -> Result<Self, String> {
        unsafe {
            register("OpenDSMonitorHost", Some(host_proc));
            register("OpenDSMonitorCanvas", Some(canvas_proc));
            register("OpenDSMonitorPopup", Some(popup_proc));
            hub.commands.configure(cfg.commands.clone(), hub.now());
            let state = Box::new(ViewState {
                commands: None,
                battery,
                battery_observed: crate::battery::View::default(),
                battery_tip: None,
                clock: None,
                hub,
                root: null_mut(),
                canvas: null_mut(),
                parent,
                popup: null_mut(),
                controls: vec![],
                font: null_mut(),
                renderer: None,
                path,
                hz: cfg.monitor_sample_hz,
                fps: cfg.monitor_fps,
                seconds: cfg.monitor_seconds,
                page_visible: false,
                saved: (0, 0, 700, 500),
                scroll: 0,
                scroll_max: 0,
                input: neutral(),
                pulse: [0; 24],
                plot: vec![],
                last: [0; 10],
                nonzero: [0; 10],
                input_time: 0,
                error: String::new(),
                recording_error_shown: false,
                has_collected: false,
                last_status: Instant::now(),
            });
            let state = Box::new(RefCell::new(*state));
            let ptr = &*state as *const RefCell<ViewState>;
            let mut data = state.borrow_mut();
            let state_ref = &mut *data;
            state_ref.root = CreateWindowExW(
                0,
                wide("OpenDSMonitorHost").as_ptr(),
                null(),
                WS_CHILD | WS_CLIPCHILDREN,
                0,
                0,
                700,
                500,
                parent,
                null_mut(),
                GetModuleHandleW(null()),
                null_mut(),
            );
            if state_ref.root.is_null() {
                return Err(crate::tr!(monitor_view_could_not_create_monitor_window).into());
            }
            SetWindowLongPtrW(state_ref.root, GWLP_USERDATA, ptr as isize);
            state_ref.battery_tip = Some(crate::battery_ui::Tooltip::new(state_ref.root));
            crate::appearance::install(state_ref.root);
            state_ref.commands = Some(command_view::View::new(
                state_ref.root,
                state_ref.hub.clone(),
                cfg.commands.clone(),
                state_ref.path.clone(),
            )?);
            for (id, cls, text, style) in [
                (
                    1,
                    "BUTTON",
                    crate::tr!(monitor_view_start_capture),
                    BS_PUSHBUTTON,
                ),
                (2, "BUTTON", crate::tr!(monitor_view_clear), BS_PUSHBUTTON),
                (
                    3,
                    "BUTTON",
                    crate::tr!(monitor_view_source),
                    BS_AUTOCHECKBOX,
                ),
                (
                    4,
                    "BUTTON",
                    crate::tr!(monitor_view_forwarded),
                    BS_AUTOCHECKBOX,
                ),
                (
                    5,
                    "BUTTON",
                    crate::tr!(monitor_view_separate_window),
                    BS_PUSHBUTTON,
                ),
                (
                    6,
                    "BUTTON",
                    crate::tr!(monitor_view_always_on_top),
                    BS_AUTOCHECKBOX,
                ),
                (7, "COMBOBOX", "", CBS_DROPDOWNLIST),
                (8, "COMBOBOX", "", CBS_DROPDOWNLIST),
                (9, "COMBOBOX", "", CBS_DROPDOWNLIST),
                (10, "STATIC", crate::tr!(monitor_view_sample), SS_LEFT),
                (11, "STATIC", crate::tr!(monitor_view_refresh), SS_LEFT),
                (12, "STATIC", crate::tr!(monitor_view_history), SS_LEFT),
                (
                    13,
                    "STATIC",
                    crate::tr!(monitor_view_waiting_for_manual_start),
                    SS_LEFT,
                ),
                (
                    14,
                    "BUTTON",
                    crate::tr!(monitor_view_start_file_recording),
                    BS_PUSHBUTTON,
                ),
            ] {
                let hwnd = CreateWindowExW(
                    0,
                    wide(cls).as_ptr(),
                    wide(text).as_ptr(),
                    WS_CHILD
                        | WS_VISIBLE
                        | WS_TABSTOP
                        | style
                        | if cls == "COMBOBOX" {
                            CBS_OWNERDRAWFIXED | CBS_HASSTRINGS
                        } else {
                            0
                        },
                    0,
                    0,
                    100,
                    24,
                    state_ref.root,
                    id as HMENU,
                    GetModuleHandleW(null()),
                    null_mut(),
                );
                state_ref.controls.push(hwnd);
            }
            SendMessageW(state_ref.controls[2], BM_SETCHECK, BST_CHECKED as usize, 0);
            SendMessageW(state_ref.controls[5], BM_SETCHECK, BST_CHECKED as usize, 0);
            for (idx, options, selected) in [
                (6, vec![20, 50, 100, 200], state_ref.hz),
                (7, vec![15, 30, 60], state_ref.fps),
                (8, vec![30, 60, 120, 300, 600, 900], state_ref.seconds),
            ] {
                for v in &options {
                    let text = if idx == 8 {
                        crate::tr!(monitor_view_seconds, seconds = crate::i18n_arg!(v))
                    } else if idx == 7 {
                        format!("{v} FPS")
                    } else {
                        format!("{v} Hz")
                    };
                    SendMessageW(
                        state_ref.controls[idx],
                        CB_ADDSTRING,
                        0,
                        wide(&text).as_ptr() as isize,
                    );
                }
                SendMessageW(
                    state_ref.controls[idx],
                    CB_SETCURSEL,
                    options.iter().position(|v| *v == selected).unwrap_or(0),
                    0,
                );
            }
            state_ref.canvas = CreateWindowExW(
                0,
                wide("OpenDSMonitorCanvas").as_ptr(),
                null(),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                0,
                96,
                700,
                370,
                state_ref.root,
                null_mut(),
                GetModuleHandleW(null()),
                null_mut(),
            );
            if state_ref.canvas.is_null() {
                DestroyWindow(state_ref.root);
                return Err(crate::tr!(monitor_view_could_not_create_drawing_canvas).into());
            }
            SetWindowLongPtrW(state_ref.canvas, GWLP_USERDATA, ptr as isize);
            state_ref.layout();
            drop(data);
            crate::appearance::install(state.borrow().root);
            Ok(Self { state })
        }
    }
    pub fn place(&mut self, x: i32, y: i32, w: i32, h: i32, visible: bool) {
        unsafe {
            let mut data = self.state.borrow_mut();
            let s = &mut *data;
            let shown = visible && !s.page_visible;
            let visibility_changed = visible != s.page_visible;
            let geometry_changed = s.saved != (x, y, w, h);
            let size_changed = (s.saved.2, s.saved.3) != (w, h);
            s.saved = (x, y, w, h);
            s.page_visible = visible;
            if s.popup.is_null() {
                if geometry_changed {
                    SetWindowPos(
                        s.root,
                        null_mut(),
                        x,
                        y,
                        w,
                        h,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
                if visibility_changed {
                    ShowWindow(s.root, if visible { SW_SHOWNA } else { SW_HIDE });
                }
            }
            if size_changed {
                s.layout();
            }
            s.timer();
            s.sync_battery(shown);
        }
    }
    pub fn validate_scroll_motion(&self) -> Result<(), String> {
        let root = self.state.borrow().root;
        unsafe {
            if IsWindowVisible(root) != 0 {
                crate::theme_controls::validate_scrollbar(root)?;
            }
        }
        Ok(())
    }
    pub fn validate_commands(&self) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        if let Some(v) = state.commands.as_mut() {
            unsafe {
                v.validate()?;
            }
        }
        unsafe {
            crate::theme_controls::validate_palette(state.controls[13])?;
        }
        Ok(())
    }
    pub fn command_action(&self, action: u32) {
        if action == 111 {
            let mut s = self.state.borrow_mut();
            let parent = s.parent;
            let dpi = crate::gui_theme::window_dpi(parent).max(96) as f32;
            s.page_visible = false;
            unsafe {
                ShowWindow(s.root, SW_HIDE);
            }
            unsafe {
                s.timer();
            }
            if let Some(v) = s.commands.as_mut() {
                unsafe {
                    v.place(parent, 230., 110., 560., 430., dpi, true);
                }
            }
            return;
        }

        if action == 110 {
            let mut s = self.state.borrow_mut();
            let mut r: RECT = unsafe { std::mem::zeroed() };
            unsafe {
                GetClientRect(s.canvas, &mut r);
            }
            let w = r.right as f32 * 96. / crate::gui_theme::window_dpi(s.canvas).max(96) as f32;
            s.scroll = (model_bottom(if w < 680. { w } else { w * 0.41 }) + 130.) as i32;
            unsafe {
                InvalidateRect(s.canvas, null(), 0);
                UpdateWindow(s.canvas);
            }
            return;
        }

        if let Some(v) = self.state.borrow_mut().commands.as_mut() {
            unsafe {
                v.action(action);
            }
        }
    }
    pub fn preferences(&self) -> (u32, u32, u32) {
        let s = self.state.borrow();
        (s.hz, s.fps, s.seconds)
    }
    pub fn refresh(&mut self) {
        unsafe {
            let mut state = self.state.borrow_mut();
            state.timer();
            state.status();
            state.sync_battery(false);
        }
    }
    pub fn visible(&self) -> bool {
        let state = self.state.borrow();
        unsafe {
            IsWindowVisible(state.root) != 0 && IsIconic(GetAncestor(state.root, GA_ROOT)) == 0
        }
    }
    pub fn smoke_battery(
        &mut self,
        popup: bool,
        expected: crate::battery::View,
    ) -> Result<(), String> {
        unsafe {
            let mut state = self.state.borrow_mut();
            if popup {
                state.detach();
            } else {
                state.attach();
            }
            state.sync_battery(true);
            if state.battery.get().view != expected {
                return Err("Monitor battery differs from shared display".into());
            }
            if !state.hub.active.load(Acquire) && state.clock.is_some() {
                return Err("Battery display started a capture rendering clock".into());
            }
            let rect = state.battery_rect();
            if rect.right <= rect.left || rect.bottom <= rect.top {
                return Err("Monitor battery has invalid bounds".into());
            }
            InvalidateRect(state.canvas, null(), 0);
            UpdateWindow(state.canvas);
        }
        Ok(())
    }
    pub fn smoke(&mut self) {
        unsafe {
            let mut data = self.state.borrow_mut();
            let s = &mut *data;
            s.page_visible = true;
            ShowWindow(s.root, SW_SHOWNA);
            s.layout();
            InvalidateRect(s.canvas, null(), 0);
            UpdateWindow(s.canvas);
            s.detach();
            s.attach();
        }
    }
}
impl Drop for MonitorView {
    fn drop(&mut self) {
        let s = self.state.get_mut();
        s.clock = None;
        s.commands.take();
        s.battery_tip.take();
        s.hub.stop();
        unsafe {
            s.renderer = None;
            KillTimer(s.root, 1);
            SetWindowLongPtrW(s.root, GWLP_USERDATA, 0);
            SetWindowLongPtrW(s.canvas, GWLP_USERDATA, 0);
            if !s.popup.is_null() {
                SetWindowLongPtrW(s.popup, GWLP_USERDATA, 0);
                DestroyWindow(s.popup);
            }
            DestroyWindow(s.root);
            if !s.font.is_null() {
                DeleteObject(s.font as _);
            }
        }
    }
}

unsafe fn register(name: &str, proc: WNDPROC) {
    let mut wc: WNDCLASSW = std::mem::zeroed();
    wc.lpfnWndProc = proc;
    wc.hInstance = GetModuleHandleW(null());
    wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
    wc.hbrBackground = null_mut();
    let n = wide(name);
    wc.lpszClassName = n.as_ptr();
    RegisterClassW(&wc);
}
impl ViewState {
    unsafe fn battery_rect(&self) -> RECT {
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(self.canvas, &mut rect);
        let dpi = crate::gui_theme::window_dpi(self.canvas);
        let width = rect.right as f32 * 96. / dpi as f32;
        let model = if width < 680. { width } else { width * 0.41 };
        let (_, x, extra) = crate::battery_ui::header(model);
        let y = 12. + extra - self.scroll as f32;
        RECT {
            left: (x * dpi as f32 / 96.) as i32,
            top: (y * dpi as f32 / 96.) as i32,
            right: ((x + 94.) * dpi as f32 / 96.) as i32,
            bottom: ((y + 28.) * dpi as f32 / 96.) as i32,
        }
    }
    unsafe fn sync_battery(&mut self, force: bool) {
        let mut display = self.battery.get();
        display.update(self.hub.battery.snapshot(), self.hub.battery.now(), force);
        self.battery.set(display);
        let changed = self.battery_observed != display.view;
        self.battery_observed = display.view;
        let visible =
            IsWindowVisible(self.root) != 0 && IsIconic(GetAncestor(self.root, GA_ROOT)) == 0;
        let rect = self.battery_rect();
        if visible && (changed || force) {
            InvalidateRect(self.canvas, &rect, 0);
        }
        let mut tip_rect = rect;
        if visible {
            MapWindowPoints(
                self.canvas,
                self.root,
                &mut tip_rect as *mut _ as *mut POINT,
                2,
            );
            let mut canvas_rect: RECT = std::mem::zeroed();
            GetWindowRect(self.canvas, &mut canvas_rect);
            MapWindowPoints(
                null_mut(),
                self.root,
                &mut canvas_rect as *mut _ as *mut POINT,
                2,
            );
            tip_rect.top = tip_rect.top.max(canvas_rect.top);
            tip_rect.bottom = tip_rect.bottom.min(canvas_rect.bottom);
            if tip_rect.bottom <= tip_rect.top {
                tip_rect = std::mem::zeroed();
            }
        } else {
            tip_rect = std::mem::zeroed();
        }
        if let Some(tip) = self.battery_tip.as_mut() {
            if changed || force || tip.needs_rect(tip_rect) {
                tip.update(tip_rect, &display.view.tooltip());
            }
        }
    }
    unsafe fn layout(&mut self) {
        let dpi = crate::gui_theme::window_dpi(self.root).max(96);
        let p = |v: i32| crate::gui_theme::px(v, dpi);
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(self.root, &mut r);
        let w = r.right;
        let h = r.bottom;
        let font = CreateFontW(
            -p(13),
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            CLEARTYPE_QUALITY as u32,
            0,
            wide(crate::gui_theme::font_family()).as_ptr(),
        );
        for c in &self.controls {
            SendMessageW(*c, WM_SETFONT, font as usize, 1);
        }
        if !self.font.is_null() {
            DeleteObject(self.font as _);
        }
        self.font = font;
        let positions = [
            (0, 4, 98, 30),
            (106, 4, 54, 30),
            (175, 6, 90, 26),
            (273, 6, 98, 26),
            (390, 4, 95, 30),
            (498, 6, 70, 26),
            (40, 43, 96, 180),
            (188, 43, 102, 180),
            (343, 43, 108, 200),
            (0, 48, 38, 24),
            (150, 48, 36, 24),
            (305, 48, 36, 24),
            (0, 0, 0, 0),
        ];
        for (i, (x, y, cw, ch)) in positions.iter().enumerate() {
            if i < 12 {
                MoveWindow(self.controls[i], p(*x), p(*y), p(*cw), p(*ch), 1);
            }
        }
        MoveWindow(self.controls[12], 0, (h - p(28)).max(p(78)), w, p(26), 1);
        MoveWindow(self.controls[13], p(466), p(42), p(128), p(30), 1);
        MoveWindow(self.canvas, 0, p(84), w, (h - p(116)).max(1), 1);
        self.renderer = None;
        self.scroll = 0;
        let cw = w as f32 * 96. / dpi as f32;
        let viewport = (h - p(116)).max(1) as f32 * 96. / dpi as f32;
        let content = if cw < 680. {
            model_bottom(cw)
                + crate::battery_ui::header(cw).2
                + 130.
                + 660.
                + self
                    .hub
                    .commands
                    .history
                    .lock()
                    .map(|h| h.settings.rows as f32 * 30. * h.settings.content_scale() + 78.)
                    .unwrap_or(318.)
        } else {
            viewport.max(
                model_bottom(cw * 0.41)
                    + crate::battery_ui::header(cw * 0.41).2
                    + 130.
                    + self
                        .hub
                        .commands
                        .history
                        .lock()
                        .map(|h| h.settings.rows as f32 * 30. * h.settings.content_scale() + 78.)
                        .unwrap_or(318.),
            )
        };
        self.scroll_max = (content - viewport).max(0.) as i32;
        let mut si: SCROLLINFO = std::mem::zeroed();
        si.cbSize = std::mem::size_of::<SCROLLINFO>() as u32;
        si.fMask = SIF_RANGE | SIF_PAGE | SIF_POS;
        si.nMax = content as i32;
        si.nPage = viewport as u32;
        SetScrollInfo(self.root, SB_VERT as i32, &si, 0);
        ShowScrollBar(self.root, SB_VERT as i32, (self.scroll_max > 0) as i32);
        crate::theme_controls::refresh_scrollbar(self.root);
        // Showing a non-client scrollbar reduces the client width synchronously.
        // WM_SIZE may be reentrant, so apply the final width here as well.
        GetClientRect(self.root, &mut r);
        MoveWindow(
            self.canvas,
            0,
            p(84),
            r.right,
            (r.bottom - p(116)).max(1),
            1,
        );
        MoveWindow(
            self.controls[12],
            0,
            (r.bottom - p(28)).max(p(78)),
            r.right,
            p(26),
            1,
        );
        InvalidateRect(self.canvas, null(), 0);
    }
    unsafe fn timer(&mut self) {
        let top = GetAncestor(self.root, GA_ROOT);
        let visible = IsWindowVisible(self.root) != 0 && IsIconic(top) == 0;
        // Keep the render target attached to its HWND while the page is hidden.
        // Recreating Direct2D on every navigation blocks the UI on driver setup.
        // No drawing clock runs while hidden; host destruction still releases it.
        let wanted = visible && self.hub.active.load(Acquire);
        if self
            .clock
            .as_ref()
            .is_some_and(|c| c.fps != self.fps || c.thread.as_ref().is_none_or(|t| t.is_finished()))
            || !wanted
        {
            self.clock = None;
        }
        if wanted && self.clock.is_none() {
            match DrawClock::new(self.root, self.fps) {
                Ok(c) => self.clock = Some(c),
                Err(e) => self.error = e.to_string(),
            }
        }
    }
    fn save_preferences(&self) {
        if let Ok(mut c) = Config::load(&self.path) {
            c.monitor_sample_hz = self.hz;
            c.monitor_fps = self.fps;
            c.monitor_seconds = self.seconds;
            if let Err(e) = c.save(&self.path) {
                crate::logging::record_message(
                    crate::i18n::MessageId::monitor_view_could_not_save_monitor_preferences,
                    &crate::tr!(
                        monitor_view_could_not_save_monitor_preferences,
                        error = crate::i18n_arg!(e)
                    ),
                );
            }
        }
    }
    unsafe fn detach(&mut self) {
        if !self.popup.is_null() {
            return;
        }
        self.renderer = None;
        let dpi = crate::gui_theme::window_dpi(self.parent).max(96) as i32;
        self.popup = CreateWindowExW(
            WS_EX_APPWINDOW
                | if SendMessageW(self.controls[5], BM_GETCHECK, 0, 0) == BST_CHECKED as isize {
                    WS_EX_TOPMOST
                } else {
                    0
                },
            wide("OpenDSMonitorPopup").as_ptr(),
            wide(crate::tr!(monitor_view_opends_live_monitor)).as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1050 * dpi / 96,
            800 * dpi / 96,
            null_mut(),
            null_mut(),
            GetModuleHandleW(null()),
            null_mut(),
        );
        if self.popup.is_null() {
            self.error = crate::tr!(monitor_view_could_not_create_separate_window).into();
            return;
        }
        SetWindowLongPtrW(
            self.popup,
            GWLP_USERDATA,
            GetWindowLongPtrW(self.root, GWLP_USERDATA),
        );
        for kind in [ICON_SMALL, ICON_BIG] {
            SendMessageW(
                self.popup,
                WM_SETICON,
                kind as usize,
                SendMessageW(self.parent, WM_GETICON, kind as usize, 0),
            );
        }
        crate::appearance::install(self.popup);
        SetParent(self.root, self.popup);
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(self.popup, &mut r);
        MoveWindow(self.root, 0, 0, r.right, r.bottom, 1);
        ShowWindow(self.root, SW_SHOWNA);
        if std::env::args().nth(1).as_deref() != Some("ui-smoke-test")
            || std::env::var("OPENDS_UI_SMOKE_THEME_STRESS").as_deref() != Ok("1")
        {
            ShowWindow(self.popup, SW_SHOWNOACTIVATE);
        }
        SetWindowTextW(
            self.controls[4],
            wide(crate::tr!(monitor_view_dock_window)).as_ptr(),
        );
        self.layout();
        self.timer();
        self.sync_battery(true);
        PostMessageW(self.parent, WM_TIMER, 0x4f44, 0);
    }
    unsafe fn attach(&mut self) {
        if self.popup.is_null() {
            return;
        }
        self.renderer = None;
        let popup = self.popup;
        self.popup = null_mut();
        SetParent(self.root, self.parent);
        let (x, y, w, h) = self.saved;
        MoveWindow(self.root, x, y, w, h, 1);
        ShowWindow(
            self.root,
            if self.page_visible {
                SW_SHOWNA
            } else {
                SW_HIDE
            },
        );
        SetWindowLongPtrW(popup, GWLP_USERDATA, 0);
        DestroyWindow(popup);
        SetWindowTextW(
            self.controls[4],
            wide(crate::tr!(monitor_view_separate_window)).as_ptr(),
        );
        self.layout();
        self.timer();
    }
    unsafe fn command(&mut self, id: usize, code: usize) {
        match id {
            1 => {
                if self.hub.active.load(Acquire) {
                    self.hub.stop();
                } else if let Err(e) = self.hub.start(self.hz) {
                    self.error = e;
                }
                SetWindowTextW(
                    self.controls[0],
                    wide(if self.hub.active.load(Acquire) {
                        crate::tr!(monitor_view_stop_capture)
                    } else {
                        crate::tr!(monitor_view_start_capture)
                    })
                    .as_ptr(),
                );
            }
            2 => self.hub.clear(),
            3 => self.hub.signal(
                false,
                SendMessageW(self.controls[2], BM_GETCHECK, 0, 0) == BST_CHECKED as isize,
            ),
            4 => self.hub.signal(
                true,
                SendMessageW(self.controls[3], BM_GETCHECK, 0, 0) == BST_CHECKED as isize,
            ),
            5 => {
                if self.popup.is_null() {
                    self.detach()
                } else {
                    self.attach()
                }
            }
            6 => {
                if !self.popup.is_null() {
                    SetWindowPos(
                        self.popup,
                        if SendMessageW(self.controls[5], BM_GETCHECK, 0, 0) == BST_CHECKED as isize
                        {
                            HWND_TOPMOST
                        } else {
                            HWND_NOTOPMOST
                        },
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                }
            }
            7..=9 if code == CBN_SELCHANGE as usize => {
                let selection =
                    SendMessageW(self.controls[id - 1], CB_GETCURSEL, 0, 0).max(0) as usize;
                match id {
                    7 => {
                        self.hz = [20, 50, 100, 200][selection.min(3)];
                        if self.hub.active.load(Acquire) {
                            if let Err(e) = self.hub.start(self.hz) {
                                self.error = e;
                            }
                        }
                    }
                    8 => self.fps = [15, 30, 60][selection.min(2)],
                    _ => self.seconds = [30, 60, 120, 300, 600, 900][selection.min(5)],
                };
                self.save_preferences();
            }
            _ => {}
        }
        self.timer();
        self.status();
        InvalidateRect(self.canvas, null(), 0);
    }
    unsafe fn status(&mut self) {
        self.has_collected |= self.hub.active.load(Acquire);
        let recording = self.hub.recording.status();
        if let Some(error) = self.hub.commands.recording.error() {
            self.error = format!("{}: {error}", crate::tr!(command_file_failed));
        }
        let label = if recording.starting {
            crate::tr!(monitor_view_checking)
        } else if recording.active {
            crate::tr!(monitor_view_stop_file_recording)
        } else if recording.finishing {
            crate::tr!(monitor_view_saving)
        } else {
            crate::tr!(monitor_view_start_file_recording)
        };
        let mut current = [0u16; 256];
        let length = GetWindowTextW(self.controls[13], current.as_mut_ptr(), 256);
        if String::from_utf16_lossy(&current[..length.max(0) as usize]) != label {
            SetWindowTextW(self.controls[13], wide(label).as_ptr());
        }
        let enabled = self.hub.active.load(Acquire) && !recording.finishing && !recording.starting;
        if (IsWindowEnabled(self.controls[13]) != 0) != enabled {
            EnableWindow(self.controls[13], enabled as i32);
        }
        if recording.error.is_some() && !self.recording_error_shown {
            if PostMessageW(self.root, RECORDING_ERROR_MESSAGE, 0, 0) != 0 {
                self.recording_error_shown = true;
            }
        }
        let record_status = if recording.error.is_some() {
            crate::tr!(monitor_view_csv_recording_failed).into()
        } else if recording.starting {
            crate::tr!(monitor_view_checking_save_location).into()
        } else if recording.path.is_some() {
            format!(
                " · CSV {} {:.2} MiB{}",
                if recording.active {
                    crate::tr!(monitor_view_recording)
                } else if recording.finishing {
                    crate::tr!(monitor_view_saving_2)
                } else {
                    crate::tr!(monitor_view_saved)
                },
                recording.bytes as f64 / 1048576.,
                if recording.dropped > 0 {
                    crate::tr!(
                        monitor_view_dropped,
                        count = crate::i18n_arg!(recording.dropped)
                    )
                } else {
                    String::new()
                }
            )
        } else {
            String::new()
        };
        let status = if !self.error.is_empty() {
            self.error.clone()
        } else {
            crate::tr!(
                monitor_view_dropped_decode_errors_stream_resets_cache_mib,
                capture = crate::i18n_arg!(if self.hub.active.load(Relaxed) {
                    crate::tr!(monitor_view_capturing)
                } else if !self.has_collected {
                    crate::tr!(monitor_view_waiting_for_manual_start)
                } else {
                    crate::tr!(monitor_view_stopped_history_retained)
                }),
                connection = crate::i18n_arg!(if self.hub.connected.load(Relaxed) {
                    crate::tr!(monitor_view_connected)
                } else {
                    crate::tr!(monitor_view_waiting_for_controller)
                }),
                recording = crate::i18n_arg!(record_status),
                drops = crate::i18n_arg!(self.hub.dropped()),
                errors = crate::i18n_arg!(self.hub.decode_failures.load(Relaxed)),
                resets = crate::i18n_arg!(self.hub.resets.load(Relaxed)),
                cache = format!("{:.1}", self.hub.allocated_bytes() as f32 / 1048576.)
            )
        };
        SetWindowTextW(self.controls[12], wide(&status).as_ptr());
    }
    unsafe fn paint(&mut self) {
        let mut ps: PAINTSTRUCT = std::mem::zeroed();
        BeginPaint(self.canvas, &mut ps);
        if self.renderer.is_none() {
            match Renderer::new(self.canvas) {
                Ok(r) => self.renderer = Some(r),
                Err(e) => self.error = e,
            }
        }
        let mut rc: RECT = std::mem::zeroed();
        GetClientRect(self.canvas, &mut rc);
        let dpi = crate::gui_theme::window_dpi(self.canvas).max(96) as f32;
        let w = rc.right as f32 * 96. / dpi;
        let h = rc.bottom as f32 * 96. / dpi;
        let now = self.hub.display_time();
        let mut pressed = [0; 3];
        if let Ok(mut history) = self.hub.history.try_lock() {
            self.input = history.input;
            self.input_time = history.input_time;
            self.last = history.last;
            self.nonzero = history.nonzero;
            pressed = history.pressed;
            history.pressed = [0; 3];
            self.plot = history.plot(
                self.seconds,
                if w < 680. { w - 70. } else { w * 0.57 - 70. } as usize,
                now,
            );
        }
        for i in 0..24 {
            if pressed[i / 8] & (1 << (i % 8)) != 0 {
                self.pulse[i] = now + 60_000_000;
            }
        }
        let mut pulse = [0u8; 3];
        for i in 0..24 {
            if self.pulse[i] > now {
                pulse[i / 8] |= 1 << (i % 8);
            }
        }
        if !self.hub.connected.load(Relaxed) {
            self.input = neutral();
            pulse = [0; 3];
        }
        if let Some(r) = self.renderer.as_ref() {
            (*r.target.0).SetDpi(dpi, dpi);
            (*r.target.0).BeginDraw();
            (*r.target.0).SetTransform(&identity());
            (*r.target.0).Clear(&palette_color(crate::appearance::palette().bg));
            let offset = self.scroll as f32;
            let narrow = w < 680.;
            let left = if narrow { w } else { w * 0.41 };
            let (title_width, battery_x, header_extra) = crate::battery_ui::header(left);
            let bottom = model_bottom(left) + header_extra;
            let command_height = self
                .hub
                .commands
                .history
                .lock()
                .map(|h| h.settings.rows as f32 * 30. * h.settings.content_scale() + 78.)
                .unwrap_or(318.);
            let model_height = bottom + 98.;
            let command_top = model_height + 16.;
            // One value drives both the scroll range and the page height so a
            // shorter or taller table cannot leave a dead strip below it.
            let stack_height = command_top + command_height + 16.;
            if let Some(v) = self.commands.as_mut() {
                v.place(
                    self.canvas,
                    8.,
                    command_top + 8. - offset,
                    left - 16.,
                    command_height,
                    dpi,
                    IsWindowVisible(self.root) != 0,
                );
            }

            let graph_x = if narrow { 0. } else { left + 16. };
            let graph_y = if narrow {
                stack_height + 12. - offset
            } else {
                -offset
            };
            let graph_w = w - graph_x;
            let graph_h = if narrow {
                208.
            } else {
                ((h - 22.) / 3.).max(208.)
            };
            r.rect(
                0.,
                -offset,
                left,
                model_height,
                12.,
                palette_color(crate::appearance::palette().surface),
            );
            r.line(
                (12., model_height + 8. - offset),
                (left - 12., model_height + 8. - offset),
                1.,
                palette_color(crate::appearance::palette().line),
            );
            r.rect(
                0.,
                command_top - offset,
                left,
                command_height + 16.,
                12.,
                palette_color(crate::appearance::palette().surface),
            );
            r.text(
                16.,
                12. - offset,
                title_width,
                30.,
                if self.hub.edge.load(Relaxed) {
                    "DualSense Edge"
                } else {
                    "DualSense"
                },
                2,
                ink_color(),
            );
            r.battery(
                battery_x,
                12. + header_extra - offset,
                self.battery.get().view,
            );
            r.text(
                16.,
                39. + header_extra - offset,
                left - 32.,
                24.,
                if !self.hub.active.load(Relaxed) {
                    crate::tr!(monitor_view_capture_stopped_last_input_retained)
                } else if !self.hub.connected.load(Relaxed) {
                    crate::tr!(monitor_view_disconnected_waiting_to_reconnect)
                } else if now.saturating_sub(self.input_time) > 500_000_000 {
                    crate::tr!(monitor_view_waiting_for_new_input_reports)
                } else {
                    crate::tr!(monitor_view_live_input_buttons_sticks_and_touchpad)
                },
                0,
                muted_color(),
            );
            r.controller(
                4.,
                86. + header_extra - offset,
                left - 8.,
                &self.input,
                pulse,
                self.hub.edge.load(Relaxed),
            );
            {
                r.text(
                    18.,
                    bottom + 8. - offset,
                    left - 30.,
                    42.,
                    &crate::tr!(
                        monitor_view_left_stick_x_y_right_stick_x,
                        lx = format!("{:3}", self.input.lx),
                        ly = format!("{:3}", self.input.ly),
                        rx = format!("{:3}", self.input.rx),
                        ry = format!("{:3}", self.input.ry)
                    ),
                    0,
                    muted_color(),
                );
                r.text(
                    18.,
                    bottom + 54. - offset,
                    left - 30.,
                    24.,
                    crate::tr!(monitor_view_blue_source_orange_dots_forwarded),
                    0,
                    muted_color(),
                );
            }
            for group in 0..3 {
                self.wave(
                    r,
                    group,
                    graph_x,
                    graph_y + group as f32 * (graph_h + 8.),
                    graph_w,
                    graph_h,
                    now,
                );
            }
            if let Err(e) = hr((*r.target.0).EndDraw(null_mut(), null_mut())) {
                self.error = e;
                self.renderer = None;
            }
        }
        EndPaint(self.canvas, &ps);
    }
    unsafe fn wave(&self, r: &Renderer, group: usize, x: f32, y: f32, w: f32, h: f32, now: u64) {
        r.rect(
            x,
            y,
            w,
            h,
            12.,
            palette_color(crate::appearance::palette().surface),
        );
        r.text(
            x + 14.,
            y + 10.,
            w - 28.,
            25.,
            [
                crate::tr!(monitor_view_speaker_front_l_r),
                crate::tr!(monitor_view_haptics_rear_l_r),
                crate::tr!(monitor_view_microphone_mono),
            ][group],
            1,
            ink_color(),
        );
        let offset = [0, 2, 4][group];
        let channels = if group == 2 { 1 } else { 2 };
        let source = self.hub.source.load(Relaxed);
        let forwarded = self.hub.forwarded.load(Relaxed);
        let activity = wave_state(group, [source, forwarded], &self.last, &self.nonzero, now);
        let status = if group == 2 && !self.hub.microphone.load(Relaxed) {
            crate::tr!(monitor_view_microphone_disabled).into()
        } else if activity == WaveState::NoSignal {
            crate::tr!(monitor_view_no_audio_signal_selected).into()
        } else if activity == WaveState::Waiting {
            crate::tr!(monitor_view_no_data_received_yet).into()
        } else if activity == WaveState::Interrupted {
            crate::tr!(monitor_view_data_interrupted_500_ms).into()
        } else if activity == WaveState::Silent {
            crate::tr!(monitor_view_silent_valid_zero_amplitude_pcm_received).into()
        } else if group == 0 {
            crate::tr!(
                monitor_view_volume,
                route = crate::i18n_arg!(if self.hub.route.load(Relaxed) == 0x96 {
                    crate::tr!(monitor_view_headphone_route)
                } else {
                    crate::tr!(gui_controller_speaker)
                }),
                volume = crate::i18n_arg!(self.hub.volume.load(Relaxed))
            )
        } else if group == 2 {
            crate::tr!(
                monitor_view_source_original_level_forwarded_db,
                gain = crate::i18n_arg!(self.hub.microphone_gain_db.load(Relaxed)),
                limiter = crate::i18n_arg!(if self.hub.microphone_gain_db.load(Relaxed) > 0 {
                    crate::tr!(monitor_view_limited)
                } else {
                    ""
                })
            )
        } else {
            crate::tr!(monitor_view_blue_source_orange_forwarded_fixed_100).into()
        };
        r.text(x + 14., y + 34., w - 28., 20., &status, 0, muted_color());
        let track = (h - 60.) / channels as f32;
        let phase = (now as f64 / 1e9 * self.hz as f64).fract() as f32 * self.plot.len() as f32
            / (self.seconds * self.hz) as f32;
        for ch in 0..channels {
            let yy = y + 56. + track * ch as f32;
            r.text(
                x + 8.,
                yy + (track - 18.) / 2. - 7.,
                15.,
                18.,
                if channels == 1 {
                    "M"
                } else if ch == 0 {
                    "L"
                } else {
                    "R"
                },
                0,
                muted_color(),
            );
            plotting::wave(
                r,
                &self.plot,
                offset + ch,
                self.seconds,
                [source, forwarded],
                (x + 28., yy, w - 36., track),
                ch + 1 == channels,
                phase,
            );
        }
    }
}
unsafe extern "system" fn host_proc(hwnd: HWND, msg: UINT, w: WPARAM, l: LPARAM) -> LRESULT {
    if matches!(
        msg,
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX
    ) {
        let dc = w as HDC;
        SetBkColor(dc, crate::appearance::palette().surface);
        SetTextColor(dc, crate::appearance::palette().text);
        return crate::appearance::surface_brush() as LRESULT;
    }
    // Native controls can enter a nested message loop in DefWindowProcW. Never
    // hold the state borrow across it: the main window refreshes on that loop.
    if !matches!(
        msg,
        WM_SIZE | WM_COMMAND | FRAME_MESSAGE | RECORDING_ERROR_MESSAGE | WM_MOUSEWHEEL | WM_VSCROLL
    ) {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<ViewState>;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    if msg == RECORDING_ERROR_MESSAGE {
        // MessageBox pumps the parent's timer messages: retain no ViewState borrow.
        let error = (*ptr)
            .try_borrow()
            .ok()
            .and_then(|s| s.hub.recording.status().error);
        if let Some(error) = error {
            crate::theme_dialog::message(
                GetAncestor(hwnd, GA_ROOT),
                crate::tr!(monitor_view_opends_file_recording_stopped),
                &error,
            );
        }
        return 0;
    }
    if msg == WM_COMMAND && w & 65535 == 14 {
        // The save dialog pumps messages. Release ViewState before entering it.
        let hub = match (*ptr).try_borrow() {
            Ok(s) => s.hub.clone(),
            Err(_) => return 0,
        };
        let result = if hub.recording.status().active {
            hub.stop_recording();
            Ok(false)
        } else {
            choose_recording_path(GetAncestor(hwnd, GA_ROOT)).and_then(|path| {
                path.map_or(Ok(false), |path| hub.start_recording(path).map(|_| true))
            })
        };
        if let Ok(mut s) = (*ptr).try_borrow_mut() {
            match result {
                Ok(started) => {
                    s.error.clear();
                    if started {
                        s.recording_error_shown = false;
                    }
                }
                Err(e) => s.error = e,
            }
            s.status();
        }
        return 0;
    }
    let Ok(mut data) = (*ptr).try_borrow_mut() else {
        return DefWindowProcW(hwnd, msg, w, l);
    };
    let s = &mut *data;
    match msg {
        WM_SIZE => {
            s.layout();
            0
        }
        WM_COMMAND => {
            s.command(w & 65535, w >> 16);
            0
        }
        FRAME_MESSAGE => {
            if let Some(v) = s.commands.as_mut() {
                v.pulse();
            }
            if let Some(c) = s.clock.as_ref() {
                c.pending.store(false, Release);
            }
            s.timer();
            InvalidateRect(s.canvas, null(), 0);
            if s.last_status.elapsed().as_millis() >= 500 {
                s.status();
                s.last_status = Instant::now();
                // A continuously rendered surface can keep low-priority native timers pending.
                // Queue the ordinary status poll so active drawing cannot starve app lifecycle work.
                PostMessageW(s.parent, WM_TIMER, 0x4f44, 0);
            }
            0
        }
        WM_MOUSEWHEEL => {
            s.scroll = (s.scroll - ((w >> 16) as i16 as i32 / 120) * 48).clamp(0, s.scroll_max);
            SetScrollPos(hwnd, SB_VERT as i32, s.scroll, 0);
            crate::theme_controls::refresh_scrollbar(hwnd);
            InvalidateRect(s.canvas, null(), 0);
            s.sync_battery(false);
            0
        }
        WM_VSCROLL => {
            let code = w & 65535;
            let delta = match code as isize {
                SB_LINEUP => -30,
                SB_LINEDOWN => 30,
                SB_PAGEUP => -200,
                SB_PAGEDOWN => 200,
                _ => 0,
            };
            if code == SB_THUMBTRACK as usize {
                s.scroll = (w >> 16) as i32;
            } else {
                s.scroll += delta;
            }
            s.scroll = s.scroll.clamp(0, s.scroll_max);
            SetScrollPos(hwnd, SB_VERT as i32, s.scroll, 0);
            crate::theme_controls::refresh_scrollbar(hwnd);
            InvalidateRect(s.canvas, null(), 0);
            s.sync_battery(false);
            0
        }
        _ => unreachable!(),
    }
}
unsafe extern "system" fn canvas_proc(hwnd: HWND, msg: UINT, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_NCHITTEST {
        return HTTRANSPARENT as isize;
    }
    if msg == WM_ERASEBKGND {
        return 1;
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<ViewState>;
    if msg == WM_PAINT && !ptr.is_null() {
        if let Ok(mut s) = (*ptr).try_borrow_mut() {
            s.paint();
            return 0;
        }
    }
    DefWindowProcW(hwnd, msg, w, l)
}
unsafe extern "system" fn popup_proc(hwnd: HWND, msg: UINT, w: WPARAM, l: LPARAM) -> LRESULT {
    // Moving, sizing and system menus run modal loops inside DefWindowProcW.
    // Borrowing here for unhandled messages prevents refresh() from reentering.
    if !matches!(msg, WM_CLOSE | WM_SIZE | WM_DPICHANGED) {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<ViewState>;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let Ok(mut data) = (*ptr).try_borrow_mut() else {
        return DefWindowProcW(hwnd, msg, w, l);
    };
    let s = &mut *data;
    match msg {
        WM_CLOSE => {
            s.attach();
            0
        }
        WM_SIZE => {
            if w != SIZE_MINIMIZED as usize {
                let mut r: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut r);
                MoveWindow(s.root, 0, 0, r.right, r.bottom, 1);
                s.layout();
                s.sync_battery(true);
            }
            s.timer();
            PostMessageW(s.parent, WM_TIMER, 0x4f44, 0);
            0
        }
        WM_DPICHANGED => {
            let r = &*(l as *const RECT);
            SetWindowPos(
                hwnd,
                null_mut(),
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            s.layout();
            0
        }
        _ => unreachable!(),
    }
}
