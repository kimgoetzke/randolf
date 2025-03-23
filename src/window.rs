use crate::rect::Rect;
use std::fmt::Formatter;
use windows::Win32::Foundation::HWND;

#[derive(Eq, Hash, PartialEq, Copy, Clone)]
pub(crate) struct Window {
  pub hwnd: isize,
}

impl From<HWND> for Window {
  fn from(value: HWND) -> Self {
    Self { hwnd: value.0 as isize }
  }
}

impl From<WindowInfo> for Window {
  fn from(value: WindowInfo) -> Self {
    Self { hwnd: value.hwnd }
  }
}

#[allow(clippy::from_over_into)]
impl Into<HWND> for Window {
  fn into(self) -> HWND {
    HWND(self.hwnd as *mut core::ffi::c_void)
  }
}

impl std::fmt::Display for Window {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "#{}", self.hwnd)
  }
}

#[derive(Debug, Clone)]
pub(crate) struct WindowInfo {
  pub title: String,
  pub rect: Rect,
  pub hwnd: isize,
}

impl WindowInfo {
  pub fn new(title: String, rect: Rect, hwnd: HWND) -> Self {
    Self {
      title,
      rect,
      hwnd: hwnd.0 as isize,
    }
  }
}

impl PartialEq for WindowInfo {
  fn eq(&self, other: &Self) -> bool {
    self.hwnd == other.hwnd && self.title == other.title && self.rect == other.rect
  }
}
