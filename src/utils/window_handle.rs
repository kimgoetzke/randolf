use crate::utils::Window;
use std::fmt::Formatter;
use windows::Win32::Foundation::HWND;

/// A simple wrapper around a window handle. Its purpose is simply to standardise the
/// handle type across the codebase and provide a few utility methods.
#[derive(Eq, Hash, PartialEq, Copy, Clone, Debug)]
pub struct WindowHandle {
  pub hwnd: isize,
}

impl WindowHandle {
  pub fn as_hwnd(&self) -> HWND {
    HWND(self.hwnd as *mut core::ffi::c_void)
  }
}

impl From<HWND> for WindowHandle {
  fn from(value: HWND) -> Self {
    Self { hwnd: value.0 as isize }
  }
}

impl From<Window> for WindowHandle {
  fn from(value: Window) -> Self {
    Self { hwnd: value.handle.hwnd }
  }
}

impl From<&Window> for WindowHandle {
  fn from(value: &Window) -> Self {
    Self { hwnd: value.handle.hwnd }
  }
}

impl From<i32> for WindowHandle {
  fn from(value: i32) -> Self {
    Self { hwnd: value as isize }
  }
}

#[allow(clippy::from_over_into)]
impl Into<HWND> for WindowHandle {
  fn into(self) -> HWND {
    HWND(self.hwnd as *mut core::ffi::c_void)
  }
}

impl std::fmt::Display for WindowHandle {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "w#{}", self.hwnd)
  }
}

#[cfg(test)]
mod tests {
  use crate::utils::window_handle::WindowHandle;
  use crate::utils::{Rect, Window};
  use windows::Win32::Foundation::HWND;

  impl WindowHandle {
    pub fn new(hwnd: isize) -> Self {
      Self { hwnd }
    }
  }

  #[test]
  fn as_hwnd_converts_to_correct_hwnd() {
    let handle = WindowHandle::new(12345);

    assert_eq!(handle.as_hwnd().0 as isize, 12345);
  }

  #[test]
  fn from_hwnd_creates_window_handle_with_correct_value() {
    let hwnd = HWND(67890 as *mut core::ffi::c_void);
    let handle: WindowHandle = hwnd.into();

    assert_eq!(handle.hwnd, 67890);
  }

  #[test]
  fn from_window_creates_window_handle_with_correct_value() {
    let window = Window {
      handle: WindowHandle::new(54321),
      title: "".to_string(),
      rect: Rect::default(),
      center: Default::default(),
    };
    let handle: WindowHandle = window.into();

    assert_eq!(handle.hwnd, 54321);
  }

  #[test]
  fn window_handle_display_handles_large_values() {
    let handle = WindowHandle::new(6235641152123349);

    assert_eq!(handle.to_string(), "w#6235641152123349");
  }

  #[test]
  fn window_handle_display_handles_negative_values() {
    let handle = WindowHandle::new(-1);

    assert_eq!(handle.to_string(), "w#-1");
  }
}
