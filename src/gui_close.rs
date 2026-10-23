//! Native close choices, separate from the bridge shutdown path.
use std::ptr::null_mut;
use winapi::{
    shared::{minwindef::*, windef::HWND},
    um::{commctrl::*, winuser::*},
};

const TRAY: i32 = 100;
const EXIT: i32 = 101;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Tray,
    Exit,
    Cancel,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Decision {
    pub choice: Choice,
    pub remember: bool,
}

pub fn ask(parent: HWND) -> Result<Decision, String> {
    show(parent, None, 0)
}

fn show(parent: HWND, callback: PFTASKDIALOGCALLBACK, data: isize) -> Result<Decision, String> {
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let title = wide("OpenDS");
    let instruction = wide("关闭 OpenDS 窗口？");
    let content =
        wide("最小化到托盘会保持桥接与采集运行。\n退出将停止桥接、结束文件记录并恢复设备可见性。");
    let tray = wide("最小化到托盘");
    let exit = wide("退出 OpenDS");
    let cancel = wide("取消");
    let remember = wide("记住我的选项");
    let buttons = [
        TASKDIALOG_BUTTON {
            nButtonID: TRAY,
            pszButtonText: tray.as_ptr(),
        },
        TASKDIALOG_BUTTON {
            nButtonID: EXIT,
            pszButtonText: exit.as_ptr(),
        },
        TASKDIALOG_BUTTON {
            nButtonID: IDCANCEL,
            pszButtonText: cancel.as_ptr(),
        },
    ];
    let mut config: TASKDIALOGCONFIG = unsafe { std::mem::zeroed() };
    config.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as u32;
    config.hwndParent = parent;
    config.dwFlags = TDF_ALLOW_DIALOG_CANCELLATION | TDF_POSITION_RELATIVE_TO_WINDOW;
    config.pszWindowTitle = title.as_ptr();
    config.pszMainInstruction = instruction.as_ptr();
    config.pszContent = content.as_ptr();
    config.pszVerificationText = remember.as_ptr();
    config.cButtons = buttons.len() as u32;
    config.pButtons = buttons.as_ptr();
    config.nDefaultButton = TRAY;
    config.pfCallback = callback;
    config.lpCallbackData = data;
    let mut selected = 0;
    let mut checked = 0;
    let result = unsafe { TaskDialogIndirect(&config, &mut selected, null_mut(), &mut checked) };
    if result < 0 {
        return Err(format!(
            "无法打开关闭选项（0x{:08X}），请使用托盘菜单或侧栏按钮。",
            result as u32
        ));
    }
    let choice = match selected {
        TRAY => Choice::Tray,
        EXIT => Choice::Exit,
        _ => Choice::Cancel,
    };
    Ok(Decision {
        choice,
        remember: checked != 0 && choice != Choice::Cancel,
    })
}

// Exercise the real system dialog without requiring human input in UI smoke runs.
pub fn smoke_test(parent: HWND) -> Result<(), String> {
    unsafe extern "system" fn click(
        hwnd: HWND,
        msg: UINT,
        _: WPARAM,
        _: LPARAM,
        button: isize,
    ) -> i32 {
        if msg == TDN_CREATED {
            if button & 0x10000 != 0 {
                PostMessageW(hwnd, TDM_CLICK_VERIFICATION, 1, 0);
            }
            PostMessageW(hwnd, TDM_CLICK_BUTTON, (button & 0xffff) as usize, 0);
        }
        0
    }
    for (button, expected) in [
        (TRAY, Choice::Tray),
        (EXIT, Choice::Exit),
        (IDCANCEL, Choice::Cancel),
    ] {
        for remember in [false, true] {
            let decision = show(
                parent,
                Some(click),
                button as isize | if remember { 0x10000 } else { 0 },
            )?;
            if decision.choice != expected
                || decision.remember != (remember && expected != Choice::Cancel)
            {
                return Err("关闭选项或记忆选项检查失败".into());
            }
        }
    }
    // Ensure dismissing with the dialog close button follows cancellation too.
    unsafe extern "system" fn dismiss(
        hwnd: HWND,
        msg: UINT,
        _: WPARAM,
        _: LPARAM,
        _: isize,
    ) -> i32 {
        if msg == TDN_CREATED {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
        0
    }
    if show(parent, Some(dismiss), 0)?.choice != Choice::Cancel {
        return Err("取消关闭检查失败".into());
    }
    Ok(())
}
