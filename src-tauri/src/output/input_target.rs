//! Local capability detection only; never reads the focused element's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputTarget {
    Editable,
    NotEditable,
    Unknown,
}

pub fn classify_target(
    enabled: bool,
    read_only: Option<bool>,
    edit_control: bool,
    known_non_input: bool,
) -> InputTarget {
    if !enabled || read_only == Some(true) || known_non_input {
        InputTarget::NotEditable
    } else if read_only == Some(false) || edit_control {
        InputTarget::Editable
    } else {
        InputTarget::Unknown
    }
}

pub async fn focused_input_target() -> InputTarget {
    #[cfg(target_os = "windows")]
    {
        tokio::time::timeout(
            std::time::Duration::from_millis(500),
            tauri::async_runtime::spawn_blocking(platform::detect),
        )
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or(InputTarget::Unknown)
    }
    #[cfg(not(target_os = "windows"))]
    {
        InputTarget::Unknown
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use windows::{
        core::Interface,
        Win32::{
            System::Com::*,
            UI::{Accessibility::*, WindowsAndMessaging::*},
        },
    };

    pub fn detect() -> InputTarget {
        unsafe {
            let initialized = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
            let result = detect_inner();
            if initialized {
                CoUninitialize();
            }
            result.unwrap_or(InputTarget::Unknown)
        }
    }

    unsafe fn detect_inner() -> windows::core::Result<InputTarget> {
        // A native caret is reliable evidence even in custom editors without UIA.
        let mut gui = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        let foreground = GetForegroundWindow();
        if foreground.is_invalid() {
            return Ok(InputTarget::NotEditable);
        }
        let mut class_name = [0u16; 128];
        let class_length = GetClassNameW(foreground, &mut class_name);
        let class_name = String::from_utf16_lossy(&class_name[..class_length.max(0) as usize]);
        if matches!(class_name.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd") {
            return Ok(InputTarget::NotEditable);
        }
        let thread = GetWindowThreadProcessId(foreground, None);
        let has_caret = GetGUIThreadInfo(thread, &mut gui).is_ok() && !gui.hwndCaret.is_invalid();
        let automation: IUIAutomation =
            match CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) {
                Ok(automation) => automation,
                Err(error) if !has_caret => return Err(error),
                Err(_) => return Ok(InputTarget::Editable),
            };
        if let Ok(settings) = automation.cast::<IUIAutomation2>() {
            let _ = settings.SetConnectionTimeout(200);
            let _ = settings.SetTransactionTimeout(200);
        }
        let focused = automation.GetFocusedElement()?;
        let enabled = focused.CurrentIsEnabled()?.as_bool();
        let control = focused.CurrentControlType()?;
        let read_only = focused
            .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
            .ok()
            .and_then(|value| value.CurrentIsReadOnly().ok())
            .map(|value| value.as_bool());
        let known_non_input = [
            UIA_ButtonControlTypeId,
            UIA_CheckBoxControlTypeId,
            UIA_RadioButtonControlTypeId,
            UIA_MenuItemControlTypeId,
            UIA_ListItemControlTypeId,
            UIA_TreeItemControlTypeId,
            UIA_ImageControlTypeId,
            UIA_TextControlTypeId,
            UIA_HyperlinkControlTypeId,
        ]
        .contains(&control);
        Ok(classify_target(
            enabled,
            read_only,
            control == UIA_EditControlTypeId || has_caret,
            known_non_input,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readonly_disabled_and_known_non_input_targets_do_not_receive_keystrokes() {
        assert_eq!(
            classify_target(false, None, true, false),
            InputTarget::NotEditable
        );
        assert_eq!(
            classify_target(true, Some(true), true, false),
            InputTarget::NotEditable
        );
        assert_eq!(
            classify_target(true, None, false, true),
            InputTarget::NotEditable
        );
    }
    #[test]
    fn editable_fields_and_unknown_custom_editors_remain_distinct() {
        assert_eq!(
            classify_target(true, Some(false), false, false),
            InputTarget::Editable
        );
        assert_eq!(
            classify_target(true, None, true, false),
            InputTarget::Editable
        );
        assert_eq!(
            classify_target(true, None, false, false),
            InputTarget::Unknown
        );
    }
}
