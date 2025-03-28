use crate::utils::Rect;
use windows::Win32::Graphics::Gdi::{HMONITOR, MONITORINFO};

#[derive(Debug, Clone)]
pub struct Monitor {
  pub handle: isize,
  pub is_primary: bool,
  /// Monitor work area (excluding taskbar)
  pub work_area: Rect,
  /// Full monitor area
  pub monitor_area: Rect,
}

impl Monitor {
  pub fn new(handle: HMONITOR, monitor_info: MONITORINFO) -> Self {
    Self {
      handle: handle.0 as isize,
      work_area: Rect::from(monitor_info.rcWork),
      monitor_area: Rect::from(monitor_info.rcMonitor),
      is_primary: monitor_info.dwFlags & 1 != 0,
    }
  }
}
