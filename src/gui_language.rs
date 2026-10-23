//! Language dialogs use an explicit catalog, independently of the active UI language.
use crate::i18n::{Localizer, MessageId as M};
use std::path::PathBuf;
use winapi::{
    shared::windef::HWND,
    um::{commdlg::*, winuser::*},
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
    crate::theme_dialog::show(
        parent,
        crate::theme_dialog::Request {
            title: crate::tr!(language_replace_title),
            message: &crate::tr!(language_replace_message, language = name),
            buttons: &[
                (IDYES, crate::tr!(dialog_yes)),
                (IDNO, crate::tr!(dialog_no)),
            ],
            default: IDNO,
            cancel: IDNO,
            verification: None,
            test: None,
        },
    )
    .is_ok_and(|r| r.button == IDYES)
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
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("language", name);
    let mut content = language.format_key(M::language_restart_message.key(), Some(&args));
    if bridging {
        content += "\n\n";
        content += language.text(M::language_restart_bridge);
    }
    let response = crate::theme_dialog::show(
        parent,
        crate::theme_dialog::Request {
            title: language.text(M::language_restart_title),
            message: &content,
            buttons: &[
                (IDCANCEL, language.text(M::language_restart_later)),
                (100, language.text(M::language_restart_now)),
            ],
            default: IDCANCEL,
            cancel: IDCANCEL,
            verification: None,
            test: test_button.map(|id| crate::theme_dialog::TestAction::Select(id, false)),
        },
    )?;
    Ok(response.button == 100)
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
