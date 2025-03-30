use crate::utils::{Direction, Point, Rect};
use std::fmt::Display;
use windows::Win32::Graphics::Gdi::{HMONITOR, MONITORINFO};

#[derive(Debug, Clone)]
pub struct Monitor {
  pub handle: isize,
  pub size: u32,
  pub is_primary: bool,
  /// Full monitor area including taskbar.
  pub monitor_area: Rect,
  /// Monitor work area i.e. excluding taskbar.
  pub work_area: Rect,
  /// The center of the monitor, calculated from the monitor area.
  pub center: Point,
}

impl Monitor {
  pub fn new(handle: HMONITOR, monitor_info: MONITORINFO) -> Self {
    let monitor_area = Rect::from(monitor_info.rcMonitor);
    Self {
      handle: handle.0 as isize,
      size: monitor_info.cbSize,
      work_area: Rect::from(monitor_info.rcWork),
      is_primary: monitor_info.dwFlags & 1 != 0,
      center: Point::from_center_of_rect(&monitor_area),
      monitor_area,
    }
  }

  /// Returns true if the monitor is in the given direction of the other monitor.
  pub fn is_in_direction_of(&self, other: &Monitor, direction: Direction) -> bool {
    match direction {
      Direction::Left => self.monitor_area.right < other.monitor_area.left,
      Direction::Right => other.monitor_area.right < self.monitor_area.left,
      Direction::Up => self.monitor_area.bottom < other.monitor_area.top,
      Direction::Down => other.monitor_area.bottom < self.monitor_area.top,
    }
  }
}

impl Display for Monitor {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "{} m#{} at ({}, {}) to ({}, {})",
      if self.is_primary { "Primary monitor" } else { "Monitor" },
      self.handle,
      self.monitor_area.left,
      self.monitor_area.top,
      self.monitor_area.right,
      self.monitor_area.bottom,
    )
  }
}
