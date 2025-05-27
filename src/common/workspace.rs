use crate::api::WindowsApi;
use crate::common::{Monitor, MonitorHandle, PersistentWorkspaceId, Rect, Sizing, Window, WindowHandle, WorkspaceAction};
use std::fmt::Display;

/// Represents a Randolf workspace, which is a collection of zero or more windows that are managed together on a
/// specific monitor's desktop. Will only ever store windows if the workspace is inactive but is also used to position
/// a window on the monitor's desktop which it represents while in an active state.
#[derive(Debug, Clone)]
pub struct Workspace {
  pub id: PersistentWorkspaceId,
  pub monitor_handle: i64,
  pub monitor: Monitor,
  windows: Vec<Window>,
  minimised_windows: Vec<(WindowHandle, bool)>, // (window_handle, is_minimised)
  margin: i32,
  is_active: bool,
}

impl Workspace {
  /// Creates a new, empty workspace with the specified ID and monitor that is marked as active.
  pub fn new_active(id: PersistentWorkspaceId, monitor: &Monitor, margin: i32) -> Self {
    Workspace {
      id,
      monitor_handle: monitor.handle.handle as i64,
      monitor: monitor.clone(),
      windows: vec![],
      minimised_windows: vec![],
      margin,
      is_active: true,
    }
  }

  /// Creates a new, empty workspace with the specified ID and monitor that is marked as inactive.
  pub fn new_inactive(id: PersistentWorkspaceId, monitor: &Monitor, margin: i32) -> Self {
    Workspace {
      id,
      monitor_handle: monitor.handle.handle as i64,
      monitor: monitor.clone(),
      windows: vec![],
      minimised_windows: vec![],
      margin,
      is_active: false,
    }
  }

  /// Returns `true` if the workspace is active.
  pub fn is_active(&self) -> bool {
    self.is_active
  }

  /// Sets the workspace as active (if `true`) or inactive (if `false`).
  pub fn set_active(&mut self, is_active: bool) {
    self.is_active = is_active;
  }

  /// Allows you to update the `MonitorHandle`, which is a non-persistent identifier of a monitor, for this workspace.
  /// Must be called prior to interacting with the workspace.
  pub fn update_handle(&mut self, monitor_handle: MonitorHandle) {
    self.monitor_handle = monitor_handle.handle as i64;
  }

  /// Returns the largest window in the workspace or `None` if none is present. The largest window is defined as the
  /// one covering with the largest area. If multiple windows have the same area, the first one found is returned (not
  /// deterministic).
  pub fn get_largest_window(&self) -> Option<Window> {
    self.windows.iter().max_by_key(|w| w.rect.area()).cloned().to_owned()
  }

  /// Moves the window if the workspace is active, otherwise stores and hides it, so that it can be restored later,
  /// when the workspace is activated, so that an active workspace must never store windows.
  pub fn move_or_store_and_hide_window(
    &mut self,
    window: Window,
    current_monitor: MonitorHandle,
    windows_api: &impl WindowsApi,
  ) -> WorkspaceAction {
    if self.is_active {
      self.move_window(window, current_monitor, windows_api);

      WorkspaceAction::Moved
    } else {
      self.store_and_hide_window(window, current_monitor, windows_api);

      WorkspaceAction::Stored
    }
  }

  /// Returns `true` if the workspace stores the specified window.
  pub fn stores(&self, handle: &WindowHandle) -> bool {
    self.windows.iter().any(|window| window.handle == *handle)
  }

  /// Stores and hides the specified windows. Clears the list of stored windows before storing the new ones.
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

  /// Removes the specified windows from the workspace. This method should be called after switching workspace and after
  /// a window is moved to a workspace to ensure windows don't exist in multiple workspaces. The reason why this is
  /// currently important is that Randolf does not listen to window events. The application does not know, for example,
  /// when another application has moved a hidden window from a workspace into the foreground. In order to allow the
  /// user to then move/hide this window again without Randolf storing it in multiple workspaces, this method is
  /// required to be called on every action that changes the window state in relation to a workspace.
  pub fn remove_windows_if_present(&mut self, windows: &[Window]) {
    for window in windows.iter() {
      self.windows.retain(|w| w.handle != window.handle);
      self.minimised_windows.retain(|(w, _)| *w != window.handle);
    }
  }

