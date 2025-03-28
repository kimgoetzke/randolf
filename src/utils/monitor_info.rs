use crate::utils::Rect;
use windows::Win32::Graphics::Gdi::MONITORINFO;

#[allow(dead_code)]
pub struct MonitorInfo {
  pub size: u32,
  pub monitor: Rect,
  pub work_area: Rect,
  pub flags: u32,
}

impl From<MONITORINFO> for MonitorInfo {
  fn from(value: MONITORINFO) -> Self {
    Self {
      size: value.cbSize,
      monitor: value.rcMonitor.into(),
      work_area: value.rcWork.into(),
      flags: value.dwFlags,
    }
  }
}
