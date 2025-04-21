use std::fmt::Display;
use windows::Win32::Graphics::Gdi::HMONITOR;

/// A simple wrapper around a monitor handle. Its purpose is simply to standardise the
/// handle type across the codebase and provide a few utility methods.
#[derive(Eq, Hash, PartialEq, PartialOrd, Ord, Copy, Clone, Debug, Default)]
pub struct MonitorHandle {
  pub handle: isize,
}

impl MonitorHandle {
  pub fn as_i64(&self) -> i64 {
    self.handle as i64
  }

  pub fn as_h_monitor(&self) -> HMONITOR {
    HMONITOR(self.handle as *mut core::ffi::c_void)
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

#[cfg(test)]
mod tests {
  use crate::utils::MonitorHandle;
  use windows::Win32::Graphics::Gdi::HMONITOR;

  #[test]
  fn monitor_handle_display_handles_large_i32_values() {
    let handle = MonitorHandle::from(i32::MAX);

    assert_eq!(handle.to_string(), "m#2147483647");
  }

  #[test]
  fn monitor_handle_display_handles_large_i64_values() {
    let handle = MonitorHandle::from(i64::MAX);

    assert_eq!(handle.to_string(), "m#9223372036854775807");
  }

  #[test]
  fn monitor_handle_display_handles_negative_values() {
    let handle = MonitorHandle::from(-42);

    assert_eq!(handle.to_string(), "m#-42");
  }

  #[test]
  fn monitor_handle_conversion_from_isize() {
    let handle = MonitorHandle::from(123isize);

    assert_eq!(handle.handle, 123);
  }

  #[test]
  fn monitor_handle_conversion_from_i32() {
    let handle = MonitorHandle::from(123i32);

    assert_eq!(handle.handle, 123);
  }

  #[test]
  fn monitor_handle_conversion_from_i64() {
    let handle = MonitorHandle::from(123i64);

    assert_eq!(handle.handle, 123);
  }

  #[test]
  fn monitor_handle_conversion_from_hmonitor() {
    let hmonitor = HMONITOR(123isize as _);
    let handle = MonitorHandle::from(hmonitor);

    assert_eq!(handle.handle, 123);
  }

  #[test]
  fn monitor_handle_as_i64_converts_correctly() {
    let handle = MonitorHandle::from(123isize);

    assert_eq!(handle.as_i64(), 123i64);
  }
}
