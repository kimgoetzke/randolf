use crate::common::Point;
use crate::window_picker::{G_INPUT_CAPTURE, InputDisposition, InputEvent};
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
use windows::Win32::UI::WindowsAndMessaging::{
  CallNextHookEx, GA_ROOT, GetAncestor, GetClassNameW, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT, SetWindowsHookExW,
  UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_RBUTTONDOWN,
  WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP, WindowFromPoint,
};
use windows::core::Result as WindowsResult;

/// Adapts native callbacks and hook ownership to the input-capture Seam.
pub(crate) struct NativeHooks {
  mouse_hook: HHOOK,
  keyboard_hook: HHOOK,
}

impl NativeHooks {
  /// Installs both hooks, rolling back partial installation.
  pub(crate) fn install() -> WindowsResult<Self> {
    let module = unsafe { GetModuleHandleW(None)? };
    let instance = HINSTANCE(module.0);
    let keyboard_hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(picker_keyboard_hook), Some(instance), 0)? };
    let mouse_hook = match unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(picker_mouse_hook), Some(instance), 0) } {
      Ok(hook) => hook,
      Err(error) => {
        unsafe {
          let _ = UnhookWindowsHookEx(keyboard_hook);
        }
        return Err(error);
      }
    };
    Ok(Self {
      mouse_hook,
      keyboard_hook,
    })
  }
}

impl Drop for NativeHooks {
  fn drop(&mut self) {
    unsafe {
      if let Err(error) = UnhookWindowsHookEx(self.mouse_hook) {
        error!("Failed to remove Window Picker mouse hook: {error}");
      }
      if let Err(error) = UnhookWindowsHookEx(self.keyboard_hook) {
        error!("Failed to remove Window Picker keyboard hook: {error}");
      }
    }
  }
}

extern "system" fn picker_mouse_hook(code: i32, message: WPARAM, hook_data: LPARAM) -> LRESULT {
  if code != HC_ACTION as i32 {
    return unsafe { CallNextHookEx(None, code, message, hook_data) };
  }
  let Some(ingress) = G_INPUT_CAPTURE.active_ingress() else {
    return unsafe { CallNextHookEx(None, code, message, hook_data) };
  };

  let event = match message.0 as u32 {
    WM_LBUTTONDOWN | WM_RBUTTONDOWN => {
      // Windows guarantees `MSLLHOOKSTRUCT` data for actionable low-level mouse-hook events.
      let mouse_event = unsafe { &*(hook_data.0 as *const MSLLHOOKSTRUCT) };
      if should_pass_mouse_input_through(mouse_event.pt) {
        InputEvent::PassthroughPointerPressed
      } else if message.0 as u32 == WM_LBUTTONDOWN {
        InputEvent::LeftPressed(Point::new(mouse_event.pt.x, mouse_event.pt.y))
      } else {
        InputEvent::RightPressed
      }
    }
    WM_LBUTTONUP => InputEvent::LeftReleased,
    WM_RBUTTONUP => InputEvent::RightReleased,
    _ => return unsafe { CallNextHookEx(None, code, message, hook_data) },
  };

  match ingress.dispatch(event) {
    InputDisposition::Suppress => LRESULT(1),
    InputDisposition::PassThrough => unsafe { CallNextHookEx(None, code, message, hook_data) },
  }
}

extern "system" fn picker_keyboard_hook(code: i32, message: WPARAM, hook_data: LPARAM) -> LRESULT {
  if code != HC_ACTION as i32 {
    return unsafe { CallNextHookEx(None, code, message, hook_data) };
  }
  let Some(ingress) = G_INPUT_CAPTURE.active_ingress() else {
    return unsafe { CallNextHookEx(None, code, message, hook_data) };
  };
  // Windows guarantees `KBDLLHOOKSTRUCT` data for actionable low-level keyboard-hook events.
  let keyboard_event = unsafe { &*(hook_data.0 as *const KBDLLHOOKSTRUCT) };
  if keyboard_event.vkCode != VK_ESCAPE.0 as u32 {
    return unsafe { CallNextHookEx(None, code, message, hook_data) };
  }
  let Some(event) = escape_input_event(message.0 as u32) else {
    return unsafe { CallNextHookEx(None, code, message, hook_data) };
  };

  match ingress.dispatch(event) {
    InputDisposition::Suppress => LRESULT(1),
    InputDisposition::PassThrough => unsafe { CallNextHookEx(None, code, message, hook_data) },
  }
}

fn escape_input_event(message: u32) -> Option<InputEvent> {
  match message {
    WM_KEYDOWN | WM_SYSKEYDOWN => Some(InputEvent::EscapePressed),
    WM_KEYUP | WM_SYSKEYUP => Some(InputEvent::EscapeReleased),
    _ => None,
  }
}

fn should_pass_mouse_input_through(point: POINT) -> bool {
  let hit_window = unsafe { WindowFromPoint(point) };
  if hit_window.0.is_null() {
    return false;
  }
  let root_window = unsafe { GetAncestor(hit_window, GA_ROOT) };
  let target_window = if root_window.0.is_null() { hit_window } else { root_window };
  let mut class_name = [0_u16; 256];
  let length = unsafe { GetClassNameW(target_window, &mut class_name) };
  is_input_passthrough_window_class(&String::from_utf16_lossy(&class_name[..length as usize]))
}

fn is_input_passthrough_window_class(class_name: &str) -> bool {
  matches!(class_name, "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "#32768")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn passthrough_classes_cover_taskbars_and_native_menus() {
    assert!(is_input_passthrough_window_class("Shell_TrayWnd"));
    assert!(is_input_passthrough_window_class("Shell_SecondaryTrayWnd"));
    assert!(is_input_passthrough_window_class("#32768"));
    assert!(!is_input_passthrough_window_class("EditorWindow"));
  }

  #[test]
  fn escape_messages_decode_regular_and_alt_modified_transitions() {
    assert_eq!(escape_input_event(WM_KEYDOWN), Some(InputEvent::EscapePressed));
    assert_eq!(escape_input_event(WM_SYSKEYDOWN), Some(InputEvent::EscapePressed));
    assert_eq!(escape_input_event(WM_KEYUP), Some(InputEvent::EscapeReleased));
    assert_eq!(escape_input_event(WM_SYSKEYUP), Some(InputEvent::EscapeReleased));
    assert_eq!(escape_input_event(WM_LBUTTONDOWN), None);
  }
}
