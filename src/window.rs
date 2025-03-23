use crate::point::Point;
use crate::rect::Rect;
use std::fmt::Formatter;
use windows::Win32::Foundation::HWND;

#[derive(Eq, Hash, PartialEq, Copy, Clone, Debug)]
pub(crate) struct WindowId {
  pub hwnd: isize,
}

impl WindowId {
  pub fn new(hwnd: isize) -> Self {
    Self { hwnd }
  }

  pub fn as_hwnd(&self) -> HWND {
    HWND(self.hwnd as *mut core::ffi::c_void)
  }
}

impl From<HWND> for WindowId {
  fn from(value: HWND) -> Self {
    Self { hwnd: value.0 as isize }
  }
}

impl From<Window> for WindowId {
  fn from(value: Window) -> Self {
    Self { hwnd: value.id.hwnd }
  }
}

impl From<&Window> for WindowId {
  fn from(value: &Window) -> Self {
    Self { hwnd: value.id.hwnd }
  }
}

#[allow(clippy::from_over_into)]
impl Into<HWND> for WindowId {
  fn into(self) -> HWND {
    HWND(self.hwnd as *mut core::ffi::c_void)
  }
}

impl std::fmt::Display for WindowId {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "#{}", self.hwnd)
  }
}

#[derive(Debug, Clone)]
pub(crate) struct Window {
  pub title: String,
  pub rect: Rect,
  pub center: Point,
  pub id: WindowId,
}

impl Window {
  pub fn new(title: String, rect: Rect, hwnd: HWND) -> Self {
    Self {
      title,
      center: Point::from_center_of_rect(&rect),
      rect,
      id: WindowId::from(hwnd),
    }
  }
}

impl PartialEq for Window {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}
