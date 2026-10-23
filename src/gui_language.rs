//! Language dialogs use an explicit catalog, independently of the active UI language.
use crate::i18n::{Localizer, MessageId as M};
use std::{path::PathBuf, ptr::null_mut};
use winapi::{
    shared::windef::HWND,
    um::{commctrl::*, commdlg::*, winuser::*},
};
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub fn choose_file(parent: HWND) -> Result<Option<PathBuf>, String> {
    let filter = wide(&format!("{}\0*.ftl\0", crate::tr!(language_import_filter)));
    let title = wide(crate::tr!(language_import_title));
    let mut filename = vec![0u16; 32768];
    let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = parent;
    ofn.lpstrFilter = filter.as_ptr();
    ofn.lpstrFile = filename.as_mut_ptr();
    ofn.nMaxFile = filename.len() as u32;
    ofn.lpstrTitle = title.as_ptr();
    ofn.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR | OFN_EXPLORER;
    if unsafe { GetOpenFileNameW(&mut ofn) } == 0 {
        let error = unsafe { CommDlgExtendedError() };
        return if error == 0 {
            Ok(None)
        } else {
            Err(crate::tr!(
                language_file_dialog_error,
                code = format!("0x{error:08X}")
            ))
        };
    }
    use std::os::windows::ffi::OsStringExt;
    let end = filename
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(filename.len());
    Ok(Some(std::ffi::OsString::from_wide(&filename[..end]).into()))
}
pub fn confirm_replace(parent: HWND, name: &str) -> bool {
    unsafe {
        MessageBoxW(
            parent,
            wide(&crate::tr!(language_replace_message, language = name)).as_ptr(),
            wide(crate::tr!(language_replace_title)).as_ptr(),
            MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2,
        ) == IDYES
    }
}
pub fn restart_prompt(
    parent: HWND,
    language: &Localizer,
    name: &str,
    bridging: bool,
) -> Result<bool, String> {
    restart_dialog(parent, language, name, bridging, None)
}
fn restart_dialog(
    parent: HWND,
    language: &Localizer,
    name: &str,
    bridging: bool,
    test_button: Option<i32>,
) -> Result<bool, String> {
    let title = wide(language.text(M::language_restart_title));
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("language", name);
    let mut content = language.format_key(M::language_restart_message.key(), Some(&args));
    if bridging {
        content += "\n\n";
        content += language.text(M::language_restart_bridge);
    }
    let content = wide(&content);
    let later = wide(language.text(M::language_restart_later));
    let now = wide(language.text(M::language_restart_now));
    let buttons = [
        TASKDIALOG_BUTTON {
            nButtonID: IDCANCEL,
            pszButtonText: later.as_ptr(),
        },
        TASKDIALOG_BUTTON {
            nButtonID: 100,
            pszButtonText: now.as_ptr(),
        },
    ];
    let mut c: TASKDIALOGCONFIG = unsafe { std::mem::zeroed() };
    c.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as u32;
    c.hwndParent = parent;
    c.dwFlags = TDF_ALLOW_DIALOG_CANCELLATION | TDF_POSITION_RELATIVE_TO_WINDOW;
    c.pszWindowTitle = title.as_ptr();
    c.pszContent = content.as_ptr();
    c.cButtons = buttons.len() as u32;
    c.pButtons = buttons.as_ptr();
    c.nDefaultButton = IDCANCEL;
    unsafe extern "system" fn click(
        hwnd: HWND,
        message: u32,
        _: usize,
        _: isize,
        data: isize,
    ) -> i32 {
        if message == TDN_CREATED {
            PostMessageW(hwnd, TDM_CLICK_BUTTON, data as usize, 0);
        }
        0
    }
    if let Some(button) = test_button {
        c.pfCallback = Some(click);
        c.lpCallbackData = button as isize;
    }
    let mut selected = 0;
    let result = unsafe { TaskDialogIndirect(&c, &mut selected, null_mut(), null_mut()) };
    if result < 0 {
        return Err(crate::tr!(
            language_restart_dialog_error,
            code = format!("0x{:08X}", result as u32)
        ));
    }
    Ok(selected == 100)
}
pub fn smoke(parent: HWND) -> Result<(), String> {
    for code in ["zh-CN", "en-US"] {
        let language = Localizer::load(
            code,
            None,
            &std::env::temp_dir().join("opends-no-local-language-files"),
        );
        for button in [IDCANCEL, 100] {
            if restart_dialog(parent, &language, code, true, Some(button))? != (button == 100) {
                return Err("Restart dialog selection mismatch".into());
            }
        }
    }
    Ok(())
}
