use crate::api::NativeApi;
use crate::utils::{Monitor, Window, WindowHandle};
use std::fmt::Display;

#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct WorkspaceId {
  pub monitor_handle: isize,
  pub workspace: usize,
}

impl WorkspaceId {
  pub fn from(monitor_handle: isize, workspace: usize) -> Self {
    WorkspaceId {
      monitor_handle,
      workspace,
    }
  }

  pub fn is_same_monitor(&self, other: &Self) -> bool {
    self.monitor_handle == other.monitor_handle
  }

  pub fn is_same_workspace(&self, other: &Self) -> bool {
    self.workspace == other.workspace
  }
}

impl Display for WorkspaceId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "s#{}-{}", self.monitor_handle, self.workspace)
  }
}

#[derive(Debug, Clone)]
pub struct Workspace {
  pub id: WorkspaceId,
  pub monitor_handle: i64,
  pub monitor: Monitor,
  windows: Vec<Window>,
  window_state_info: Vec<(WindowHandle, bool)>, // (window_handle, is_minimised)
}

impl Workspace {
  pub fn from(id: WorkspaceId, monitor: &Monitor) -> Self {
    Workspace {
      id,
      monitor_handle: monitor.handle as i64,
      monitor: monitor.clone(),
      windows: vec![],
      window_state_info: vec![],
    }
  }

  pub fn store_and_hide_window(&mut self, window: Window, windows_api: &impl NativeApi) {
    if !self.windows.iter().any(|w| w.handle == window.handle) {
      let is_minimised = windows_api.is_window_minimised(window.handle);
      if !is_minimised {
        windows_api.do_hide_window(window.handle);
      }
      self.window_state_info.push((window.handle, is_minimised));
      self.windows.push(window);
    } else {
      warn!("Window {} already exists in workspace {}", window.handle, self.id);
    }
  }

  pub fn store_and_hide_windows(&mut self, windows: Vec<Window>, windows_api: &impl NativeApi) {
    self.clear_windows();
    for window in windows.iter() {
      self.store_and_hide_window(window.clone(), windows_api);
    }
  }

  fn clear_windows(&mut self) {
    self.windows.clear();
    self.window_state_info.clear();
  }

