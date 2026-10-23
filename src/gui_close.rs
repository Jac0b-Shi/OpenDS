//! Native close choices, separate from the bridge shutdown path.
use winapi::{shared::windef::HWND, um::winuser::*};

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
    show(parent, None)
}
fn show(parent: HWND, test: Option<(i32, bool)>) -> Result<Decision, String> {
    let response = crate::theme_dialog::show(
        parent,
        crate::theme_dialog::Request {
            title: "OpenDS",
            message: &format!(
                "{}\n\n{}",
                crate::tr!(gui_close_close_the_opends_window),
                crate::tr!(gui_close_minimizing_to_the_tray_keeps_bridging_and)
            ),
            buttons: &[
                (TRAY, crate::tr!(gui_minimize_to_tray)),
                (EXIT, crate::tr!(gui_exit_opends)),
                (IDCANCEL, crate::tr!(audio_test_view_cancel)),
            ],
            default: TRAY,
            cancel: IDCANCEL,
            verification: Some(crate::tr!(gui_close_remember_my_choice)),
            test: test.map(|(id, checked)| crate::theme_dialog::TestAction::Select(id, checked)),
        },
    )?;
    let choice = match response.button {
        TRAY => Choice::Tray,
        EXIT => Choice::Exit,
        _ => Choice::Cancel,
    };
    Ok(Decision {
        choice,
        remember: response.checked && choice != Choice::Cancel,
    })
}
pub fn smoke_test(parent: HWND) -> Result<(), String> {
    for (button, expected) in [
        (TRAY, Choice::Tray),
        (EXIT, Choice::Exit),
        (IDCANCEL, Choice::Cancel),
    ] {
        for checked in [false, true] {
            let response = show(parent, Some((button, checked)))?;
            if response.choice != expected
                || response.remember != (checked && expected != Choice::Cancel)
            {
                return Err(
                    crate::tr!(gui_close_close_action_or_remember_choice_test_failed).into(),
                );
            }
        }
    }
    Ok(())
}