  /// Restores all windows that were stored in this workspace by unhiding them. Clears the list of stored windows
  /// after restoring.
  pub fn restore_windows(&mut self, api: &impl WindowsApi) {
    if self.windows.is_empty() && self.minimised_windows.is_empty() {
      debug!("No windows to restore for workspace [{}]", self.id);
      return;
    }
    if self.windows.len() != self.minimised_windows.len() {
      error!(
        "Data inconsistency detected: {} stores [{}] window(s) but [{}] window state(s)",
        self.id,
        self.windows.len(),
        self.minimised_windows.len()
      );
      return;
    }
    let mut i = 0;
    for (window_handle, is_minimised) in self.minimised_windows.iter() {
      i += 1;
      if *is_minimised {
        continue;
      }
      match self.windows.iter().find(|w| w.handle == *window_handle) {
        Some(window) => {
          if api.is_window_hidden(&window.handle) {
            debug!(
              "Restoring {} \"{}\" on workspace [{}]",
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
    debug!("Restored [{}] window(s) on workspace [{}]", i, self.id);
    self.clear_windows();
  }

  fn move_window(&mut self, mut window: Window, current_monitor_handle: MonitorHandle, windows_api: &impl WindowsApi) {
    window = self.update_window_rect_if_required(window, current_monitor_handle, windows_api);
    if current_monitor_handle != self.monitor.handle {
      windows_api.set_window_position(window.handle, window.rect);
      std::thread::sleep(std::time::Duration::from_millis(10));
    }
    windows_api.set_window_position(window.handle, window.rect);
    windows_api.set_cursor_position(&window.rect.center());
    trace!(
      "Moved {} \"{}\" to active workspace [{}]",
      window.handle,
      window.title_trunc(),
      self.id
    );
  }

  fn store_and_hide_window(&mut self, mut window: Window, current_monitor: MonitorHandle, windows_api: &impl WindowsApi) {
    if !self.windows.iter().any(|w| w.handle == window.handle) {
      if windows_api.is_window_minimised(window.handle) {
        debug!("{} is minimised, ignoring it for workspace [{}]", window.handle, self.id);
        return;
      }
      window = self.update_window_rect_if_required(window, current_monitor, windows_api);
      windows_api.do_hide_window(window.handle);
      self.minimised_windows.push((window.handle, false));
      self.windows.push(window.clone());
      trace!(
        "Stored and hid {} \"{}\" in workspace [{}]",
        window.handle,
        window.title_trunc(),
        self.id
      );
    } else {
      warn!(
        "{} already exists in workspace [{}], ignoring request",
        window.handle, self.id
      );
    }
  }

  fn update_window_rect_if_required(
    &mut self,
    mut window: Window,
    current_monitor: MonitorHandle,
    windows_api: &impl WindowsApi,
  ) -> Window {
    if self.monitor_handle == current_monitor.as_i64() {
      return window;
    }

    // Check if window was near maximised or near-snapped on current monitor
    let new_sizing = if let Some(monitor_info) = windows_api.get_monitor_info_for_monitor(current_monitor) {
      let current_monitor_work_area = monitor_info.work_area;
      let current_sizing = Sizing::from(window.rect);
      match current_sizing {
        sizing if sizing == Sizing::near_maximised(current_monitor_work_area, self.margin) => {
          Some(Sizing::near_maximised(self.monitor.work_area, self.margin))
        }
        sizing if sizing == Sizing::left_half_of_screen(current_monitor_work_area, self.margin) => {
          Some(Sizing::left_half_of_screen(self.monitor.work_area, self.margin))
        }
        sizing if sizing == Sizing::right_half_of_screen(current_monitor_work_area, self.margin) => {
          Some(Sizing::right_half_of_screen(self.monitor.work_area, self.margin))
        }
        sizing if sizing == Sizing::top_half_of_screen(current_monitor_work_area, self.margin) => {
          Some(Sizing::top_half_of_screen(self.monitor.work_area, self.margin))
        }
        sizing if sizing == Sizing::bottom_half_of_screen(current_monitor_work_area, self.margin) => {
          Some(Sizing::bottom_half_of_screen(self.monitor.work_area, self.margin))
        }
        _ => None,
      }
    } else {
      error!(
        "Unable to get monitor info for current monitor {}, cannot detect if window was near-maximised or -snapped",
        current_monitor
      );

      None
    };

    let old_rect = window.rect;
    if let Some(new_sizing) = new_sizing {
      debug!("{} is currently near-maximised or -snapped", window.handle);
      window.rect = new_sizing.into();
    } else {
      debug!("{} is currently NOT near-maximised or -snapped", window.handle);
      let width = window.rect.width();
      let height = window.rect.height();
      let target_monitor_work_area_center = self.monitor.work_area.center();
      let left = target_monitor_work_area_center.x() - (width / 2);
      let top = target_monitor_work_area_center.y() - (height / 2);
      window.rect = Rect::new(left, top, left + width, top + height).clamp(&self.monitor.work_area, 10);
    }

    window.center = window.rect.center();
    trace!(
      "{} is being moved to a different monitor, its location was updated from {} to {}",
      window.handle, old_rect, window.rect
    );

    window
  }

  fn clear_windows(&mut self) {
    self.windows.clear();
    self.minimised_windows.clear();
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
  use crate::common::{Monitor, Rect, Sizing, Window};

  impl Workspace {
    /// Creates a new workspace for testing purposes with margin set to 0 and inactive by default.
    pub fn new_test(id: PersistentWorkspaceId, monitor: &Monitor) -> Self {
      Workspace {
        id,
        monitor_handle: monitor.handle.handle as i64,
        monitor: monitor.clone(),
        windows: vec![],
        minimised_windows: vec![],
        margin: 0,
        is_active: false,
      }
    }

    pub fn get_windows(&self) -> Vec<Window> {
      self.windows.clone()
    }

    pub fn get_window_state_info(&self) -> Vec<(WindowHandle, bool)> {
      self.minimised_windows.clone()
    }
  }

  #[test]
  fn update_window_rect_if_required_returns_window_unchanged_when_staying_on_same_monitor() {
    let monitor = Monitor::new_test(1, Rect::new(0, 0, 1920, 1080));
    let mut workspace = Workspace::new_test(PersistentWorkspaceId::new(monitor.id, 1, true), &monitor);
    let window = Window::new_test(1, Rect::new(10, 10, 110, 110));
    let mock_api = MockWindowsApi::new();

    let updated_window = workspace.update_window_rect_if_required(window.clone(), monitor.handle, &mock_api);

    assert_eq!(updated_window.rect, window.rect);
  }

  #[test]
  fn update_window_rect_if_required_maintains_near_maximised_layout_when_changing_monitors() {
    let target_monitor = Monitor::new_test(2, Rect::new(0, 0, 1920, 1080));
    let mut workspace = Workspace::new_test(PersistentWorkspaceId::new(target_monitor.id, 1, true), &target_monitor);
    let current_monitor_handle = MonitorHandle::from(1);
    MockWindowsApi::add_monitor(current_monitor_handle, Rect::new(0, 0, 800, 600), true);
    let mock_api = MockWindowsApi::new();
    let current_monitor = mock_api.get_monitor_info_for_monitor(current_monitor_handle).unwrap();
    let current_sizing_near_maximised = Sizing::near_maximised(current_monitor.work_area, workspace.margin);
    let window = Window::new_test(1, current_sizing_near_maximised.into());

    let updated_window = workspace.update_window_rect_if_required(window, current_monitor_handle, &mock_api);

    let expected_sizing = Sizing::near_maximised(target_monitor.work_area, workspace.margin);
    assert_eq!(updated_window.rect, expected_sizing.into());
  }

  #[test]
  fn update_window_rect_if_required_maintains_left_half_layout_when_changing_monitors() {
    let target_monitor = Monitor::new_test(2, Rect::new(0, 0, 1920, 1080));
    let mut workspace = Workspace::new_test(PersistentWorkspaceId::new(target_monitor.id, 1, true), &target_monitor);
    let current_monitor_handle = MonitorHandle::from(1);
    MockWindowsApi::add_monitor(current_monitor_handle, Rect::new(0, 0, 800, 600), true);
    let mock_api = MockWindowsApi::new();
    let current_monitor = mock_api.get_monitor_info_for_monitor(current_monitor_handle).unwrap();
    let current_sizing_left_half = Sizing::left_half_of_screen(current_monitor.work_area, workspace.margin);
    let window = Window::new_test(1, current_sizing_left_half.into());

    let updated_window = workspace.update_window_rect_if_required(window, current_monitor_handle, &mock_api);

    let expected_sizing = Sizing::left_half_of_screen(target_monitor.work_area, workspace.margin);
    assert_eq!(updated_window.rect, expected_sizing.into());
  }

  #[test]
  fn update_window_rect_if_required_centers_normal_window_when_changing_monitors() {
    let source_monitor = Monitor::new_test(1, Rect::new(0, 0, 1000, 800));
    let target_monitor = Monitor::new_test(2, Rect::new(0, 0, 1920, 1080));
    let mut workspace = Workspace::new_test(PersistentWorkspaceId::new(target_monitor.id, 1, true), &target_monitor);
    let window = Window::new_test(1, Rect::new(100, 100, 300, 200));
    MockWindowsApi::add_monitor(source_monitor.handle, source_monitor.monitor_area, true);
    let mock_api = MockWindowsApi::new();

    let updated_window = workspace.update_window_rect_if_required(window, source_monitor.handle, &mock_api);

    assert_eq!(updated_window.rect.width(), 200);
    assert_eq!(updated_window.rect.height(), 100);
    assert_eq!(updated_window.center, target_monitor.work_area.center());
  }

  #[test]
  fn update_window_rect_if_required_centers_window_when_monitor_info_missing() {
    let source_monitor = Monitor::new_test(1, Rect::new(0, 0, 1024, 768));
    let target_monitor = Monitor::new_test(2, Rect::new(0, 0, 1920, 1080));
    let mut workspace = Workspace::new_test(PersistentWorkspaceId::new(target_monitor.id, 1, true), &target_monitor);
    let current_sizing_near_maximised = Sizing::near_maximised(source_monitor.work_area, workspace.margin);
    let window = Window::new_test(1, current_sizing_near_maximised.into());
    let mock_api = MockWindowsApi::new();

    let updated_window = workspace.update_window_rect_if_required(window.clone(), source_monitor.handle, &mock_api);

    assert_eq!(updated_window.rect.width(), 1024);
    assert_eq!(updated_window.rect.height(), 768);
    assert_eq!(updated_window.center, target_monitor.work_area.center());
  }

  #[test]
  fn move_or_store_and_hide_window_stores_window_if_workspace_is_inactive() {
    let monitor = Monitor::new_test(1, Rect::default());
    let workspace_id = PersistentWorkspaceId::new(monitor.id, 1, true);
    let mut workspace = Workspace::new_test(workspace_id, &monitor); // Inactive by default
    let window = Window::new_test(1, Rect::new(0, 0, 100, 100));
    MockWindowsApi::add_or_update_window(window.handle, window.title.clone(), window.rect.into(), false, false, true);
    let mock_api = MockWindowsApi::new();

    workspace.move_or_store_and_hide_window(window.clone(), monitor.handle, &mock_api);

    assert_eq!(mock_api.get_all_visible_windows().len(), 0);
    assert_eq!(workspace.windows.len(), 1);
    assert_eq!(workspace.minimised_windows.len(), 1);
  }

  #[test]
  fn move_or_store_and_hide_window_moves_window_if_workspace_is_active() {
    let monitor = Monitor::new_test(1, Rect::default());
    let workspace_id = PersistentWorkspaceId::new(monitor.id, 1, true);
    let mut workspace = Workspace::new_active(workspace_id, &monitor, 20);
    let window = Window::new_test(1, Rect::new(0, 0, 100, 100));
    MockWindowsApi::add_or_update_window(window.handle, window.title.clone(), window.rect.into(), false, false, true);
    let mock_api = MockWindowsApi::new();

    workspace.move_or_store_and_hide_window(window.clone(), monitor.handle, &mock_api);

    let visible_windows = mock_api.get_all_visible_windows();
    assert_eq!(visible_windows.len(), 1);
    assert_eq!(visible_windows[0].handle, window.handle);
    assert!(workspace.windows.is_empty());
    assert_eq!(workspace.minimised_windows.len(), 0);
  }

  #[test]
  fn store_and_hide_window_stores_and_hide_window() {
    let monitor = Monitor::new_test(1, Rect::default());
    let mut workspace = Workspace::new_test(PersistentWorkspaceId::new(monitor.id, 1, true), &monitor);
    let window = Window::new_test(1, Rect::new(0, 0, 100, 100));
    MockWindowsApi::add_or_update_window(window.handle, window.title.clone(), window.rect.into(), false, false, true);
    let mock_api = MockWindowsApi::new();

    workspace.store_and_hide_window(window.clone(), monitor.handle, &mock_api);

    assert_eq!(mock_api.get_all_visible_windows().len(), 0);
    assert_eq!(workspace.windows.len(), 1);
    assert_eq!(workspace.windows[0].title, window.title);
    assert_eq!(workspace.windows[0].handle, window.handle);
    assert_eq!(workspace.windows[0].rect, Rect::new(0, 0, 100, 100));
    assert_eq!(workspace.minimised_windows[0].0, window.handle);
    assert!(!workspace.minimised_windows[0].1);
  }

  #[test]
  fn store_and_hide_window_does_not_add_duplicate_window() {
    let mut workspace = Workspace::new_test(PersistentWorkspaceId::new_test(1), &Monitor::mock_1());
    let window = Window::new_test(1, Rect::new(0, 0, 100, 100));
    MockWindowsApi::add_or_update_window(window.handle, window.title.clone(), window.rect.into(), false, false, true);
    let mock_api = MockWindowsApi;

    workspace.store_and_hide_window(window.clone(), 1.into(), &mock_api);
    workspace.store_and_hide_window(window.clone(), 1.into(), &mock_api);

    assert_eq!(workspace.get_windows().len(), 1);
  }

  #[test]
  fn store_and_hide_windows_adds_windows_to_workspace() {
    let mut workspace = Workspace::new_test(PersistentWorkspaceId::new_test(1), &Monitor::mock_1());
    let window_1 = Window::new_test(1, Rect::new(0, 0, 100, 100));
    let window_2 = Window::new_test(2, Rect::new(100, 100, 200, 200));
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
  fn stores_returns_true_if_window_is_in_workspace() {
    let monitor = Monitor::new_test(1, Rect::default());
    let mut workspace = Workspace::new_test(PersistentWorkspaceId::new(monitor.id, 1, true), &monitor);
    let window = Window::new_test(1, Rect::new(0, 0, 100, 100));
    workspace.windows.push(window.clone());

    assert!(workspace.stores(&window.handle));
    assert!(!workspace.stores(&WindowHandle::new(42)));
  }

  #[test]
  fn stores_returns_false_if_window_is_not_in_workspace() {
    let monitor = Monitor::new_test(1, Rect::default());
    let workspace = Workspace::new_test(PersistentWorkspaceId::new(monitor.id, 1, true), &monitor);

    assert!(!workspace.stores(&WindowHandle::new(2)));
  }

  #[test]
  fn restore_windows_restores_all_windows() {
    let mut workspace = Workspace::new_test(PersistentWorkspaceId::new_test(1), &Monitor::mock_1());
    let sizing_window_1 = Sizing::new(0, 0, 100, 100);
    let sizing_window_2 = Sizing::new(100, 100, 100, 100);
    MockWindowsApi::add_or_update_window(1.into(), "Test Window 1".to_string(), sizing_window_1, false, false, true);
    MockWindowsApi::add_or_update_window(2.into(), "Test Window 2".to_string(), sizing_window_2, false, false, true);
    let mock_api = MockWindowsApi;
    let windows = mock_api.get_all_visible_windows();
    workspace.store_and_hide_windows(windows, 1.into(), &mock_api);

    workspace.restore_windows(&mock_api);

    let windows = mock_api.get_all_visible_windows();
    assert_eq!(windows.len(), 2);
    assert!(workspace.get_windows().is_empty());
  }

  #[test]
  fn restore_windows_handles_empty_workspace() {
    let mut workspace = Workspace::new_test(PersistentWorkspaceId::new_test(1), &Monitor::mock_1());
    let mock_api = MockWindowsApi;

    workspace.restore_windows(&mock_api);

    assert!(workspace.get_windows().is_empty());
  }

  #[test]
  fn remove_windows_if_present_removes_specified_windows() {
    let mut workspace = Workspace::new_test(PersistentWorkspaceId::new_test(1), &Monitor::mock_1());
    let window_1 = Window::new_test(1, Rect::new(0, 0, 100, 100));
    let window_2 = Window::new_test(2, Rect::new(100, 100, 200, 200));
    workspace.windows.push(window_1.clone());
    workspace.windows.push(window_2.clone());
    workspace.minimised_windows.push((window_1.handle, false));
    workspace.minimised_windows.push((window_2.handle, true));

    workspace.remove_windows_if_present(&[window_1.clone()]);

    assert_eq!(workspace.windows.len(), 1);
    assert_eq!(workspace.windows[0].handle, window_2.handle);
    assert_eq!(workspace.minimised_windows.len(), 1);
    assert_eq!(workspace.minimised_windows[0].0, window_2.handle);
  }

  #[test]
  fn remove_windows_if_present_does_nothing_if_windows_not_present() {
    let mut workspace = Workspace::new_test(PersistentWorkspaceId::new_test(1), &Monitor::mock_1());
    let window_1 = Window::new_test(1, Rect::new(0, 0, 100, 100));
    let window_2 = Window::new_test(2, Rect::new(100, 100, 200, 200));
    workspace.windows.push(window_1.clone());
    workspace.minimised_windows.push((window_1.handle, false));

    workspace.remove_windows_if_present(&[window_2.clone()]);

    assert_eq!(workspace.windows.len(), 1);
    assert_eq!(workspace.windows[0].handle, window_1.handle);
    assert_eq!(workspace.minimised_windows.len(), 1);
    assert_eq!(workspace.minimised_windows[0].0, window_1.handle);
  }

  #[test]
  fn remove_windows_if_present_handles_empty_input() {
    let mut workspace = Workspace::new_test(PersistentWorkspaceId::new_test(1), &Monitor::mock_1());
    let window = Window::new_test(1, Rect::new(0, 0, 100, 100));
    workspace.windows.push(window.clone());
    workspace.minimised_windows.push((window.handle, false));

    workspace.remove_windows_if_present(&[]);

    assert_eq!(workspace.windows.len(), 1);
    assert_eq!(workspace.windows[0].handle, window.handle);
    assert_eq!(workspace.minimised_windows.len(), 1);
    assert_eq!(workspace.minimised_windows[0].0, window.handle);
  }

  #[test]
  fn remove_windows_if_present_handles_empty_workspace() {
    let mut workspace = Workspace::new_test(PersistentWorkspaceId::new_test(1), &Monitor::mock_1());
    let window = Window::new_test(1, Rect::new(0, 0, 100, 100));

    workspace.remove_windows_if_present(&[window]);

    assert!(workspace.windows.is_empty());
    assert!(workspace.minimised_windows.is_empty());
  }
}
