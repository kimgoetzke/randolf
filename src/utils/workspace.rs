use crate::native_api;
use crate::utils::{Monitor, Window, WindowHandle};
use std::fmt::Display;

#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct WorkspaceId {
  pub monitor_id: isize,
  pub workspace: usize,
}

impl WorkspaceId {
  pub fn from(monitor_id: isize, workspace: usize) -> Self {
    WorkspaceId { monitor_id, workspace }
  }

  pub fn is_same_monitor(&self, other: &Self) -> bool {
    self.monitor_id == other.monitor_id
  }

  pub fn is_same_workspace(&self, other: &Self) -> bool {
    self.workspace == other.workspace
  }
}

impl Display for WorkspaceId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "s#{}-{}", self.monitor_id, self.workspace)
  }
}

#[derive(Debug, Clone)]
pub struct Workspace {
  pub id: WorkspaceId,
  pub monitor_id: i64,
  pub monitor: Monitor,
  windows: Vec<Window>,
  window_state_info: Vec<(WindowHandle, bool, bool)>, // (window_handle, is_minimised, is_maximised)
}

impl Workspace {
  pub fn from(id: WorkspaceId, monitor: &Monitor) -> Self {
    Workspace {
      id,
      monitor_id: monitor.handle as i64,
      monitor: monitor.clone(),
      windows: vec![],
      window_state_info: vec![],
    }
  }

  pub fn store_and_hide_windows(&mut self, windows: Vec<Window>) {
    self.clear_windows();
    let mut window_state_info = vec![];

    for window in windows.iter() {
      let (is_minimised, is_maximised) = native_api::get_window_minimised_maximised_state(window.handle);
      window_state_info.push((window.handle, is_minimised, is_maximised));
      if !is_minimised {
        native_api::hide_window(window.handle);
      }
    }

    self.windows = windows;
    self.window_state_info = window_state_info;
  }

  fn clear_windows(&mut self) {
    self.windows.clear();
    self.window_state_info.clear();
  }

  pub fn restore_windows(&mut self) {
    if self.windows.is_empty() && self.window_state_info.is_empty() {
      debug!("No windows to restore for workspace {}", self.id);
      return;
    }
    if !(!self.windows.is_empty() && !self.window_state_info.is_empty()) {
      warn!(
        "Data inconsistency detected: {} stores [{}] windows but [{}] window states",
        self.id,
        self.windows.len(),
        self.window_state_info.len()
      );
      return;
    }
    for (window_handle, is_minimised, _) in self.window_state_info.iter() {
      if !*is_minimised {
        match self.windows.iter().find(|w| w.handle == *window_handle) {
          Some(window) => {
            if window.is_hidden() {
              native_api::restore_window(window, is_minimised);
            } else {
              debug!("Attempted to restore window {} but it is already visible", window_handle);
            }
          }
          None => {
            warn!("Attempted to restore window {window_handle} but workspace manager doesn't recognise it");
          }
        }
      }
    }
    self.clear_windows();
  }
}

impl Display for Workspace {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "Workspace {{ id: {}, monitor_id: {}, is_primary_monitor: {} }}",
      self.id, self.monitor_id, self.monitor.is_primary
    )
  }
}
