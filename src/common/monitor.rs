use crate::common::{Direction, MonitorHandle, Point, Rect};
use crate::utils::id_to_string;
use std::fmt::Display;
use windows::Win32::Graphics::Gdi::MONITORINFO;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monitor {
  pub id: [u16; 32],
  pub handle: MonitorHandle,
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
  pub fn new(id: [u16; 32], handle: MonitorHandle, monitor_info: MONITORINFO) -> Self {
    let monitor_area = Rect::from(monitor_info.rcMonitor);
    Self {
      id,
      handle,
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

  pub fn id_to_string(&self) -> String {
    id_to_string(&self.id, &self.handle)
  }
}

impl Display for Monitor {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "{} {} at ({}, {}) to ({}, {})",
      if self.is_primary { "Primary monitor" } else { "Monitor" },
      self.id_to_string(),
      self.monitor_area.left,
      self.monitor_area.top,
      self.monitor_area.right,
      self.monitor_area.bottom,
    )
  }
}

#[cfg(test)]
mod tests {
  use crate::common::{Direction, Monitor, MonitorHandle, Point, Rect};

  impl Monitor {
    pub fn new_test(handle: isize, monitor_area: Rect) -> Self {
      let name = format!("DISPLAY {}", handle);
      Self {
        id: name
          .as_bytes()
          .iter()
          .map(|&b| b as u16)
          .chain(std::iter::repeat(0).take(32 - name.len()))
          .collect::<Vec<u16>>()
          .try_into()
          .unwrap(),
        handle: handle.into(),
        size: 0,
        is_primary: false,
        monitor_area,
        work_area: monitor_area,
        center: Point::from_center_of_rect(&monitor_area),
      }
    }

    pub fn mock_1() -> Self {
      Monitor {
        id: "DISPLAY1"
          .as_bytes()
          .iter()
          .map(|&b| b as u16)
          .chain(std::iter::repeat(0).take(32 - "DISPLAY1".len()))
          .collect::<Vec<u16>>()
          .try_into()
          .unwrap(),
        handle: MonitorHandle::from(1),
        size: 0,
        is_primary: true,
        monitor_area: Rect::new(0, 0, 1920, 1080),
        work_area: Rect::new(0, 0, 1920, 1030),
        center: Point::new(960, 540),
      }
    }

    pub fn mock_2() -> Self {
      Monitor {
        id: "DISPLAY2"
          .as_bytes()
          .iter()
          .map(|&b| b as u16)
          .chain(std::iter::repeat(0).take(32 - "DISPLAY2".len()))
          .collect::<Vec<u16>>()
          .try_into()
          .unwrap(),
        handle: MonitorHandle::from(2),
        size: 0,
        is_primary: false,
        monitor_area: Rect::new(-800, 0, 0, 600),
        work_area: Rect::new(-800, 0, 0, 550),
        center: Point::new(-400, 300),
      }
    }
  }

  #[test]
  fn display_formats_primary_monitor_correctly() {
    let monitor = Monitor::mock_1();

    let formatted = format!("{}", monitor);

    assert_eq!(formatted, "Primary monitor DISPLAY1 at (0, 0) to (1920, 1080)");
  }

  #[test]
  fn display_formats_non_primary_monitor_correctly() {
    let monitor = Monitor {
      id: "\\\\.\\DISPLAY10"
        .as_bytes()
        .iter()
        .map(|&b| b as u16)
        .chain(std::iter::repeat(0).take(32 - "\\\\.\\DISPLAY10".len()))
        .collect::<Vec<u16>>()
        .try_into()
        .unwrap(),
      handle: MonitorHandle::from(10),
      size: 0,
      is_primary: false,
      monitor_area: Rect::new(-800, 0, 0, 600),
      work_area: Rect::new(-800, 0, 0, 550),
      center: Point::new(-400, 300),
    };

    let formatted = format!("{}", monitor);

    assert_eq!(formatted, "Monitor \\\\.\\DISPLAY10 at (-800, 0) to (0, 600)");
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
