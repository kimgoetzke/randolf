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
      Direction::Left => self.monitor_area.right <= other.monitor_area.left,
      Direction::Right => other.monitor_area.right <= self.monitor_area.left,
      Direction::Up => self.monitor_area.bottom <= other.monitor_area.top,
      Direction::Down => other.monitor_area.bottom <= self.monitor_area.top,
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

#[cfg(test)]
mod tests {
  use crate::utils::{Direction, Monitor, Point, Rect};

  impl Monitor {
    pub fn new_test(handle: isize, monitor_area: Rect) -> Self {
      Self {
        handle,
        size: 0,
        is_primary: false,
        monitor_area,
        work_area: monitor_area,
        center: Point::from_center_of_rect(&monitor_area),
      }
    }

    pub fn mock_1() -> Self {
      Monitor {
        handle: 1,
        size: 0,
        is_primary: false,
        monitor_area: Rect::new(0, 0, 1920, 1080),
        work_area: Rect::new(0, 0, 1920, 1030),
        center: Point::new(960, 540),
      }
    }

    pub fn mock_2() -> Self {
      Monitor {
        handle: 2,
        size: 0,
        is_primary: false,
        monitor_area: Rect::new(-800, 600, 0, 0),
        work_area: Rect::new(-800, 550, 0, 0),
        center: Point::new(-400, 300),
      }
    }
  }

  #[test]
  fn is_in_direction_of_returns_true() {
    let monitor1 = Monitor::mock_1();
    let monitor2 = Monitor::mock_2();

    assert!(monitor2.is_in_direction_of(&monitor1, Direction::Left));
    assert!(monitor1.is_in_direction_of(&monitor2, Direction::Right));
  }

  #[test]
  fn is_in_direction_of_returns_false() {
    let monitor1 = Monitor::new_test(1, Rect::new(0, 0, 1920, 1080));
    let monitor2 = Monitor::new_test(2, Rect::new(1920, 0, 3840, 1080));

    assert!(!monitor1.is_in_direction_of(&monitor2, Direction::Right));
    assert!(!monitor1.is_in_direction_of(&monitor2, Direction::Up));
    assert!(!monitor1.is_in_direction_of(&monitor2, Direction::Down));
  }

  #[test]
  fn is_in_direction_of_returns_false_for_if_no_other_monitors() {
    let monitor1 = Monitor::mock_1();

    assert!(!monitor1.is_in_direction_of(&monitor1, Direction::Left));
    assert!(!monitor1.is_in_direction_of(&monitor1, Direction::Right));
    assert!(!monitor1.is_in_direction_of(&monitor1, Direction::Up));
    assert!(!monitor1.is_in_direction_of(&monitor1, Direction::Down));
  }
}
