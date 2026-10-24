//! Isolated GUI acceptance checks for dependency preferences; no driver access.
use super::*;
use crate::dependency_paths::Component;

impl App {
    pub(super) fn test_dependency_directories(&self) -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "opends-dependency-ui-{}-中文 空格",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("x64")).map_err(|e| e.to_string())?;
        for file in ["usbip.exe", "x64/HidHideClient.exe"] {
            std::fs::write(root.join(file), []).map_err(|e| e.to_string())?;
        }
        let check = |ok: bool, detail: &str| {
            if ok {
                Ok(())
            } else {
                Err(format!("Dependency UI validation: {detail}"))
            }
        };
        let result = (|| {
            self.port.set_text("4321");
            let saved_port = Config::load(&self.path)?.port;
            let directory = root.to_str().unwrap().to_owned();
            for component in [Component::Usbip, Component::HidHide] {
                self.set_dependency_directory(component, directory.clone())?;
                check(
                    component.directory(&Config::load(&self.path)?) == directory,
                    "directory was not persisted",
                )?;
                check(
                    component.directory(&self.read_config()?) == directory,
                    "directory lost when reading other controls",
                )?;
                check(
                    self.port.text() == "4321" && Config::load(&self.path)?.port == saved_port,
                    "unrelated settings changed",
                )?;
                check(
                    self.set_dependency_directory(
                        component,
                        root.join("missing").to_string_lossy().into_owned(),
                    )
                    .is_err(),
                    "invalid directory accepted",
                )?;
                check(
                    component.directory(&Config::load(&self.path)?) == directory,
                    "invalid selection changed saved directory",
                )?;
                self.controller
                    .borrow()
                    .state
                    .running
                    .store(true, Ordering::Release);
                self.update();
                check(
                    !self.usbip_choose.enabled()
                        && !self.hidhide_choose.enabled()
                        && !self.hidhide_open.enabled(),
                    "path controls enabled during bridging",
                )?;
                check(
                    self.set_dependency_directory(component, String::new())
                        .is_err(),
                    "directory changed during bridging",
                )?;
                self.controller
                    .borrow()
                    .state
                    .running
                    .store(false, Ordering::Release);
            }
            let usbip = Component::Usbip.resolve(&directory);
            let hidhide = Component::HidHide.resolve(&directory);
            self.show_dependency_locations(usbip.as_deref(), hidhide.as_deref());
            self.update();
            check(
                self.usbip_directory.text().contains("usbip.exe")
                    && self.hidhide_directory.text().contains("HidHideClient.exe"),
                "resolved tool paths not visible",
            )?;
            check(
                self.hidhide_open.enabled(),
                "optional configuration tool did not become available",
            )?;
            // Use the actual command handlers to reset both preferences.
            for handle in [self.usbip_auto.handle, self.hidhide_auto.handle] {
                self.event(nwg::Event::OnButtonClick, nwg::EventData::NoData, handle);
            }
            let reset = Config::load(&self.path)?;
            check(
                reset.usbip_directory.is_empty() && reset.hidhide_directory.is_empty(),
                "automatic discovery reset failed",
            )?;
            self.port.set_text(&saved_port.to_string());
            self.page.set(4);
            self.layout();
            // Long guidance must retain every line and remain scrollable.
            let summary = crate::tr!(
                dependencies_usb_ip_win2_ude_hidhide_libviiper_opus,
                usbip = crate::tr!(dependency_usbip_custom_invalid),
                hidhide = crate::tr!(dependencies_not_accessible_check_driver_and_permissions),
                viiper = crate::tr!(dependencies_matching_dll_missing_place_it_beside_opends),
                opus = crate::tr!(dependencies_libopus_0_dll_missing_audio_and_native),
                path = "opends.ini"
            );
            self.dependencies.set_text_unix2dos(&summary);
            unsafe {
                let hwnd = theme::hwnd(&self.dependencies.handle);
                check(
                    SendMessageW(hwnd, EM_GETLINECOUNT as u32, 0, 0) >= 5,
                    "diagnostic lines were truncated",
                )?;
                check(
                    GetWindowLongPtrW(hwnd, GWL_STYLE) & (ES_READONLY | WS_VSCROLL) as isize
                        == (ES_READONLY | WS_VSCROLL) as isize,
                    "diagnostics must remain read-only and scrollable",
                )?;
            }
            self.show_dependency_locations(usbip.as_deref(), hidhide.as_deref());
            self.update();
            println!("Dependency UI validation: persistence, invalid selection, runtime guards, optional tool, reset and paths passed");
            Ok(())
        })();
        self.controller
            .borrow()
            .state
            .running
            .store(false, Ordering::Release);
        std::fs::remove_dir_all(root).map_err(|e| e.to_string())?;
        result
    }
}
