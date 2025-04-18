use crate::api::WindowsApi;
use crate::utils::{Monitor, MonitorHandle, Rect, Window, WindowHandle, WorkspaceId};
use std::fmt::Display;

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
      monitor_handle: monitor.handle.handle as i64,
      monitor: monitor.clone(),
      windows: vec![],
      window_state_info: vec![],
    }
  }

  pub fn move_window(&mut self, mut window: Window, current_monitor: MonitorHandle, windows_api: &impl WindowsApi) {
    window = self.update_window_rect_if_required(window, current_monitor, windows_api);
    windows_api.set_window_position(window.handle, window.rect);
    windows_api.set_cursor_position(&window.rect.center());
    debug!(
      "Moved {} {} to active workspace {}",
      window.handle,
      window.title_trunc(),
      self.id
    );
  }

  pub fn stores(&self, handle: &WindowHandle) -> bool {
    self.windows.iter().any(|window| window.handle == *handle)
  }

  pub fn store_and_hide_window(
    &mut self,
    mut window: Window,
    current_monitor: MonitorHandle,
    windows_api: &impl WindowsApi,
  ) {
    if !self.windows.iter().any(|w| w.handle == window.handle) {
      if windows_api.is_window_minimised(window.handle) {
        debug!("{} is minimised, ignoring it for workspace {}", window.handle, self.id);
        return;
      }
      window = self.update_window_rect_if_required(window, current_monitor, windows_api);
      windows_api.do_hide_window(window.handle);
      self.window_state_info.push((window.handle, false));
      self.windows.push(window);
    } else {
      warn!("{} already exists in workspace {}", window.handle, self.id);
    }
  }

  // TODO: Check if window is snapped by Randolf - if yes, replicate on new monitor
  fn update_window_rect_if_required(
    &mut self,
    mut window: Window,
    current_monitor: MonitorHandle,
    _windows_api: &impl WindowsApi,
  ) -> Window {
    if self.monitor_handle != current_monitor.as_i64() {
      let width = window.rect.width();
      let height = window.rect.height();
      let target_monitor_work_area_center = self.monitor.work_area.center();
      let left = target_monitor_work_area_center.x() - (width / 2);
      let top = target_monitor_work_area_center.y() - (height / 2);
      let old_rect = window.rect;
      window.rect = Rect::new(left, top, left + width, top + height).clamp(&self.monitor.work_area, 10);
      window.center = window.rect.center();

      debug!(
        "Because {} is being moved to a different monitor, its placement was updated from {} to {}",
        window.handle, old_rect, window.rect
      );
    }

    window
  }

  pub fn store_and_hide_windows(
    &mut self,
    windows: Vec<Window>,
    current_monitor: MonitorHandle,
    windows_api: &impl WindowsApi,
  ) {
    self.clear_windows();
    for window in windows.iter() {
      self.store_and_hide_window(window.clone(), current_monitor, windows_api);
    }
  }

  fn clear_windows(&mut self) {
    self.windows.clear();
    self.window_state_info.clear();
  }

  pub fn get_largest_window(&self) -> Option<Window> {
    self.windows.iter().max_by_key(|w| w.rect.area()).cloned().to_owned()
  }

  pub fn restore_windows(&mut self, api: &impl WindowsApi) {
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
    let mut i = 0;
    for (window_handle, is_minimised) in self.window_state_info.iter() {
      i += 1;
      if !*is_minimised {
        match self.windows.iter().find(|w| w.handle == *window_handle) {
          Some(window) => {
            if api.is_window_hidden(&window.handle) {
              debug!(
                "Restoring {} {} on workspace {}",
                window.handle,
                window.title_trunc(),
                self.id
              );
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
    debug!("Restored [{}] window(s) on workspace {}", i, self.id);
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
  use crate::utils::{Monitor, Rect, Sizing, Window};

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
    MockWindowsApi::add_or_update_window(window.handle, window.title.clone(), window.rect.into(), false, false, true);

    workspace.store_and_hide_window(window.clone(), 1.into(), &MockWindowsApi::new());

    assert_eq!(workspace.windows.len(), 1);
    assert_eq!(workspace.windows[0].title, "Test Window");
    assert_eq!(workspace.windows[0].handle, window.handle);
    assert_eq!(workspace.windows[0].rect, Rect::new(0, 0, 100, 100));
    assert_eq!(workspace.window_state_info[0].0, window.handle);
    assert!(!workspace.window_state_info[0].1);
  }

  #[test]
  fn store_and_hide_window_adds_window_to_workspace() {
    let mut workspace = Workspace::from(WorkspaceId::from(1, 1), &Monitor::mock_1());
    let window = Window::from(1, "Test Window".to_string(), Rect::new(0, 0, 100, 100));
    MockWindowsApi::add_or_update_window(window.handle, window.title.clone(), window.rect.into(), false, false, true);

    workspace.store_and_hide_window(window.clone(), 1.into(), &MockWindowsApi);

    assert_eq!(workspace.get_windows().len(), 1);
    assert_eq!(workspace.get_windows()[0].handle, window.handle);
  }

  #[test]
  fn store_and_hide_window_does_not_add_duplicate_window() {
    let mut workspace = Workspace::from(WorkspaceId::from(1, 1), &Monitor::mock_1());
    let window = Window::from(1, "Test Window".to_string(), Rect::new(0, 0, 100, 100));
    MockWindowsApi::add_or_update_window(window.handle, window.title.clone(), window.rect.into(), false, false, true);
    let mock_api = MockWindowsApi;

    workspace.store_and_hide_window(window.clone(), 1.into(), &mock_api);
    workspace.store_and_hide_window(window.clone(), 1.into(), &mock_api);

    assert_eq!(workspace.get_windows().len(), 1);
  }

  #[test]
  fn store_and_hide_windows_adds_windows_to_workspace() {
    let mut workspace = Workspace::from(WorkspaceId::from(1, 1), &Monitor::mock_1());
    let window_1 = Window::from(1, "Test Window 1".to_string(), Rect::new(0, 0, 100, 100));
    let window_2 = Window::from(2, "Test Window 2".to_string(), Rect::new(100, 100, 200, 200));
    let mock_api = MockWindowsApi;
    MockWindowsApi::add_or_update_window(
      window_1.handle,
      window_1.title.clone(),
      window_1.rect.into(),
      false,
      false,
      true,
    );
    MockWindowsApi::add_or_update_window(
      window_2.handle,
      window_2.title.clone(),
      window_2.rect.into(),
      false,
      false,
      true,
    );

    workspace.store_and_hide_windows(vec![window_1.clone(), window_2.clone()], 1.into(), &mock_api);

    assert_eq!(workspace.get_windows().len(), 2);
    assert!(workspace.get_windows().contains(&window_1));
    assert!(workspace.get_windows().contains(&window_2));
  }

  #[test]
  fn restore_windows_restores_all_windows() {
    let mut workspace = Workspace::from(WorkspaceId::from(1, 1), &Monitor::mock_1());
    let sizing_window_1 = Sizing::new(0, 0, 100, 100);
    let sizing_window_2 = Sizing::new(100, 100, 100, 100);
    MockWindowsApi::add_or_update_window(1.into(), "Test Window 1".to_string(), sizing_window_1, false, false, true);
    MockWindowsApi::add_or_update_window(2.into(), "Test Window 2".to_string(), sizing_window_2, false, false, true);
    let mock_api = MockWindowsApi;
    let windows = mock_api.get_all_visible_windows();

    workspace.store_and_hide_windows(windows, 1.into(), &mock_api);
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
