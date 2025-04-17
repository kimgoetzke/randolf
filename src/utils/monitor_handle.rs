use std::fmt::Display;
use windows::Win32::Graphics::Gdi::HMONITOR;

#[derive(Eq, Hash, PartialEq, PartialOrd, Ord, Copy, Clone, Debug, Default)]
pub struct MonitorHandle {
  pub handle: isize,
}

impl MonitorHandle {
  pub fn as_i64(&self) -> i64 {
    self.handle as i64
  }
}

impl From<isize> for MonitorHandle {
  fn from(value: isize) -> Self {
    Self { handle: value }
  }
}

impl From<i32> for MonitorHandle {
  fn from(value: i32) -> Self {
    Self { handle: value as isize }
  }
}

impl From<i64> for MonitorHandle {
  fn from(value: i64) -> Self {
    Self { handle: value as isize }
  }
}

impl From<HMONITOR> for MonitorHandle {
  fn from(value: HMONITOR) -> Self {
    Self {
      handle: value.0 as isize,
    }
  }
}

impl Display for MonitorHandle {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "m#{}", self.handle)
  }
}
