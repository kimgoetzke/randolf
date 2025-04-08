use crate::utils::{Point, Rect};
use std::fmt::Formatter;
use windows::Win32::Foundation::HWND;

const CHAR_LIMIT: usize = 20;

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

#[derive(Debug, Clone)]
pub struct Window {
  pub handle: WindowHandle,
  pub title: String,
  pub rect: Rect,
  pub center: Point,
}

impl Window {
  pub fn new(hwnd: HWND, title: String, rect: Rect) -> Self {
    Self {
      title,
      center: Point::from_center_of_rect(&rect),
      rect,
      handle: WindowHandle::from(hwnd),
    }
  }

  pub fn title_trunc(&self) -> String {
    let char_count = self.title.chars().count();
    if char_count <= CHAR_LIMIT + CHAR_LIMIT {
      self.title.to_string()
    } else {
      let prefix: String = self.title.chars().take(CHAR_LIMIT).collect();
      let suffix: String = self.title.chars().skip(char_count - CHAR_LIMIT).collect();
      format!("{}...{}", prefix, suffix)
    }
  }
}

impl PartialEq for Window {
  fn eq(&self, other: &Self) -> bool {
    self.handle == other.handle
  }
}

#[cfg(test)]
mod tests {
  use crate::utils::{Point, Rect, Window, WindowHandle};

  impl WindowHandle {
    pub fn new(hwnd: isize) -> Self {
      Self { hwnd }
    }
  }

  impl Window {
    pub fn from(window_handle: isize, title: String, rect: Rect) -> Self {
      Window {
        handle: WindowHandle::new(window_handle),
        title,
        center: Point::from_center_of_rect(&rect),
        rect,
      }
    }
  }
}