  pub fn restore_windows(&mut self, api: &impl NativeApi) {
    if self.windows.is_empty() && self.window_state_info.is_empty() {
      debug!("No windows to restore for workspace {}", self.id);
      return;
    }
    if !(!self.windows.is_empty() && !self.window_state_info.is_empty()) {
      warn!(
        "Data inconsistency detected: {} stores [{}] window(s) but [{}] window state(s)",
        self.id,
        self.windows.len(),
        self.window_state_info.len()
      );
      return;
    }
    for (window_handle, is_minimised) in self.window_state_info.iter() {
      if !*is_minimised {
        match self.windows.iter().find(|w| w.handle == *window_handle) {
          Some(window) => {
            if api.is_window_hidden(&window.handle) {
              api.do_restore_window(window, is_minimised);
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
      "Workspace {{ id: {}, monitor_handle: {}, is_primary_monitor: {} }}",
      self.id, self.monitor_handle, self.monitor.is_primary
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api::MockWindowsApi;
  use crate::utils::{Monitor, Rect, Window};

  impl Workspace {
    pub fn get_windows(&self) -> Vec<Window> {
      self.windows.clone()
    }

    pub fn get_window_state_info(&self) -> Vec<(WindowHandle, bool)> {
      self.window_state_info.clone()
    }
  }

  #[test]
  fn workspace_can_store_window() {
    let monitor = Monitor::new_test(1, Rect::default());
    let workspace_id = WorkspaceId::from(1, 1);
    let mut workspace = Workspace::from(workspace_id, &monitor);

    let window = Window::from(1, "Test Window".to_string(), Rect::new(0, 0, 100, 100));
    workspace.store_and_hide_window(window.clone(), &MockWindowsApi::new());

    assert_eq!(workspace.windows.len(), 1);
    assert_eq!(workspace.windows[0].title, "Test Window");
    assert_eq!(workspace.windows[0].handle, window.handle);
    assert_eq!(workspace.windows[0].rect, Rect::new(0, 0, 100, 100));
    assert_eq!(workspace.window_state_info[0].0, window.handle);
    assert!(!workspace.window_state_info[0].1);
  }

  #[test]
  fn workspace_id_same_monitor_returns_true() {
    let id1 = WorkspaceId::from(1, 1);
    let id2 = WorkspaceId::from(1, 2);

    assert!(id1.is_same_monitor(&id2));
  }

  #[test]
  fn workspace_id_different_monitor_returns_false() {
    let id1 = WorkspaceId::from(1, 1);
    let id2 = WorkspaceId::from(2, 1);

    assert!(!id1.is_same_monitor(&id2));
  }

  #[test]
  fn workspace_id_same_workspace_returns_true() {
    let id1 = WorkspaceId::from(1, 1);
    let id2 = WorkspaceId::from(1, 1);

    assert!(id1.is_same_workspace(&id2));
  }

  #[test]
  fn workspace_id_different_workspace_returns_false() {
    let id1 = WorkspaceId::from(1, 1);
    let id2 = WorkspaceId::from(1, 2);

    assert!(!id1.is_same_workspace(&id2));
  }

  #[test]
  fn store_and_hide_window_adds_window_to_workspace() {
    let mut workspace = Workspace::from(WorkspaceId::from(1, 1), &Monitor::mock_1());
    let window = Window::from(1, "Test Window".to_string(), Rect::new(0, 0, 100, 100));
    let mock_api = MockWindowsApi;

    workspace.store_and_hide_window(window.clone(), &mock_api);

    assert_eq!(workspace.get_windows().len(), 1);
    assert_eq!(workspace.get_windows()[0].handle, window.handle);
  }

  #[test]
  fn store_and_hide_window_does_not_add_duplicate_window() {
    let mut workspace = Workspace::from(WorkspaceId::from(1, 1), &Monitor::mock_1());
    let window = Window::from(1, "Test Window".to_string(), Rect::new(0, 0, 100, 100));
    let mock_api = MockWindowsApi;

    workspace.store_and_hide_window(window.clone(), &mock_api);
    workspace.store_and_hide_window(window.clone(), &mock_api);

    assert_eq!(workspace.get_windows().len(), 1);
  }

  #[test]
  fn store_and_hide_windows_adds_windows_to_workspace() {
    let mut workspace = Workspace::from(WorkspaceId::from(1, 1), &Monitor::mock_1());
    let window_1 = Window::from(1, "Test Window 1".to_string(), Rect::new(0, 0, 100, 100));
    let window_2 = Window::from(2, "Test Window 2".to_string(), Rect::new(100, 100, 200, 200));
    let mock_api = MockWindowsApi;

    workspace.store_and_hide_windows(vec![window_1.clone(), window_2.clone()], &mock_api);

    assert_eq!(workspace.get_windows().len(), 2);
    assert!(workspace.get_windows().contains(&window_1));
    assert!(workspace.get_windows().contains(&window_2));
  }

  #[test]
  fn restore_windows_restores_all_windows() {
    let mut workspace = Workspace::from(WorkspaceId::from(1, 1), &Monitor::mock_1());
    let window_1 = Window::from(1, "Test Window 1".to_string(), Rect::new(0, 0, 100, 100));
    let window_2 = Window::from(2, "Test Window 2".to_string(), Rect::new(100, 100, 200, 200));
    MockWindowsApi::set_is_window_hidden(window_1.handle, false);
    MockWindowsApi::set_is_window_hidden(window_2.handle, false);
    let mock_api = MockWindowsApi;

    workspace.store_and_hide_windows(vec![window_1.clone(), window_2.clone()], &mock_api);
    workspace.restore_windows(&mock_api);

    assert!(workspace.get_windows().is_empty());
  }

  #[test]
  fn restore_windows_handles_empty_workspace() {
    let mut workspace = Workspace::from(WorkspaceId::from(1, 1), &Monitor::mock_1());
    let mock_api = MockWindowsApi;

    workspace.restore_windows(&mock_api);

    assert!(workspace.get_windows().is_empty());
  }
}
