use crate::utils::{Monitor, Rect};
use windows::Win32::Graphics::Gdi::MONITORINFO;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct MonitorInfo {
  pub size: u32,
  /// Full monitor area including taskbar.
  pub monitor_area: Rect,
  /// Monitor work area i.e. excluding taskbar.
  pub work_area: Rect,
  pub flags: u32,
}

impl From<MONITORINFO> for MonitorInfo {
  fn from(value: MONITORINFO) -> Self {
    Self {
      size: value.cbSize,
      monitor_area: value.rcMonitor.into(),
      work_area: value.rcWork.into(),
      flags: value.dwFlags,
    }
  }
}

impl From<&Monitor> for MonitorInfo {
  fn from(value: &Monitor) -> Self {
    Self {
      size: value.size,
      monitor_area: value.monitor_area,
      work_area: value.work_area,
      flags: if value.is_primary { 1 } else { 0 },
    }
  }
}
