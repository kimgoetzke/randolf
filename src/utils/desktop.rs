use crate::utils::{Monitor, Window, WindowPlacement};
use std::fmt::Display;

#[derive(Debug)]
pub struct DesktopContainer {
  pub id: isize,
  pub layer: usize,
  pub monitor_id: i64,
  pub monitor: Monitor,
  pub window_info: Vec<(Window, WindowPlacement)>,
}

impl DesktopContainer {
  pub fn new(id: isize, layer: usize, monitor: &Monitor) -> Self {
    DesktopContainer {
      id,
      layer,
      monitor_id: monitor.handle as i64,
      monitor: monitor.clone(),
      window_info: vec![],
    }
  }

  fn store_windows(&mut self, windows: Vec<(Window, WindowPlacement)>) {
    self.window_info = windows;
  }

  fn clear_windows(&mut self) {
    self.window_info.clear();
  }
}

impl Display for DesktopContainer {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "DesktopContainer {{ id: {}, layer: {}, monitor_id: {}, is_primary_monitor: {} }}",
      self.id, self.layer, self.monitor_id, self.monitor.is_primary
    )
  }
}
