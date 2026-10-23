//! NWG 原生控件 + 按显示器 DPI 布局；UI 不参与实时数据处理。
use crate::{
    bridge::BridgeController,
    config::{CloseAction, Config},
    gui_theme as theme,
};
use native_windows_gui as nwg;
use std::{
    cell::{Cell, RefCell},
    ops::{Deref, DerefMut},
    path::PathBuf,
    rc::Rc,
    sync::atomic::Ordering,
};
use winapi::{
    shared::{minwindef::*, windef::*},
    um::{
        wingdi::{
            CreateSolidBrush, DeleteObject, SetBkColor, SetBkMode, SetTextColor, TRANSPARENT,
        },
        winuser::*,
    },
};

#[derive(Default)]
struct Controls {
    window: nwg::Window,
    icon: nwg::Icon,
    taskbar_icon: nwg::Icon,
    tray: nwg::TrayNotification,
    menu: nwg::Menu,
    start_menu: nwg::MenuItem,
    stop_menu: nwg::MenuItem,
    show_menu: nwg::MenuItem,
    exit_menu: nwg::MenuItem,
    start: nwg::Button,
    stop: nwg::Button,
    save: nwg::Button,
    refresh: nwg::Button,
    restore: nwg::Button,
    minimize_to_tray: nwg::Button,
    quit: nwg::Button,
    status: nwg::Label,
    count_input: nwg::Label,
    count_output: nwg::Label,
    count_audio: nwg::Label,
    dependencies: nwg::Label,
    install_status: nwg::Label,
    tips: nwg::Tooltip,
    logs: nwg::TextBox,
    auto_start: nwg::CheckBox,
    start_on_login: nwg::CheckBox,
    reconnect: nwg::CheckBox,
    hide: nwg::CheckBox,
    audio: nwg::CheckBox,
    haptics: nwg::CheckBox,
    microphone: nwg::CheckBox,
    audio_test: nwg::Button,
    route: nwg::ComboBox<String>,
    device: nwg::ComboBox<String>,
    buffer: nwg::TextInput,
    port: nwg::TextInput,
    speaker: nwg::TextInput,
    haptic_gain: nwg::TextInput,
    mic_gain: nwg::TrackBar,
    mic_buffer: nwg::ComboBox<String>,
    close_action: nwg::ComboBox<String>,
    language: nwg::ComboBox<String>,
    import_language: nwg::Button,
    language_folder: nwg::Button,
    language_notice: nwg::Label,
    active: nwg::CheckBox,
    whitelist: nwg::ListBox<String>,
    app_path: nwg::TextInput,
    add: nwg::Button,
    remove: nwg::Button,
    add_self: nwg::Button,
    doctor: nwg::Button,
    usbip: nwg::Button,
    hidhide_link: nwg::Button,
    notice: nwg::Notice,
    nav: Vec<nwg::Button>,
}
struct App {
    ui: Controls,
    controller: RefCell<BridgeController>,
    config: RefCell<Config>,
    path: PathBuf,
    devices: RefCell<Vec<String>>,
    mic_buffers: Vec<u16>,
    languages: RefCell<Vec<crate::i18n::Language>>,
    initial_language: String,
    language_files_changed: Cell<bool>,
    exiting: Cell<bool>,
    close_prompt_open: Cell<bool>,
    page: Cell<u8>,
    dpi: Cell<u32>,
    fonts: RefCell<theme::Fonts>,
    last_logs: RefCell<String>,
    white_brush: HBRUSH,
    smoke_deadline: Cell<Option<std::time::Instant>>,
    dependency_attention: Cell<bool>,
    dependency_check: Cell<std::time::Instant>,
    installation: RefCell<Option<crate::dependencies::Installation>>,
    monitor_view: RefCell<Option<crate::monitor_view::MonitorView>>,
    audio_test_view: RefCell<Option<crate::audio_test_view::Dialog>>,
}
impl Deref for App {
    type Target = Controls;
    fn deref(&self) -> &Controls {
        &self.ui
    }
}
impl DerefMut for App {
    fn deref_mut(&mut self) -> &mut Controls {
        &mut self.ui
    }
}
fn check(v: bool) -> nwg::CheckBoxState {
    if v {
        nwg::CheckBoxState::Checked
    } else {
        nwg::CheckBoxState::Unchecked
    }
}
fn checked(c: &nwg::CheckBox) -> bool {
    c.check_state() == nwg::CheckBoxState::Checked
}
fn mic_buffer_label(ms: u16) -> String {
    crate::tr!(
        gui_ms_samples,
        milliseconds = crate::i18n_arg!(ms),
        samples = crate::i18n_arg!(ms as u32 * 48)
    )
}
fn selected_mic_buffer(combo: &nwg::ComboBox<String>, values: &[u16]) -> u16 {
    combo
        .selection()
        .and_then(|i| values.get(i).copied())
        .unwrap_or(128)
        .clamp(16, 512)
}
impl App {
    fn build(
        cfg: Config,
        path: PathBuf,
        error: Option<String>,
        background: bool,
        smoke: bool,
    ) -> Result<Rc<Self>, String> {
        nwg::init().map_err(|e| e.to_string())?;
        let dpi = unsafe { crate::gui_theme::system_dpi() }.max(96);
        let fonts = theme::Fonts::new(dpi)?;
        let mut a = Self {
            ui: Controls::default(),
            controller: RefCell::new(BridgeController::default()),
            config: RefCell::new(cfg.clone()),
            path,
            devices: RefCell::new(vec![]),
            mic_buffers: vec![],
            languages: RefCell::new(vec![]),
            initial_language: cfg.language.clone(),
            language_files_changed: Cell::new(false),
            exiting: Cell::new(false),
            close_prompt_open: Cell::new(false),
            page: Cell::new(0),
            dpi: Cell::new(dpi),
            fonts: RefCell::new(fonts),
            last_logs: RefCell::new(String::new()),
            white_brush: unsafe { CreateSolidBrush(theme::WHITE) },
            smoke_deadline: Cell::new(None),
            dependency_attention: Cell::new(false),
            dependency_check: Cell::new(std::time::Instant::now()),
            installation: RefCell::new(None),
            monitor_view: RefCell::new(None),
            audio_test_view: RefCell::new(None),
        };
        let resources = nwg::EmbedResource::load(None).map_err(|e| e.to_string())?;
        a.ui.icon = nwg::Icon::from_embed(&resources, Some(1), None).map_err(|e| e.to_string())?;
        nwg::Icon::builder()
            .source_embed(Some(&resources))
            .source_embed_id(1)
            .size(Some((64, 64)))
            .build(&mut a.ui.taskbar_icon)
            .map_err(|e| e.to_string())?;
        let build = (|| -> Result<(), nwg::NwgError> {
            nwg::Window::builder()
                .size((theme::px(980, dpi), theme::px(720, dpi)))
                .position((theme::px(80, dpi), theme::px(60, dpi)))
                .title(if smoke { "OpenDS · UI test" } else { "OpenDS" })
                .flags(
                    nwg::WindowFlags::WINDOW
                        | if background {
                            nwg::WindowFlags::empty()
                        } else {
                            nwg::WindowFlags::VISIBLE
                        }
                        | nwg::WindowFlags::RESIZABLE
                        | nwg::WindowFlags::MINIMIZE_BOX,
                )
                .icon(Some(&a.ui.icon))
                .build(&mut a.ui.window)?;
            // NWG 1.0.13 sets ICON_SMALL only; the taskbar and Alt+Tab need ICON_BIG.
            unsafe {
                SendMessageW(
                    theme::hwnd(&a.ui.window.handle),
                    WM_SETICON,
                    ICON_BIG as usize,
                    a.ui.taskbar_icon.handle as isize,
                );
            }
            let ui = &mut a.ui;
            let window = &ui.window;
            macro_rules! button {
                ($field:ident,$text:expr) => {
                    nwg::Button::builder()
                        .parent(window)
                        .text($text)
                        .build(&mut ui.$field)?;
                    theme::owner_button(&ui.$field.handle);
                };
            }
            macro_rules! cb {
                ($field:ident,$text:expr,$v:expr) => {
                    nwg::CheckBox::builder()
                        .parent(window)
                        .text($text)
                        .background_color(Some([255, 255, 255]))
                        .check_state(check($v))
                        .build(&mut ui.$field)?;
                };
            }
            macro_rules! input {
                ($field:ident,$text:expr) => {
                    nwg::TextInput::builder()
                        .parent(window)
                        .text($text)
                        .build(&mut ui.$field)?;
                };
            }
            button!(start, crate::tr!(gui_start_bridge));
            button!(stop, crate::tr!(gui_stop));
            button!(save, crate::tr!(gui_save_settings));
            button!(refresh, crate::tr!(gui_refresh_devices));
            button!(audio_test, crate::tr!(gui_microphone_test));
            button!(restore, crate::tr!(gui_restore_controller_visibility));
            button!(minimize_to_tray, crate::tr!(gui_minimize_to_tray));
            button!(quit, crate::tr!(gui_exit_opends));
            button!(import_language, crate::tr!(language_import));
            button!(language_folder, crate::tr!(language_open_folder));
            nwg::ComboBox::builder()
                .parent(window)
                .collection(Vec::<String>::new())
                .build(&mut ui.language)?;
            for label in [
                &mut ui.status,
                &mut ui.count_input,
                &mut ui.count_output,
                &mut ui.count_audio,
                &mut ui.dependencies,
                &mut ui.install_status,
                &mut ui.language_notice,
            ] {
                nwg::Label::builder()
                    .parent(window)
                    .background_color(Some([255, 255, 255]))
                    .text("")
                    .build(label)?;
            }
            nwg::TextBox::builder()
                .parent(window)
                .readonly(true)
                .flags(
                    nwg::TextBoxFlags::VISIBLE
                        | nwg::TextBoxFlags::AUTOVSCROLL
                        | nwg::TextBoxFlags::VSCROLL,
                )
                .build(&mut ui.logs)?;
            cb!(
                auto_start,
                crate::tr!(gui_start_bridging_when_opends_opens),
                cfg.auto_start
            );
            cb!(
                start_on_login,
                crate::tr!(gui_run_in_the_tray_at_windows_sign),
                crate::startup::enabled().unwrap_or(false)
            );
            cb!(
                reconnect,
                crate::tr!(gui_reconnect_automatically),
                cfg.reconnect
            );
            cb!(
                hide,
                crate::tr!(gui_hide_the_physical_controller_to_avoid_duplicate),
                cfg.hide_device
            );
            cb!(audio, crate::tr!(gui_forward_game_audio), cfg.audio_enabled);
            cb!(
                haptics,
                crate::tr!(gui_forward_native_stereo_haptics),
                cfg.haptics_enabled
            );
            cb!(
                microphone,
                crate::tr!(gui_forward_controller_microphone),
                cfg.microphone_enabled
            );
            nwg::ComboBox::builder()
                .parent(window)
                .collection(vec![
                    crate::tr!(gui_ask).into(),
                    crate::tr!(gui_minimize_to_tray).into(),
                    crate::tr!(gui_exit_application).into(),
                ])
                .selected_index(Some(match cfg.close_action {
                    CloseAction::Ask => 0,
                    CloseAction::Tray => 1,
                    CloseAction::Exit => 2,
                }))
                .build(&mut ui.close_action)?;
            nwg::ComboBox::builder()
                .parent(window)
                .collection(vec![crate::tr!(gui_auto_select_bluetooth_dualsense).into()])
                .selected_index(Some(0))
                .build(&mut ui.device)?;
            nwg::ComboBox::builder()
                .parent(window)
                .collection(vec![
                    crate::tr!(gui_automatic_game_setting_headphone_connection).into(),
                    crate::tr!(gui_controller_speaker).into(),
                    crate::tr!(gui_headphones).into(),
                ])
                .selected_index(Some(cfg.audio_route as usize))
                .build(&mut ui.route)?;
            input!(port, &cfg.port.to_string());
            input!(buffer, &cfg.buffer.to_string());
            input!(speaker, &cfg.speaker_volume.to_string());
            input!(haptic_gain, &cfg.haptics_gain.to_string());
            let mut mic_buffers = vec![
                16u16,
                24,
                32,
                48,
                64,
                80,
                96,
                112,
                128,
                144,
                160,
                176,
                192,
                208,
                224,
                240,
                256,
                320,
                384,
                512,
                cfg.microphone_buffer_ms,
            ];
            mic_buffers.sort_unstable();
            mic_buffers.dedup();
            a.mic_buffers = mic_buffers.clone();
            nwg::ComboBox::builder()
                .parent(window)
                .selected_index(
                    mic_buffers
                        .iter()
                        .position(|ms| *ms == cfg.microphone_buffer_ms),
                )
                .collection(mic_buffers.iter().map(|ms| mic_buffer_label(*ms)).collect())
                .build(&mut ui.mic_buffer)?;
            nwg::TrackBar::builder()
                .parent(window)
                .range(Some(0..50))
                .pos(Some(cfg.microphone_gain_db as usize))
                .background_color(Some([255, 255, 255]))
                .build(&mut ui.mic_gain)?;
            cb!(active, crate::tr!(gui_enable_hidhide_device_hiding), false);
            nwg::ListBox::builder()
                .parent(window)
                .collection(Vec::<String>::new())
                .build(&mut ui.whitelist)?;
            input!(app_path, "");
            button!(add, crate::tr!(gui_add_application));
            button!(remove, crate::tr!(gui_remove_selected));
            button!(add_self, crate::tr!(gui_allow_opends_access));
            button!(doctor, crate::tr!(gui_check_again));
            button!(usbip, crate::tr!(gui_install_usb_ip_win2));
            button!(hidhide_link, crate::tr!(gui_install_hidhide));
            for name in [
                crate::tr!(gui_overview),
                crate::tr!(gui_live_monitor),
                crate::tr!(gui_connection_feedback),
                crate::tr!(gui_device_hiding),
                crate::tr!(gui_diagnostics_logs),
                crate::tr!(language_settings_title),
            ] {
                let mut b = Default::default();
                nwg::Button::builder()
                    .parent(window)
                    .text(name)
                    .build(&mut b)?;
                theme::owner_button(&b.handle);
                ui.nav.push(b);
            }
            nwg::TrayNotification::builder()
                .parent(window)
                .icon(Some(&ui.icon))
                .tip(Some("OpenDS"))
                .build(&mut ui.tray)?;
            nwg::Menu::builder()
                .parent(window)
                .popup(true)
                .build(&mut ui.menu)?;
            for (text, item) in [
                (crate::tr!(gui_start_bridge), &mut ui.start_menu),
                (crate::tr!(gui_stop_bridge), &mut ui.stop_menu),
                (crate::tr!(gui_open_settings), &mut ui.show_menu),
                (crate::tr!(gui_exit_opends), &mut ui.exit_menu),
            ] {
                nwg::MenuItem::builder()
                    .parent(&ui.menu)
                    .text(text)
                    .build(item)?;
            }
            nwg::Notice::builder()
                .parent(window)
                .build(&mut ui.notice)?;
            nwg::Tooltip::builder()
                .register_callback(&ui.count_input)
                .register_callback(&ui.count_output)
                .register_callback(&ui.count_audio)
                .register_callback(&ui.nav[4])
                .register(
                    &ui.usbip,
                    crate::tr!(gui_stop_bridging_to_download_and_open_the),
                )
                .register(
                    &ui.hidhide_link,
                    crate::tr!(gui_stop_bridging_to_download_and_open_the_2),
                )
                .build(&mut ui.tips)?;
            ui.install_status.set_text(crate::tr!(
                gui_installation_requires_administrator_approval_dependencies_will_be
            ));
            Ok(())
        })();
        build.map_err(|e| e.to_string())?;
        a.dpi
            .set(unsafe { crate::gui_theme::window_dpi(theme::hwnd(&a.window.handle)) }.max(96));
        unsafe {
            let h = theme::hwnd(&a.window.handle);
            let style = GetWindowLongPtrW(h, GWL_STYLE);
            SetWindowLongPtrW(h, GWL_STYLE, style | WS_CLIPCHILDREN as isize);
        }
        let a = Rc::new(a);
        *a.monitor_view.borrow_mut() = Some(crate::monitor_view::MonitorView::new(
            theme::hwnd(&a.window.handle),
            a.controller.borrow().state.monitor.clone(),
            &cfg,
            a.path.clone(),
        )?);
        a.apply_fonts()?;
        unsafe {
            let hwnd = theme::hwnd(&a.logs.handle);
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex & !(WS_EX_CLIENTEDGE as isize));
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            SetWindowLongPtrW(hwnd, GWL_STYLE, style & !(WS_BORDER as isize));
        }
        a.refresh();
        a.refresh_languages();
        if !crate::dependencies::ready() {
            a.page.set(4);
            a.controller.borrow().state.log_message(
                crate::i18n::MessageId::gui_first_install_the_missing_drivers_on_this,
                crate::tr!(gui_first_install_the_missing_drivers_on_this),
            );
        }
        a.layout();
        if let Some(e) = error {
            a.controller.borrow().state.error(e);
        } else if !smoke
            && crate::restart::startup_override().unwrap_or(cfg.auto_start || background)
        {
            a.start_bridge();
        }
        Ok(a)
    }
    fn all(&self) -> Vec<&nwg::ControlHandle> {
        vec![
            &self.start.handle,
            &self.stop.handle,
            &self.save.handle,
            &self.refresh.handle,
            &self.restore.handle,
            &self.minimize_to_tray.handle,
            &self.quit.handle,
            &self.status.handle,
            &self.count_input.handle,
            &self.count_output.handle,
            &self.count_audio.handle,
            &self.dependencies.handle,
            &self.install_status.handle,
            &self.logs.handle,
            &self.auto_start.handle,
            &self.start_on_login.handle,
            &self.reconnect.handle,
            &self.hide.handle,
            &self.audio.handle,
            &self.haptics.handle,
            &self.microphone.handle,
            &self.audio_test.handle,
            &self.route.handle,
            &self.device.handle,
            &self.buffer.handle,
            &self.port.handle,
            &self.speaker.handle,
            &self.haptic_gain.handle,
            &self.mic_gain.handle,
            &self.mic_buffer.handle,
            &self.close_action.handle,
            &self.language.handle,
            &self.import_language.handle,
            &self.language_folder.handle,
            &self.language_notice.handle,
            &self.active.handle,
            &self.whitelist.handle,
            &self.app_path.handle,
            &self.add.handle,
            &self.remove.handle,
            &self.add_self.handle,
            &self.doctor.handle,
            &self.usbip.handle,
            &self.hidhide_link.handle,
        ]
    }
    fn apply_fonts(&self) -> Result<(), String> {
        *self.fonts.borrow_mut() = theme::Fonts::new(self.dpi.get())?;
        let f = self.fonts.borrow();
        for h in self.all() {
            theme::set_font(h, &f.body);
        }
        for b in &self.nav {
            theme::set_font(&b.handle, &f.body);
        }
        for h in [
            &self.count_input.handle,
            &self.count_output.handle,
            &self.count_audio.handle,
        ] {
            theme::set_font(h, &f.value);
        }
        Ok(())
    }
    fn dimensions(&self) -> (i32, i32) {
        unsafe {
            let mut r: RECT = std::mem::zeroed();
            GetClientRect(theme::hwnd(&self.window.handle), &mut r);
            let dpi = self.dpi.get() as i32;
            (
                (r.right * 96 / dpi).max(760),
                (r.bottom * 96 / dpi).max(600),
            )
        }
    }
    fn layout(&self) {
        let (w, h) = self.dimensions();
        let dpi = self.dpi.get();
        let x = 228;
        let cw = w - x - 32;
        let page = self.page.get();
        if let Some(v) = self.monitor_view.borrow_mut().as_mut() {
            v.place(
                theme::px(x, dpi),
                theme::px(112, dpi),
                theme::px(cw, dpi),
                theme::px(h - 136, dpi),
                page == 1,
            );
        }
        for control in self.all() {
            unsafe {
                ShowWindow(theme::hwnd(control), SW_HIDE);
            }
        }
        let put =
            |control: &nwg::ControlHandle, x, y, w, h| theme::place(control, x, y, w, h, dpi, true);
        for (i, b) in self.nav.iter().enumerate() {
            put(&b.handle, 16, 156 + i as i32 * 54, 176, 42);
        }
        put(&self.minimize_to_tray.handle, 16, h - 102, 176, 36);
        put(&self.quit.handle, 16, h - 58, 176, 36);
        match page {
            0 => {
                put(&self.status.handle, x + 28, 199, cw - 56, 24);
                put(&self.start.handle, x + 28, 240, 128, 36);
                put(&self.stop.handle, x + 168, 240, 88, 36);
                let card = (cw - 32) / 3;
                put(&self.count_input.handle, x + 24, 366, card - 48, 42);
                put(&self.count_output.handle, x + card + 40, 366, card - 48, 42);
                put(
                    &self.count_audio.handle,
                    x + card * 2 + 56,
                    366,
                    card - 48,
                    42,
                );
                put(&self.logs.handle, x + 24, 498, cw - 48, (h - 546).max(52));
            }
            1 => {}
            2 => {
                put(&self.device.handle, x + 24, 158, cw - 48, 240);
                put(&self.audio.handle, x + 24, 221, cw / 2 - 32, 28);
                put(&self.haptics.handle, x + cw / 2, 221, cw / 2 - 24, 28);
                put(&self.microphone.handle, x + 24, 258, cw / 2 - 32, 28);
                put(&self.audio_test.handle, x + cw / 2, 258, 156, 30);
                put(&self.route.handle, x + 24, 324, cw - 48, 180);
                put(&self.speaker.handle, x + 24, 396, cw / 3 - 40, 34);
                put(
                    &self.haptic_gain.handle,
                    x + cw / 3 + 8,
                    396,
                    cw / 3 - 32,
                    34,
                );
                put(&self.mic_gain.handle, x + 2 * cw / 3, 396, cw / 3 - 24, 34);
                put(&self.port.handle, x + 24, 464, cw / 3 - 40, 34);
                put(&self.buffer.handle, x + cw / 3 + 8, 464, cw / 3 - 32, 34);
                put(
                    &self.mic_buffer.handle,
                    x + 2 * cw / 3,
                    464,
                    cw / 3 - 24,
                    220,
                );
                put(&self.reconnect.handle, x + 24, 511, cw - 48, 28);
                put(&self.hide.handle, x + 24, 552, cw - 48, 28);
                put(&self.refresh.handle, x, h - 62, 136, 38);
                put(&self.save.handle, w - 176, h - 62, 144, 38);
            }
            3 => {
                put(&self.active.handle, x + 24, 176, cw - 48, 28);
                put(&self.whitelist.handle, x + 24, 244, cw - 48, 174);
                put(&self.app_path.handle, x + 24, 443, cw - 48, 34);
                put(&self.add.handle, x + 24, 497, 114, 36);
                put(&self.remove.handle, x + 150, 497, 114, 36);
                put(&self.add_self.handle, x + 276, 497, 176, 36);
                put(&self.restore.handle, x, 580, 176, 38);
            }
            4 => {
                put(&self.dependencies.handle, x + 24, 136, cw - 48, 110);
                put(&self.doctor.handle, x + 24, 259, 120, 36);
                put(&self.usbip.handle, x + 156, 259, 176, 36);
                put(&self.hidhide_link.handle, x + 344, 259, 150, 36);
                put(&self.install_status.handle, x + 24, 304, cw - 48, 28);
                put(&self.logs.handle, x + 24, 400, cw - 48, (h - 448).max(88));
            }
            _ => {
                put(&self.language.handle, x + 24, 164, cw - 48, 240);
                put(&self.import_language.handle, x + 24, 212, 208, 38);
                put(&self.language_folder.handle, x + 248, 212, 208, 38);
                put(&self.language_notice.handle, x + 24, 266, cw - 48, 52);
                put(&self.auto_start.handle, x + 24, 340, cw - 48, 38);
                put(&self.start_on_login.handle, x + 24, 388, cw - 48, 38);
                put(&self.close_action.handle, x + 24, 488, 280, 220);
                put(&self.save.handle, w - 176, h - 62, 144, 38);
            }
        }
        unsafe {
            InvalidateRect(theme::hwnd(&self.window.handle), std::ptr::null(), 1);
            for b in &self.nav {
                InvalidateRect(theme::hwnd(&b.handle), std::ptr::null(), 1);
            }
        }
    }
    unsafe fn paint(&self, hwnd: HWND) {
        let mut ps: PAINTSTRUCT = std::mem::zeroed();
        let dc = BeginPaint(hwnd, &mut ps);
        let (w, h) = self.dimensions();
        let dpi = self.dpi.get();
        let f = self.fonts.borrow();
        let x = 228;
        let cw = w - x - 32;
        let rect = |x, y, w, h| RECT {
            left: theme::px(x, dpi),
            top: theme::px(y, dpi),
            right: theme::px(x + w, dpi),
            bottom: theme::px(y + h, dpi),
        };
        theme::fill(dc, &rect(0, 0, w, h), theme::BG);
        theme::fill(dc, &rect(0, 0, 208, h), theme::WHITE);
        theme::fill(dc, &rect(207, 0, 1, h), theme::LINE);
        let text = |x, y, w, h, s: &str, font: &nwg::Font, color| {
            theme::text(
                dc,
                &rect(x, y, w, h),
                s,
                font,
                color,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
            )
        };
        text(28, 36, 160, 40, "OpenDS", &f.title, theme::INK);
        text(
            28,
            78,
            164,
            24,
            crate::tr!(gui_dualsense_control_center),
            &f.small,
            theme::MUTED,
        );
        text(
            28,
            h - 189,
            154,
            25,
            crate::tr!(gui_bluetooth_connection_usb_experience),
            &f.small,
            theme::MUTED,
        );
        text(
            28,
            h - 160,
            154,
            22,
            concat!("v", env!("CARGO_PKG_VERSION")),
            &f.small,
            theme::MUTED,
        );
        let titles = [
            crate::tr!(gui_overview),
            crate::tr!(gui_live_monitor),
            crate::tr!(gui_connection_feedback),
            crate::tr!(gui_device_hiding),
            crate::tr!(gui_diagnostics_logs),
            crate::tr!(language_settings_title),
        ];
        let descriptions = [
            crate::tr!(gui_connection_status_and_feedback_at_a_glance),
            crate::tr!(gui_view_controller_input_and_audio_start_capture),
            crate::tr!(gui_choose_your_controller_and_adjust_audio_haptics),
            crate::tr!(gui_let_games_see_only_the_virtual_controller),
            crate::tr!(gui_check_dependencies_and_review_connection_and_recovery),
            crate::tr!(language_settings_description),
        ];
        let p = self.page.get() as usize;
        text(x, 28, cw, 42, titles[p], &f.title, theme::INK);
        text(x, 73, cw, 24, descriptions[p], &f.body, theme::MUTED);
        let card = |x, y, w, h| {
            theme::round(
                dc,
                &rect(x, y, w, h),
                theme::px(16, dpi),
                theme::WHITE,
                theme::LINE,
            )
        };
        match p {
            0 => {
                card(x, 112, cw, 188);
                text(
                    x + 28,
                    132,
                    cw - 56,
                    42,
                    crate::tr!(gui_native_feedback_wireless_freedom),
                    &f.title,
                    theme::INK,
                );
                text(
                    x + 28,
                    176,
                    cw - 56,
                    23,
                    crate::tr!(gui_dualsense_bluetooth_local_virtual_usb_controller),
                    &f.body,
                    theme::MUTED,
                );
                let c = (cw - 32) / 3;
                for (i, name) in [
                    crate::tr!(gui_input_reports),
                    crate::tr!(gui_output_feedback),
                    crate::tr!(gui_audio_blocks),
                ]
                .iter()
                .enumerate()
                {
                    let left = x + i as i32 * (c + 16);
                    card(left, 320, c, 112);
                    text(left + 24, 337, c - 48, 24, name, &f.small, theme::MUTED);
                }
                card(x, 452, cw, h - 476);
                text(
                    x + 24,
                    466,
                    cw - 48,
                    26,
                    crate::tr!(gui_recent_activity),
                    &f.strong,
                    theme::INK,
                );
                if self.last_logs.borrow().is_empty() {
                    text(
                        x + 24,
                        506,
                        cw - 48,
                        30,
                        crate::tr!(gui_ready_start_bridging_to_see_connection_and),
                        &f.body,
                        theme::MUTED,
                    );
                }
            }
            1 => {}
            2 => {
                card(x, 112, cw, 490);
                for (left, y, width, s) in [
                    (x + 24, 130, cw - 48, crate::tr!(gui_physical_controller)),
                    (x + 24, 296, cw - 48, crate::tr!(gui_audio_output)),
                    (
                        x + 24,
                        364,
                        cw / 3 - 40,
                        crate::tr!(gui_speaker_volume_0_255),
                    ),
                    (
                        x + cw / 3 + 8,
                        364,
                        cw / 3 - 32,
                        crate::tr!(gui_haptics_gain_10_20),
                    ),
                    (
                        x + 2 * cw / 3,
                        364,
                        cw / 3 - 24,
                        &crate::tr!(
                            gui_microphone_db_0_50,
                            gain = crate::i18n_arg!(self.mic_gain.pos())
                        ),
                    ),
                    (x + 24, 432, cw / 3 - 40, crate::tr!(gui_local_service_port)),
                    (
                        x + cw / 3 + 8,
                        432,
                        cw / 3 - 32,
                        crate::tr!(gui_speaker_haptics_buffer_blocks),
                    ),
                    (
                        x + 2 * cw / 3,
                        432,
                        cw / 3 - 24,
                        crate::tr!(gui_microphone_safety_buffer_ms),
                    ),
                ] {
                    text(left, y, width, 24, s, &f.small, theme::MUTED);
                }
            }
            3 => {
                card(x, 112, cw, 448);
                text(
                    x + 24,
                    131,
                    cw - 48,
                    26,
                    crate::tr!(gui_application_whitelist),
                    &f.strong,
                    theme::INK,
                );
                text(
                    x + 24,
                    211,
                    cw - 48,
                    25,
                    crate::tr!(gui_listed_applications_can_still_see_the_physical),
                    &f.small,
                    theme::MUTED,
                );
                text(
                    x,
                    630,
                    cw,
                    24,
                    crate::tr!(gui_stopping_the_bridge_restores_the_hiding_settings),
                    &f.small,
                    theme::MUTED,
                );
            }
            4 => {
                card(x, 112, cw, 226);
                card(x, 358, cw, h - 382);
                text(
                    x + 24,
                    370,
                    cw - 48,
                    25,
                    crate::tr!(gui_application_log),
                    &f.strong,
                    theme::INK,
                );
            }
            _ => {
                card(x, 112, cw, 442);
                text(
                    x + 24,
                    130,
                    cw - 48,
                    24,
                    crate::tr!(language_label),
                    &f.strong,
                    theme::INK,
                );
                text(
                    x + 24,
                    452,
                    cw - 48,
                    24,
                    crate::tr!(gui_on_close),
                    &f.strong,
                    theme::INK,
                );
            }
        }
        EndPaint(hwnd, &ps);
    }
    fn raw(&self, hwnd: HWND, msg: UINT, w: WPARAM, l: LPARAM) -> Option<LRESULT> {
        unsafe {
            match msg {
                WM_TIMER if w == 0x4f44 => {
                    if self
                        .smoke_deadline
                        .get()
                        .is_some_and(|t| std::time::Instant::now() >= t)
                    {
                        self.confirm_close();
                    }
                    self.update();
                    SetTimer(
                        hwnd,
                        0x4f44,
                        if (IsWindowVisible(hwnd) != 0 && IsIconic(hwnd) == 0) || self.exiting.get()
                        {
                            500
                        } else {
                            5000
                        },
                        None,
                    );
                    Some(0)
                }
                WM_ENDSESSION if w != 0 => {
                    self.controller.borrow().stop();
                    self.controller.borrow_mut().wait();
                    None
                }
                WM_PAINT => {
                    self.paint(hwnd);
                    Some(0)
                }
                WM_ERASEBKGND => Some(1),
                WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
                    let dc = w as HDC;
                    SetBkColor(dc, theme::WHITE);
                    SetBkMode(dc, TRANSPARENT as i32);
                    SetTextColor(
                        dc,
                        if l as HWND == theme::hwnd(&self.logs.handle)
                            && self.last_logs.borrow().is_empty()
                        {
                            theme::MUTED
                        } else {
                            theme::INK
                        },
                    );
                    Some(self.white_brush as isize)
                }
                WM_DRAWITEM => {
                    let item = &*(l as *const DRAWITEMSTRUCT);
                    let nav = self
                        .nav
                        .iter()
                        .position(|b| theme::hwnd(&b.handle) == item.hwndItem);
                    theme::draw_button(
                        item,
                        &self.fonts.borrow(),
                        item.hwndItem == theme::hwnd(&self.start.handle)
                            || item.hwndItem == theme::hwnd(&self.save.handle),
                        nav.is_some(),
                        nav == Some(self.page.get() as usize),
                        self.dpi.get(),
                    );
                    if nav == Some(4) && self.dependency_attention.get() {
                        let dpi = self.dpi.get();
                        let radius = theme::px(8, dpi);
                        let right = item.rcItem.right - theme::px(14, dpi);
                        let top = (item.rcItem.top + item.rcItem.bottom - radius) / 2;
                        let color = theme::rgb(224, 58, 64);
                        theme::round(
                            item.hDC,
                            &RECT {
                                left: right - radius,
                                top,
                                right,
                                bottom: top + radius,
                            },
                            radius,
                            color,
                            color,
                        );
                    }
                    Some(1)
                }
                WM_DPICHANGED => {
                    self.dpi.set((w & 0xffff) as u32);
                    if let Err(e) = self.apply_fonts() {
                        self.controller.borrow().state.error(e);
                    }
                    let r = &*(l as *const RECT);
                    SetWindowPos(
                        hwnd,
                        std::ptr::null_mut(),
                        r.left,
                        r.top,
                        r.right - r.left,
                        r.bottom - r.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                    self.layout();
                    Some(0)
                }
                _ => None,
            }
        }
    }
    fn error(&self, e: impl Into<String>) {
        let e = e.into();
        self.controller.borrow().state.error(e.clone());
        nwg::modal_error_message(&self.window, "OpenDS", &e);
    }
    fn error_message(&self, id: crate::i18n::MessageId, e: impl Into<String>) {
        let e = e.into();
        self.controller.borrow().state.error_message(id, e.clone());
        nwg::modal_error_message(&self.window, "OpenDS", &e);
    }
    fn refresh_languages(&self) {
        let mut languages = crate::i18n::languages_in(&crate::i18n::directory());
        let requested = self.config.borrow().language.clone();
        if requested != "auto" && !languages.iter().any(|l| l.code == requested) {
            languages.push(crate::i18n::Language {
                code: requested.clone(),
                name: requested.clone(),
                local: false,
                translated: 0,
                total: 1,
            });
        }
        let mut labels = vec![crate::tr!(language_auto).to_owned()];
        labels.extend(languages.iter().map(|l| {
            crate::tr!(
                language_option,
                name = l.name.as_str(),
                source = if l.translated == 0 {
                    crate::tr!(language_unavailable)
                } else if l.local {
                    crate::tr!(language_local)
                } else {
                    crate::tr!(language_builtin)
                },
                coverage = l.translated * 100 / l.total.max(1)
            )
        }));
        self.language.set_collection(labels);
        self.language.set_selection(Some(
            languages
                .iter()
                .position(|l| l.code == requested)
                .map_or(0, |i| i + 1),
        ));
        *self.languages.borrow_mut() = languages;
        self.language_notice.set_text(
            if requested != self.initial_language || self.language_files_changed.get() {
                crate::tr!(language_pending)
            } else {
                ""
            },
        );
    }
    fn select_language(&self) -> Result<(), String> {
        let code = self
            .language
            .selection()
            .and_then(|i| i.checked_sub(1))
            .and_then(|i| self.languages.borrow().get(i).map(|l| l.code.clone()))
            .unwrap_or_else(|| "auto".into());
        if code == self.config.borrow().language && !self.language_files_changed.get() {
            return Ok(());
        }
        let result = Config::load(&self.path).and_then(|mut c| {
            c.language = code.clone();
            c.save(&self.path)
        });
        if let Err(e) = result {
            self.refresh_languages();
            return Err(e);
        }
        self.config.borrow_mut().language = code.clone();
        self.refresh_languages();
        self.offer_language_restart(&code)
    }
    fn offer_language_restart(&self, code: &str) -> Result<(), String> {
        let target = crate::i18n::Localizer::load(
            code,
            crate::i18n::system_language().as_deref(),
            &crate::i18n::directory(),
        );
        let name = self
            .languages
            .borrow()
            .iter()
            .find(|l| l.code == target.code)
            .map(|l| l.name.clone())
            .unwrap_or_else(|| target.code.clone());
        let bridging = self
            .controller
            .borrow()
            .state
            .running
            .load(Ordering::Acquire);
        // No RefCell borrow may cross the native dialog's nested message loop.
        if crate::gui_language::restart_prompt(
            theme::hwnd(&self.window.handle),
            &target,
            &name,
            bridging,
        )? {
            crate::restart::request(bridging);
            self.request_exit();
        }
        Ok(())
    }
    fn import_language(&self) -> Result<(), String> {
        let hwnd = theme::hwnd(&self.window.handle);
        let Some(path) = crate::gui_language::choose_file(hwnd)? else {
            return Ok(());
        };
        let (catalog, text) = crate::i18n::read_pack(&path)
            .map_err(|e| crate::tr!(language_import_error, error = e.localized()))?;
        if crate::i18n::languages_in(&crate::i18n::directory())
            .iter()
            .any(|l| l.code == catalog.code)
            && !crate::gui_language::confirm_replace(hwnd, &catalog.name)
        {
            return Ok(());
        }
        let imported = crate::i18n::install_text(catalog, &text, &crate::i18n::directory())
            .map_err(|e| crate::tr!(language_import_error, error = e.localized()))?;
        let requested = self.config.borrow().language.clone();
        let resolved = crate::i18n::resolve(
            &requested,
            crate::i18n::system_language().as_deref(),
            &crate::i18n::languages_in(&crate::i18n::directory()),
        );
        if resolved == imported.code {
            self.language_files_changed.set(true);
        }
        self.refresh_languages();
        if resolved == imported.code {
            return self.offer_language_restart(&requested);
        }
        nwg::modal_info_message(
            &self.window,
            crate::tr!(language_import_title),
            &crate::tr!(language_import_done, language = imported.name),
        );
        Ok(())
    }
    fn read_config(&self) -> Result<Config, String> {
        let mut c = self.config.borrow().clone();
        c.port = self
            .port
            .text()
            .parse()
            .map_err(|_| crate::tr!(gui_port_must_be_1_65535))?;
        c.buffer = self
            .buffer
            .text()
            .parse()
            .map_err(|_| crate::tr!(gui_buffer_must_be_1_128_blocks))?;
        c.speaker_volume = self
            .speaker
            .text()
            .parse()
            .map_err(|_| crate::tr!(gui_speaker_volume_must_be_0_255))?;
        c.haptics_gain = self
            .haptic_gain
            .text()
            .parse()
            .map_err(|_| crate::tr!(gui_haptics_gain_must_be_10_20))?;
        if c.port == 0 || !(1..=128).contains(&c.buffer) || !(10..=20).contains(&c.haptics_gain) {
            return Err(crate::tr!(gui_port_buffer_or_haptics_gain_is_out).into());
        }
        c.auto_start = checked(&self.auto_start);
        c.reconnect = checked(&self.reconnect);
        c.hide_device = checked(&self.hide);
        c.audio_enabled = checked(&self.audio);
        c.haptics_enabled = checked(&self.haptics);
        c.microphone_enabled = checked(&self.microphone);
        c.microphone_gain_db = self.mic_gain.pos().min(50) as u8;
        c.microphone_buffer_ms = selected_mic_buffer(&self.mic_buffer, &self.mic_buffers);
        c.audio_route = self.route.selection().unwrap_or(0) as u8;
        c.device_path = self
            .device
            .selection()
            .and_then(|i| self.devices.borrow().get(i).cloned())
            .unwrap_or_default();
        c.validate()?;
        Ok(c)
    }
    fn save_settings(&self) -> Result<Config, String> {
        let mut c = self.read_config()?;
        if let Some(v) = self.monitor_view.borrow().as_ref() {
            (c.monitor_sample_hz, c.monitor_fps, c.monitor_seconds) = v.preferences();
        }
        c.save(&self.path)?;
        let startup = checked(&self.start_on_login);
        if crate::startup::enabled()? != startup {
            crate::startup::set_enabled(startup)?;
        }
        *self.config.borrow_mut() = c.clone();
        Ok(c)
    }
    fn start_bridge(&self) {
        if self.installation.borrow().is_some() {
            self.error_message(
                crate::i18n::MessageId::gui_driver_installation_is_in_progress_wait_for,
                crate::tr!(gui_driver_installation_is_in_progress_wait_for),
            );
            return;
        }
        match self.save_settings() {
            Ok(c) => {
                let result = self.controller.borrow_mut().start(c, None);
                if let Err(e) = result {
                    self.error(e);
                }
            }
            Err(e) => self.error(e),
        }
        self.update();
    }
    fn refresh(&self) {
        let mut paths = vec![String::new()];
        let mut names = vec![crate::tr!(gui_auto_select_bluetooth_dualsense).to_string()];
        let selected = self.config.borrow().device_path.clone();
        for p in crate::hid::HidDevice::enum_paths() {
            if let Some(d) = crate::hid::HidDevice::open(&p) {
                if d.is_dualsense_bluetooth() {
                    names.push(format!(
                        "{} · {}",
                        if d.product_id == 0x0DF2 {
                            "DualSense Edge"
                        } else {
                            "DualSense"
                        },
                        d.serial_number().unwrap_or_else(|| p.clone())
                    ));
                    paths.push(p);
                }
            }
        }
        if !selected.is_empty() && !paths.iter().any(|p| p.eq_ignore_ascii_case(&selected)) {
            names.push(crate::tr!(gui_saved_controller_currently_disconnected).into());
            paths.push(selected.clone());
        }
        let index = paths
            .iter()
            .position(|p| p.eq_ignore_ascii_case(&selected))
            .unwrap_or(0);
        self.device.set_collection(names);
        self.device.set_selection(Some(index));
        *self.devices.borrow_mut() = paths;
        if let Some(h) = crate::hidhide::HidHide::open() {
            self.active
                .set_check_state(check(h.get_active().unwrap_or(false)));
            if let Some(v) = h.get_whitelist() {
                self.whitelist.set_collection(v);
            }
        }
        self.refresh_dependencies();
        self.update();
    }
    fn refresh_dependencies(&self) {
        let owned_healthy = self
            .controller
            .borrow()
            .state
            .hidhide_healthy
            .load(Ordering::Acquire);
        let status = crate::dependencies::inspect(owned_healthy);
        self.dependencies.set_text(&status.summary());
        let attention = !status.ready();
        if self.dependency_attention.replace(attention) != attention {
            unsafe {
                InvalidateRect(theme::hwnd(&self.nav[4].handle), std::ptr::null(), 1);
            }
        }
        self.dependency_check.set(std::time::Instant::now());
    }
    fn begin_installation(&self, component: &str) -> Result<(), String> {
        if self.installation.borrow().is_some() {
            return Err(crate::tr!(gui_another_driver_installation_is_in_progress_wait).into());
        }
        if self
            .controller
            .borrow()
            .state
            .running
            .load(Ordering::Acquire)
        {
            return Err(crate::tr!(gui_stop_bridging_before_installing_drivers).into());
        }
        let job = crate::dependencies::install(component)?;
        let message = crate::tr!(
            gui_preparing_complete_the_installation_in_the_official,
            component = crate::i18n_arg!(job.component)
        );
        self.install_status.set_text(&message);
        self.controller.borrow().state.log_message(
            crate::i18n::MessageId::gui_installation_log,
            crate::tr!(
                gui_installation_log,
                message = crate::i18n_arg!(message),
                path = crate::i18n_arg!(job.log_path.display())
            ),
        );
        *self.installation.borrow_mut() = Some(job);
        Ok(())
    }
    fn poll_installation(&self) {
        let outcome = {
            let mut slot = self.installation.borrow_mut();
            match slot.as_mut() {
                None => return,
                Some(job) => match job.poll() {
                    Ok(None) => return,
                    result => Some((slot.take().unwrap(), result)),
                },
            }
        };
        if let Some((job, result)) = outcome {
            let state = self.controller.borrow().state.clone();
            let tail = job.log_tail();
            for line in tail.lines().filter(|l| !l.trim().is_empty()).take(40) {
                state.log(line);
            }
            let success = matches!(&result,Ok(Some(status)) if status.success());
            if let Err(error) = &result {
                state.log(error);
            }
            let message = if success {
                if crate::dependencies::ready() {
                    crate::tr!(
                        gui_installed_dependencies_are_ready,
                        component = crate::i18n_arg!(job.component)
                    )
                } else {
                    crate::tr!(
                        gui_wizard_finished_but_dependencies_are_not_ready,
                        component = crate::i18n_arg!(job.component)
                    )
                }
            } else {
                crate::tr!(
                    gui_installation_failed_or_was_cancelled_see_the,
                    component = crate::i18n_arg!(job.component)
                )
            };
            self.install_status.set_text(&message);
            state.log(&message);
            self.refresh_dependencies();
        }
    }
    fn update(&self) {
        if let Some(v) = self.monitor_view.borrow_mut().as_mut() {
            v.refresh();
        }
        self.poll_installation();
        let mut c = self.controller.borrow_mut();
        c.poll();
        let s = c.state.clone();
        let idle = c.is_idle();
        drop(c);
        let running = s.running.load(Ordering::Acquire);
        let stopping = s.stop.load(Ordering::Acquire) && running;
        if self.exiting.get() {
            unsafe {
                SetTimer(theme::hwnd(&self.window.handle), 0x4f44, 500, None);
            }
        }
        let installing = self.installation.borrow().is_some();
        self.start_menu.set_enabled(!running && !installing);
        self.stop_menu.set_enabled(running && !stopping);
        if self.exiting.get() && idle {
            nwg::stop_thread_dispatch();
        }
        let phase = if stopping {
            crate::tr!(gui_stopping_and_restoring_devices).to_string()
        } else if s.connected.load(Ordering::Acquire) {
            crate::tr!(gui_connected_forwarding).to_string()
        } else if running {
            crate::tr!(gui_bridge_started_waiting_for_controller).to_string()
        } else {
            s.last_error
                .lock()
                .ok()
                .and_then(|e| e.clone())
                .map(|e| {
                    crate::tr!(
                        gui_bridge_stopped,
                        error = crate::i18n_arg!(e.chars().take(64).collect::<String>())
                    )
                })
                .unwrap_or_else(|| crate::tr!(gui_bridge_not_started).to_string())
        };
        if self.status.text() != phase {
            self.status.set_text(&phase);
        }
        let hwnd = theme::hwnd(&self.window.handle);
        if unsafe { IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 } {
            return;
        }
        if self.dependency_check.get().elapsed() >= std::time::Duration::from_secs(5) {
            self.refresh_dependencies();
        }
        for (label, value) in [
            (&self.count_input, s.frames_out.load(Ordering::Relaxed)),
            (
                &self.count_output,
                s.output_callbacks.load(Ordering::Relaxed),
            ),
            (&self.count_audio, s.audio_blocks.load(Ordering::Relaxed)),
        ] {
            let value = theme::compact_count(value);
            if label.text() != value {
                label.set_text(&value);
            }
        }
        self.start.set_enabled(!running && !installing);
        self.start_menu.set_enabled(!running && !installing);
        self.stop.set_enabled(running && !stopping);
        self.stop_menu.set_enabled(running && !stopping);
        self.restore.set_enabled(!running);
        self.active
            .set_enabled(!running && crate::hidhide::HidHide::open().is_some());
        for button in [&self.add, &self.remove, &self.add_self] {
            button.set_enabled(!running);
        }
        self.usbip.set_enabled(!running && !installing);
        self.hidhide_link.set_enabled(!running && !installing);
        if !installing {
            let hint = crate::tr!(gui_bridging_is_running_stop_it_before_installing);
            if running && self.install_status.text() != hint {
                self.install_status.set_text(hint);
            } else if !running && self.install_status.text() == hint {
                self.install_status.set_text(crate::tr!(
                    gui_installation_requires_administrator_approval_dependencies_will_be
                ));
            }
        }
        let logs = s.logs();
        if *self.last_logs.borrow() != logs {
            self.logs.set_text(&logs);
            self.logs.scroll_lastline();
            *self.last_logs.borrow_mut() = logs;
        }
        if self.last_logs.borrow().is_empty() && self.logs.text().is_empty() {
            self.logs.set_text(crate::tr!(
                gui_ready_start_bridging_to_see_connection_feedback
            ));
        }
        if self.exiting.get() && idle {
            nwg::stop_thread_dispatch();
        }
    }
    fn show_window(&self) {
        self.window.set_visible(true);
        // Showing a hidden iconic window alone leaves it minimized on the taskbar.
        if unsafe { IsIconic(theme::hwnd(&self.window.handle)) } != 0 {
            self.window.restore();
        }
        self.update();
        unsafe {
            SetTimer(theme::hwnd(&self.window.handle), 0x4f44, 500, None);
        }
        self.window.set_focus();
    }
    fn minimize_to_tray(&self) {
        if let Some(dialog) = self.audio_test_view.borrow().as_ref() {
            dialog.hide();
        }
        self.window.set_visible(false);
        unsafe {
            SetTimer(theme::hwnd(&self.window.handle), 0x4f44, 5000, None);
        }
    }
    fn request_exit(&self) {
        self.exiting.set(true);
        self.controller.borrow().stop();
        self.update();
    }
    fn apply_close_choice(&self, choice: crate::gui_close::Choice) {
        match choice {
            crate::gui_close::Choice::Tray => self.minimize_to_tray(),
            crate::gui_close::Choice::Exit => self.request_exit(),
            crate::gui_close::Choice::Cancel => {}
        }
    }
    fn confirm_close(&self) {
        if self.exiting.get() || self.close_prompt_open.replace(true) {
            return;
        }
        let saved = self.config.borrow().close_action;
        if saved != CloseAction::Ask {
            self.close_prompt_open.set(false);
            self.apply_close_choice(match saved {
                CloseAction::Tray => crate::gui_close::Choice::Tray,
                CloseAction::Exit => crate::gui_close::Choice::Exit,
                CloseAction::Ask => unreachable!(),
            });
            return;
        }
        // The modal dialog pumps timer messages; hold no controller/view borrow.
        let result = crate::gui_close::ask(theme::hwnd(&self.window.handle));
        self.close_prompt_open.set(false);
        match result {
            Ok(decision) => {
                if decision.remember {
                    let action = match decision.choice {
                        crate::gui_close::Choice::Tray => CloseAction::Tray,
                        crate::gui_close::Choice::Exit => CloseAction::Exit,
                        crate::gui_close::Choice::Cancel => CloseAction::Ask,
                    };
                    if let Err(error) = self.set_close_action(action) {
                        self.error(error);
                        return;
                    }
                }
                self.apply_close_choice(decision.choice);
            }
            Err(error) => self.error(error),
        }
    }
    fn set_close_action(&self, action: CloseAction) -> Result<(), String> {
        let result = Config::load(&self.path).and_then(|mut c| {
            c.close_action = action;
            c.save(&self.path)
        });
        if result.is_ok() {
            self.config.borrow_mut().close_action = action;
        }
        self.close_action
            .set_selection(Some(match self.config.borrow().close_action {
                CloseAction::Ask => 0,
                CloseAction::Tray => 1,
                CloseAction::Exit => 2,
            }));
        result
    }
    fn event(&self, event: nwg::Event, data: nwg::EventData, handle: nwg::ControlHandle) {
        use nwg::Event as E;
        if event == E::OnComboxBoxSelection && handle == self.language {
            if let Err(e) = self.select_language() {
                self.error(e);
            }
            return;
        }
        if event == E::OnComboxBoxSelection && handle == self.close_action {
            let action = match self.close_action.selection() {
                Some(1) => CloseAction::Tray,
                Some(2) => CloseAction::Exit,
                _ => CloseAction::Ask,
            };
            if let Err(error) = self.set_close_action(action) {
                self.error(error);
            }
            return;
        }
        if event == E::OnComboxBoxSelection && handle == self.mic_buffer {
            let ms = selected_mic_buffer(&self.mic_buffer, &self.mic_buffers);
            let result = Config::load(&self.path).and_then(|mut c| {
                c.microphone_buffer_ms = ms;
                c.save(&self.path)
            });
            if let Err(e) = result {
                self.mic_buffer.set_selection(
                    self.mic_buffers
                        .iter()
                        .position(|ms| *ms == self.config.borrow().microphone_buffer_ms),
                );
                self.error(e);
            } else {
                self.config.borrow_mut().microphone_buffer_ms = ms;
                self.controller
                    .borrow()
                    .state
                    .monitor
                    .microphone_buffer_ms
                    .store(ms as u32, Ordering::Relaxed);
            }
            return;
        }
        if event == E::OnHorizontalScroll && handle == self.mic_gain {
            let db = self.mic_gain.pos().min(50) as u8;
            self.controller
                .borrow()
                .state
                .monitor
                .microphone_gain_db
                .store(db as u32, Ordering::Relaxed);
            unsafe {
                InvalidateRect(theme::hwnd(&self.window.handle), std::ptr::null(), 0);
                if GetKeyState(VK_LBUTTON) < 0 {
                    return;
                }
            }
        }
        if matches!(event, E::TrackBarUpdated | E::OnHorizontalScroll) && handle == self.mic_gain {
            let db = self.mic_gain.pos().min(50) as u8;
            if self.config.borrow().microphone_gain_db == db {
                return;
            }
            let result = Config::load(&self.path).and_then(|mut c| {
                c.microphone_gain_db = db;
                c.save(&self.path)
            });
            if let Err(e) = result {
                self.mic_gain
                    .set_pos(self.config.borrow().microphone_gain_db as usize);
                self.controller
                    .borrow()
                    .state
                    .monitor
                    .microphone_gain_db
                    .store(self.mic_gain.pos() as u32, Ordering::Relaxed);
                self.error(e);
            } else {
                self.config.borrow_mut().microphone_gain_db = db;
                self.controller
                    .borrow()
                    .state
                    .monitor
                    .microphone_gain_db
                    .store(db as u32, Ordering::Relaxed);
            }
            return;
        }
        if event == E::OnTooltipText {
            let state = self.controller.borrow().state.clone();
            let value = if handle == self.count_input {
                Some((
                    crate::tr!(gui_input_reports),
                    state.frames_out.load(Ordering::Relaxed),
                ))
            } else if handle == self.count_output {
                Some((
                    crate::tr!(gui_output_feedback),
                    state.output_callbacks.load(Ordering::Relaxed),
                ))
            } else if handle == self.count_audio {
                Some((
                    crate::tr!(gui_audio_blocks),
                    state.audio_blocks.load(Ordering::Relaxed),
                ))
            } else {
                None
            };
            if let Some((name, count)) = value {
                data.on_tooltip_text().set_text(&format!("{name}：{count}"));
            } else if handle == self.nav[4] {
                data.on_tooltip_text()
                    .set_text(if self.dependency_attention.get() {
                        crate::tr!(gui_some_dependencies_are_missing_or_inaccessible_open)
                    } else {
                        crate::tr!(gui_dependencies_are_ready)
                    });
            }
            return;
        }
        if event == E::OnNotice && handle == self.notice {
            if self
                .smoke_deadline
                .get()
                .is_some_and(|t| std::time::Instant::now() >= t)
            {
                self.confirm_close();
            }
            self.update();
            return;
        }
        if event == E::OnResize && handle == self.window {
            self.layout();
            return;
        }
        if event == E::OnMinMaxInfo {
            if let nwg::EventData::OnMinMaxInfo(v) = data {
                v.set_min_size(
                    theme::px(860, self.dpi.get()),
                    theme::px(700, self.dpi.get()),
                );
            }
            return;
        }
        if event == E::OnWindowClose && handle == self.window {
            if let nwg::EventData::OnWindowClose(v) = data {
                v.close(false);
            }
            self.confirm_close();
            return;
        }
        if event == E::OnContextMenu && handle == self.tray {
            self.update();
            let (x, y) = nwg::GlobalCursor::position();
            self.menu.popup(x, y);
            return;
        }
        if event == E::OnMousePress(nwg::MousePressEvent::MousePressLeftUp) && handle == self.tray {
            self.show_window();
            return;
        }
        if event == E::OnMenuItemSelected {
            if handle == self.start_menu {
                self.start_bridge();
            } else if handle == self.stop_menu {
                self.controller.borrow().stop();
            } else if handle == self.show_menu {
                self.show_window();
            } else if handle == self.exit_menu {
                self.request_exit();
            }
            return;
        }
        if event != E::OnButtonClick {
            return;
        }
        if let Some(i) = self.nav.iter().position(|b| handle == *b) {
            if i != 2 {
                if let Some(d) = self.audio_test_view.borrow().as_ref() {
                    d.hide();
                }
            }
            self.page.set(i as u8);
            self.layout();
            self.update();
            return;
        }
        let result: Result<(), String> = if handle == self.start {
            self.start_bridge();
            return;
        } else if handle == self.stop {
            self.controller.borrow().stop();
            Ok(())
        } else if handle == self.quit {
            self.request_exit();
            return;
        } else if handle == self.minimize_to_tray {
            self.minimize_to_tray();
            return;
        } else if handle == self.import_language {
            self.import_language()
        } else if handle == self.language_folder {
            let folder = crate::i18n::directory();
            std::fs::create_dir_all(&folder)
                .map_err(|e| e.to_string())
                .and_then(|_| {
                    std::process::Command::new("explorer.exe")
                        .arg(folder)
                        .spawn()
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                })
        } else if handle == self.save {
            self.save_settings().map(|_| {
                self.controller.borrow().state.log_message(
                    crate::i18n::MessageId::gui_settings_saved_applied_the_next_time_bridging,
                    crate::tr!(gui_settings_saved_applied_the_next_time_bridging),
                )
            })
        } else if handle == self.refresh {
            self.refresh();
            Ok(())
        } else if handle == self.audio_test {
            (|| -> Result<(), String> {
                let mut dialog = self.audio_test_view.borrow_mut();
                if dialog.is_none() {
                    *dialog = Some(crate::audio_test_view::Dialog::new(
                        theme::hwnd(&self.window.handle),
                        self.controller.borrow().state.monitor.clone(),
                    )?);
                }
                if let Some(dialog) = dialog.as_ref() {
                    dialog.show();
                }
                Ok(())
            })()
        } else if handle == self.restore {
            crate::hidhide::recover_if_needed().map(|_| self.refresh())
        } else if handle == self.add {
            crate::hidhide::add_application(&self.app_path.text()).map(|_| {
                self.app_path.set_text("");
                self.refresh();
            })
        } else if handle == self.remove {
            match self.whitelist.selection_string() {
                Some(p) => crate::hidhide::remove_application(&p).map(|_| self.refresh()),
                None => Err(crate::tr!(gui_select_a_whitelist_entry_first).into()),
            }
        } else if handle == self.add_self {
            std::env::current_exe()
                .map_err(|e| e.to_string())
                .and_then(|p| crate::hidhide::add_application(&p.display().to_string()))
                .map(|_| self.refresh())
        } else if handle == self.active {
            let desired = checked(&self.active);
            match crate::hidhide::HidHide::open() {
                Some(h) if h.set_active(desired) && h.get_active() == Some(desired) => Ok(()),
                _ => Err(crate::tr!(gui_could_not_change_hidhide_state).into()),
            }
        } else if handle == self.doctor {
            self.refresh_dependencies();
            Ok(())
        } else if handle == self.usbip {
            self.begin_installation("usbip")
        } else if handle == self.hidhide_link {
            self.begin_installation("hidhide")
        } else {
            Ok(())
        };
        if let Err(e) = result {
            self.error(e);
        }
        self.update();
    }
}
impl Drop for App {
    fn drop(&mut self) {
        self.audio_test_view.get_mut().take();
        self.monitor_view.get_mut().take();
        unsafe {
            DeleteObject(self.white_brush as _);
        }
    }
}
pub fn run(
    cfg: Config,
    path: PathBuf,
    error: Option<String>,
    background: bool,
) -> Result<(), String> {
    run_inner(cfg, path, error, false, background)
}
pub fn smoke_test() -> Result<(), String> {
    let path = std::env::temp_dir().join(format!("opends-ui-smoke-{}.ini", std::process::id()));
    let result = run_inner(Config::default(), path.clone(), None, true, false);
    let _ = std::fs::remove_file(path);
    result
}
fn run_inner(
    cfg: Config,
    path: PathBuf,
    error: Option<String>,
    smoke: bool,
    background: bool,
) -> Result<(), String> {
    let app = App::build(cfg, path, error, background, smoke)?;
    #[cfg(debug_assertions)]
    let demo = if smoke && std::env::var_os("OPENDS_UI_SMOKE_DEMO").is_some() {
        app.window
            .set_text(crate::tr!(gui_opends_ui_test_simulated_signals));
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        Some((
            stop.clone(),
            crate::monitor::demo(app.controller.borrow().state.monitor.clone(), stop),
        ))
    } else {
        None
    };
    if smoke {
        // Exercise tray-first startup before showing the overview: background
        // refresh must publish waiting/error states even while hidden.
        app.window.set_visible(false);
        let state = app.controller.borrow().state.clone();
        state.running.store(true, Ordering::Release);
        app.update();
        if app.status.text() != crate::tr!(gui_bridge_started_waiting_for_controller) {
            return Err(crate::tr!(gui_tray_startup_did_not_display_the_waiting).into());
        }
        state.running.store(false, Ordering::Release);
        *state
            .last_error
            .lock()
            .map_err(|_| crate::tr!(gui_state_unavailable))? =
            Some(crate::tr!(gui_ui_smoke_error).into());
        app.update();
        if !app.status.text().contains(crate::tr!(gui_ui_smoke_error)) {
            return Err(crate::tr!(gui_overview_did_not_display_the_bridge_error).into());
        }
        *state
            .last_error
            .lock()
            .map_err(|_| crate::tr!(gui_state_unavailable))? = None;
        app.show_window();
        if app.status.text() != crate::tr!(gui_bridge_not_started) {
            return Err(crate::tr!(gui_opening_from_tray_did_not_refresh_the).into());
        }
        let hwnd = theme::hwnd(&app.window.handle);
        if unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } & WS_MINIMIZEBOX as isize == 0 {
            return Err(crate::tr!(gui_main_window_is_missing_its_minimize_button).into());
        }
        app.window.minimize();
        if unsafe { IsIconic(hwnd) == 0 || IsWindowVisible(hwnd) == 0 } {
            return Err(crate::tr!(gui_minimizing_did_not_preserve_the_taskbar_window).into());
        }
        app.apply_close_choice(crate::gui_close::Choice::Tray);
        if unsafe { IsWindowVisible(hwnd) } != 0 || app.exiting.get() {
            return Err(crate::tr!(gui_minimizing_to_tray_did_not_hide_the).into());
        }
        app.show_window();
        if unsafe { IsIconic(hwnd) != 0 || IsWindowVisible(hwnd) == 0 } {
            return Err(crate::tr!(gui_restoring_from_tray_did_not_restore_the).into());
        }
        if std::env::var_os("OPENDS_UI_SMOKE_SKIP_DIALOGS").is_none() {
            crate::gui_close::smoke_test(hwnd)?;
            crate::gui_language::smoke(hwnd)?;
        }
        app.apply_close_choice(crate::gui_close::Choice::Cancel);
        if app.exiting.get() || unsafe { IsWindowVisible(hwnd) } == 0 {
            return Err(crate::tr!(gui_cancelling_close_changed_the_window_or_exit).into());
        }
        app.set_close_action(CloseAction::Tray)?;
        if Config::load(&app.path)?.close_action != CloseAction::Tray
            || app.close_action.selection() != Some(1)
        {
            return Err(crate::tr!(gui_close_preference_was_not_persisted_or_synchronized).into());
        }
        app.confirm_close();
        if unsafe { IsWindowVisible(hwnd) } != 0 || app.exiting.get() {
            return Err(crate::tr!(gui_remembered_tray_action_did_not_take_effect).into());
        }
        app.show_window();
        app.set_close_action(CloseAction::Ask)?;
        if Config::load(&app.path)?.close_action != CloseAction::Ask
            || app.close_action.selection() != Some(0)
        {
            return Err(crate::tr!(gui_could_not_restore_the_close_confirmation).into());
        }
        app.window.set_visible(true);
        for page in 0..6 {
            app.page.set(page);
            app.layout();
        }
        app.page.set(
            std::env::var("OPENDS_UI_SMOKE_PAGE")
                .ok()
                .and_then(|s| s.parse::<u8>().ok())
                .filter(|&p| p < 6)
                .unwrap_or(1),
        );
        app.layout();
        if let Some(v) = app.monitor_view.borrow_mut().as_mut() {
            v.smoke();
        }
        let hwnd = theme::hwnd(&app.window.handle);
        for kind in [ICON_SMALL, ICON_BIG] {
            if unsafe { SendMessageW(hwnd, WM_GETICON, kind as usize, 0) } == 0 {
                return Err(crate::tr!(gui_window_or_taskbar_icon_was_not_loaded).into());
            }
        }
        // The deadline follows the saved exit path through confirm_close.
        app.set_close_action(CloseAction::Exit)?;
        if std::env::args().any(|a| a == "--smoke-restart") {
            crate::restart::request(crate::restart::startup_override().unwrap_or(false));
        }
        app.smoke_deadline.set(Some(
            std::time::Instant::now()
                + std::time::Duration::from_secs(
                    std::env::var("OPENDS_UI_SMOKE_SECONDS")
                        .ok()
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(2)
                        .clamp(2, 600),
                ),
        ));
    }
    let weak = Rc::downgrade(&app);
    let handler = nwg::full_bind_event_handler(&app.window.handle, move |e, d, h| {
        if let Some(a) = weak.upgrade() {
            a.event(e, d, h);
        }
    });
    let weak = Rc::downgrade(&app);
    let raw = nwg::bind_raw_event_handler(&app.window.handle, 0x10001, move |hwnd, m, w, l| {
        weak.upgrade().and_then(|a| a.raw(hwnd, m, w, l))
    })
    .map_err(|e| e.to_string())?;
    if unsafe { SetTimer(theme::hwnd(&app.window.handle), 0x4f44, 500, None) } == 0 {
        nwg::unbind_raw_event_handler(&raw).map_err(|e| e.to_string())?;
        nwg::unbind_event_handler(&handler);
        return Err(crate::tr!(gui_could_not_create_the_ui_status_timer).into());
    }
    app.update();
    app.layout();
    nwg::dispatch_thread_events();
    #[cfg(debug_assertions)]
    if let Some((stop, worker)) = demo {
        stop.store(true, Ordering::Release);
        let _ = worker.join();
    }
    unsafe {
        KillTimer(theme::hwnd(&app.window.handle), 0x4f44);
    }
    nwg::unbind_raw_event_handler(&raw).map_err(|e| e.to_string())?;
    nwg::unbind_event_handler(&handler);
    drop(app);
    Ok(())
}
