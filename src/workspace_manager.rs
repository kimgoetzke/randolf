use crate::api::NativeApi;
use crate::utils::{Window, Workspace, WorkspaceId};
use std::collections::HashMap;

pub struct WorkspaceManager<T: NativeApi> {
  active_workspaces: Vec<WorkspaceId>,
  workspaces: HashMap<WorkspaceId, Workspace>,
  windows_api: T,
}

impl<T: NativeApi + Copy> WorkspaceManager<T> {
  pub fn new(additional_workspace_count: i32, api: T) -> Self {
    let mut workspace_manager = Self {
      active_workspaces: Vec::new(),
      workspaces: HashMap::new(),
      windows_api: api,
    };
    workspace_manager.initialise_workspaces(additional_workspace_count);

    workspace_manager
  }

  fn initialise_workspaces(&mut self, additional_workspace_count: i32) {
    let mut workspaces = HashMap::new();
    let mut active_workspace_ids = Vec::new();
    let all_monitors = self.windows_api.get_all_monitors();
    for monitor in all_monitors.get_all().iter() {
      let monitor_handle = monitor.handle;
      if monitor.is_primary {
        for layer in 1..=additional_workspace_count as usize + 1 {
          let id = WorkspaceId::from(monitor_handle, layer);
          let container = Workspace::from(id, monitor);
          if layer == 1 {
            active_workspace_ids.push(id);
          }
          workspaces.insert(id, container);
        }
      } else {
        let id = WorkspaceId::from(monitor_handle, 1);
        workspaces.insert(id, Workspace::from(id, monitor));
        active_workspace_ids.push(id);
      }
    }
    debug!(
      "Set [{}] workspaces to active: {}",
      active_workspace_ids.len(),
      active_workspace_ids
        .iter()
        .map(|id| format!("{}", id))
        .collect::<Vec<_>>()
        .join(", ")
    );

    self.workspaces = workspaces;
    self.active_workspaces = active_workspace_ids;
  }

  /// Returns the unique IDs for all workspaces across all monitors. Ordered by monitor position. Returned in ascending
  /// order from top-left to bottom-right.
  pub fn get_ordered_workspace_ids(&self) -> Vec<WorkspaceId> {
    let mut workspaces_by_monitor: HashMap<i64, Vec<&Workspace>> = HashMap::new();

    for workspace in self.workspaces.values() {
      workspaces_by_monitor
        .entry(workspace.monitor_handle)
        .or_default()
        .push(workspace);
    }

    let mut monitor_handles: Vec<i64> = workspaces_by_monitor.keys().cloned().collect();
    monitor_handles.sort_by(|a, b| {
      let monitor_a = self
        .workspaces
        .values()
        .find(|w| w.monitor_handle == *a)
        .map(|w| &w.monitor)
        .expect("Monitor not found");

      let monitor_b = self
        .workspaces
        .values()
        .find(|w| w.monitor_handle == *b)
        .map(|w| &w.monitor)
        .expect("Monitor not found");

      // Left to right
      let a_center_x = monitor_a.center.x();
      let b_center_x = monitor_b.center.x();
      if a_center_x != b_center_x {
        return a_center_x.cmp(&b_center_x);
      }

      // Top to bottom
      let a_center_y = monitor_a.center.y();
      let b_center_y = monitor_b.center.y();

      a_center_y.cmp(&b_center_y)
    });

    let mut result = Vec::new();
    for monitor_handle in monitor_handles {
      let mut monitor_workspaces = workspaces_by_monitor.remove(&monitor_handle).expect("Monitor not found");
      monitor_workspaces.sort_by_key(|w| w.id);
      for workspace in monitor_workspaces {
        result.push(workspace.id);
      }
    }

    debug!(
      "Found [{}] workspaces (ordered): {}",
      result.len(),
      result.iter().map(|id| format!("{}", id)).collect::<Vec<_>>().join(", ")
    );

    result
  }

  pub fn switch_workspace(&mut self, target_workspace_id: WorkspaceId) {
    let current_workspace_id = match self.remove_current_workspace_if_different(target_workspace_id) {
      Some(id) => id,
      None => return,
    };

    let target_monitor_active_workspace_id = if let Some(workspace) = self.remove_active_workspace(&target_workspace_id) {
      workspace
    } else {
      if target_workspace_id.monitor_handle != current_workspace_id.monitor_handle {
        error!(
          "Failed to switch workspace because: The target workspace ({}) does not exist",
          target_workspace_id
        );
        return;
      }
      trace!(
        "Expecting target monitor workspace ({}) and current workspace ({}) to be on the same monitor",
        target_workspace_id.monitor_handle, current_workspace_id.monitor_handle
      );
      current_workspace_id
    };

    // Hide and store all windows in the target workspace, if required
    if !target_workspace_id.is_same_workspace(&target_monitor_active_workspace_id) {
      if let Some(target_monitor_active_workspace) = self.workspaces.get_mut(&target_monitor_active_workspace_id) {
        let current_windows = self
          .windows_api
          .get_all_visible_windows_within_area(target_monitor_active_workspace.monitor.monitor_area);
        target_monitor_active_workspace.store_and_hide_windows(current_windows, &self.windows_api);
      } else {
        warn!(
          "Failed to switch workspace because: The workspace ({}) to store the window doesn't exist",
          target_monitor_active_workspace_id
        );
        self.add_active_workspace(current_workspace_id);
        self.add_active_workspace(target_monitor_active_workspace_id);
        self.log_active_workspaces();
        return;
      };
    }

    // Restore windows for the new workspace and set the cursor position
    if let Some(new_workspace) = self.workspaces.get_mut(&target_workspace_id) {
      new_workspace.restore_windows(&self.windows_api);
      self.windows_api.set_cursor_position(&new_workspace.monitor.center);
    } else {
      // Restore the original workspace
      warn!(
        "Failed to switch workspace because: The target workspace ({}) does not exist",
        target_workspace_id
      );
      if let Some(original_workspace) = self.workspaces.get_mut(&current_workspace_id) {
        original_workspace.restore_windows(&self.windows_api);
        self.windows_api.set_cursor_position(&original_workspace.monitor.center);
        debug!(
          "Restored original workspace [{}] due to earlier failures",
          current_workspace_id
        );
      } else {
        panic!(
          "Failed to restore original workspace [{}] because it does not exist",
          current_workspace_id
        );
      }
      self.add_active_workspace(current_workspace_id);
      return;
    };

    // Set the active workspace(s)
    self.add_active_workspace(target_workspace_id);
    if !target_workspace_id.is_same_monitor(&current_workspace_id) {
      self.add_active_workspace(current_workspace_id)
    }

    info!("Switched workspace from {} to {}", current_workspace_id, target_workspace_id);
  }

  fn remove_current_workspace_if_different(&mut self, target_workspace_id: WorkspaceId) -> Option<WorkspaceId> {
    let Some(current_workspace_id) = self.remove_active_workspace_for_cursor_position() else {
      warn!("Failed to complete request: Unable to find the active workspace");
      return None;
    };

    if target_workspace_id == current_workspace_id {
      info!(
        "Ignored request because current and target workspaces are the same: {}",
        target_workspace_id
      );
      self.add_active_workspace(current_workspace_id);
      return None;
    }

    Some(current_workspace_id)
  }

  fn remove_active_workspace_for_cursor_position(&mut self) -> Option<WorkspaceId> {
    let cursor_position = self.windows_api.get_cursor_position();
    let monitor_handle = self.windows_api.get_monitor_for_point(&cursor_position);
    let Some(position_in_vec) = self
      .active_workspaces
      .iter()
      .position(|id| id.monitor_handle == monitor_handle)
    else {
      warn!("Unable to find the active workspace for monitor [{monitor_handle}]");

      return None;
    };

    Some(self.active_workspaces.remove(position_in_vec))
  }

  fn remove_active_workspace(&mut self, workspace_id: &WorkspaceId) -> Option<WorkspaceId> {
    self
      .active_workspaces
      .iter()
      .position(|id| id.monitor_handle == workspace_id.monitor_handle)
      .map(|position| self.active_workspaces.remove(position))
  }

  fn add_active_workspace(&mut self, workspace_id: WorkspaceId) {
    if !self.active_workspaces.contains(&workspace_id) {
      self.active_workspaces.push(workspace_id);
    } else {
      warn!("Attempted to add workspace [{workspace_id}] to active workspaces but it already exists");
    }
  }

  pub fn move_window_to_workspace(&mut self, target_workspace_id: WorkspaceId) {
    let current_workspace_id = match self.remove_current_workspace_if_different(target_workspace_id) {
      Some(id) => id,
      None => return,
    };
    self.add_active_workspace(current_workspace_id);

    let Some(foreground_window) = self.windows_api.get_foreground_window() else {
      debug!("Ignored request to move window to workspace because there is no foreground window");
      return;
    };
    let Some(window_placement) = self.windows_api.get_window_placement(foreground_window) else {
      debug!("Ignored request to move window to workspace because the window is not visible");
      return;
    };
    let window_title = self.windows_api.get_window_title(&foreground_window);
    let window = Window::new(foreground_window.as_hwnd(), window_title, window_placement.normal_position);

    if let Some(target_workspace) = self.workspaces.get_mut(&target_workspace_id) {
      target_workspace.store_and_hide_window(window.clone(), &self.windows_api);
    } else {
      warn!(
        "Failed to move window to workspace because: The target workspace ({}) does not exist",
        target_workspace_id
      );
    }

    info!(
      "Moved window {} ({}) to workspace {}",
      window.handle,
      window.title_trunc(),
      target_workspace_id
    );
  }

  fn log_active_workspaces(&self) {
    debug!(
      "Found [{}] active workspaces: {:?}",
      self.active_workspaces.len(),
      self.active_workspaces
    );
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api::MockWindowsApi;
  use crate::utils::WindowHandle;
  use crate::utils::{Monitor, Point, Rect, WindowPlacement, Workspace, WorkspaceId};

  impl WorkspaceManager<MockWindowsApi> {
    pub fn default() -> Self {
      Self {
        active_workspaces: Vec::new(),
        workspaces: HashMap::new(),
        windows_api: MockWindowsApi::new(),
      }
    }

    pub fn new_test(target_workspace_id: WorkspaceId) -> Self {
      let foreground_window_handle = WindowHandle::new(1);
      let foreground_window_placement = WindowPlacement::new_from_rect(Rect::new(50, 50, 100, 100));
      let foreground_window = Window::new(
        foreground_window_handle.as_hwnd(),
        "Test Window".to_string(),
        foreground_window_placement.normal_position,
      );
      MockWindowsApi::set_foreground_window(foreground_window_handle);
      MockWindowsApi::set_window_placement(foreground_window_handle, foreground_window_placement);
      MockWindowsApi::set_window_title(foreground_window.title.to_string());
      MockWindowsApi::set_visible_windows(vec![foreground_window]);

      let primary_monitor = Monitor::mock_1();
      let secondary_monitor = Monitor::mock_2();
      MockWindowsApi::set_cursor_position(Point::new(50, 50));
      MockWindowsApi::set_monitor_for_point(primary_monitor.handle);
      MockWindowsApi::set_monitors(vec![primary_monitor.clone(), secondary_monitor.clone()]);

      let mock_api = MockWindowsApi;
      let current_workspace_id = WorkspaceId::from(primary_monitor.handle, 1);
      let secondary_active_workspace_id = WorkspaceId::from(secondary_monitor.handle, 1);
      let current_workspace = Workspace::from(current_workspace_id, &primary_monitor);
      let target_workspace = Workspace::from(target_workspace_id, &primary_monitor);

      WorkspaceManager {
        active_workspaces: vec![current_workspace_id, secondary_active_workspace_id],
        workspaces: HashMap::from([
          (current_workspace_id, current_workspace),
          (target_workspace_id, target_workspace),
          (
            secondary_active_workspace_id,
            Workspace::from(secondary_active_workspace_id, &secondary_monitor),
          ),
        ]),
        windows_api: mock_api,
      }
    }

    pub fn from_workspaces(workspaces: &[&Workspace]) -> Self {
      let mut workspace_map = HashMap::new();
      for workspace in workspaces {
        workspace_map.insert(workspace.id, workspace.to_owned().clone());
      }

      Self {
        active_workspaces: vec![],
        workspaces: workspace_map,
        windows_api: MockWindowsApi::new(),
      }
    }
  }

  #[test]
  fn get_ordered_workspace_ids_left_to_right() {
    MockWindowsApi::reset();

    let left_monitor = Monitor::new_test(1, Rect::new(0, 0, 99, 100));
    let center_monitor = Monitor::new_test(2, Rect::new(100, 0, 199, 100));
    let right_monitor = Monitor::new_test(3, Rect::new(200, 0, 299, 100));
    let left_workspace = Workspace::from(WorkspaceId::from(left_monitor.handle, 1), &left_monitor);
    let center_workspace = Workspace::from(WorkspaceId::from(center_monitor.handle, 1), &center_monitor);
    let right_workspace = Workspace::from(WorkspaceId::from(right_monitor.handle, 1), &right_monitor);
    let workspace_manager = WorkspaceManager::from_workspaces(&[&left_workspace, &center_workspace, &right_workspace]);

    let ordered_workspaces = workspace_manager.get_ordered_workspace_ids();

    assert_eq!(ordered_workspaces.len(), 3);
    assert_eq!(ordered_workspaces[0], left_workspace.id);
    assert_eq!(ordered_workspaces[1], center_workspace.id);
    assert_eq!(ordered_workspaces[2], right_workspace.id);
  }

  #[test]
  fn get_ordered_workspace_ids_top_to_bottom() {
    MockWindowsApi::reset();

    let top_monitor = Monitor::new_test(1, Rect::new(0, 0, 100, 99));
    let center_monitor = Monitor::new_test(2, Rect::new(0, 100, 100, 199));
    let bottom_monitor = Monitor::new_test(3, Rect::new(0, 200, 100, 299));
    let top_workspace = Workspace::from(WorkspaceId::from(top_monitor.handle, 1), &top_monitor);
    let center_workspace = Workspace::from(WorkspaceId::from(center_monitor.handle, 1), &center_monitor);
    let bottom_workspace = Workspace::from(WorkspaceId::from(bottom_monitor.handle, 1), &bottom_monitor);
    let workspace_manager = WorkspaceManager::from_workspaces(&[&top_workspace, &center_workspace, &bottom_workspace]);

    let ordered_workspaces = workspace_manager.get_ordered_workspace_ids();

    assert_eq!(ordered_workspaces.len(), 3);
    assert_eq!(ordered_workspaces[0], top_workspace.id);
    assert_eq!(ordered_workspaces[1], center_workspace.id);
    assert_eq!(ordered_workspaces[2], bottom_workspace.id);
  }

  #[test]
  fn get_ordered_workspace_ids_with_multiple_workspaces_on_same_monitor() {
    MockWindowsApi::reset();

    let top_monitor = Monitor::new_test(1, Rect::new(0, 0, 100, 99));
    let bottom_monitor = Monitor::new_test(3, Rect::new(0, 200, 100, 299));
    let top_workspace_1 = Workspace::from(WorkspaceId::from(top_monitor.handle, 1), &top_monitor);
    let top_workspace_2 = Workspace::from(WorkspaceId::from(top_monitor.handle, 2), &top_monitor);
    let bottom_workspace_1 = Workspace::from(WorkspaceId::from(bottom_monitor.handle, 1), &bottom_monitor);
    let bottom_workspace_2 = Workspace::from(WorkspaceId::from(bottom_monitor.handle, 2), &bottom_monitor);
    let workspace_manager =
      WorkspaceManager::from_workspaces(&[&top_workspace_1, &top_workspace_2, &bottom_workspace_1, &bottom_workspace_2]);

    let ordered_workspaces = workspace_manager.get_ordered_workspace_ids();

    assert_eq!(ordered_workspaces.len(), 4);
    assert_eq!(ordered_workspaces[0], top_workspace_1.id);
    assert_eq!(ordered_workspaces[1], top_workspace_2.id);
    assert_eq!(ordered_workspaces[2], bottom_workspace_1.id);
    assert_eq!(ordered_workspaces[3], bottom_workspace_2.id);
  }

  #[test]
  fn switch_workspace() {
    MockWindowsApi::reset();

    // Given the current workspace has one window and target workspace is not active
    let target_workspace_id = WorkspaceId::from(1, 2);
    let mut workspace_manager = WorkspaceManager::new_test(target_workspace_id);
    assert_eq!(workspace_manager.active_workspaces.len(), 2);
    assert!(!workspace_manager.active_workspaces.contains(&target_workspace_id));

    // When the user switches to the target workspace
    workspace_manager.switch_workspace(target_workspace_id);

    // Then the active workspace for the relevant monitor is updated
    assert_eq!(workspace_manager.active_workspaces.len(), 2);
    assert!(workspace_manager.active_workspaces.contains(&target_workspace_id));

    // And the window on the original workspace has been stored
    let original_workspace = workspace_manager
      .workspaces
      .get(&WorkspaceId::from(1, 1))
      .expect("Original workspace not found");
    assert_eq!(original_workspace.get_windows().len(), 1);
  }

  #[test]
  fn move_window_to_different_workspace_on_same_monitor() {
    MockWindowsApi::reset();

    // Given the target workspace as one window and is not active
    let workspace_id = WorkspaceId::from(1, 2);
    let mut workspace_manager = WorkspaceManager::new_test(workspace_id);

    // When the user moves a window to a different workspace on the same monitor
    workspace_manager.move_window_to_workspace(workspace_id);

    // Then the window appears in the target workspace
    let target_workspace = workspace_manager
      .workspaces
      .get(&workspace_id)
      .expect("Target workspace not found");
    assert_eq!(target_workspace.get_windows().len(), 1);
    assert_eq!(target_workspace.get_window_state_info().len(), 1);
    assert_eq!(
      target_workspace
        .get_windows()
        .first()
        .expect("Failed to retrieve window title")
        .title,
      "Test Window"
    );

    // But the active workspace has not changed
    let active_workspaces = workspace_manager.active_workspaces;
    assert_eq!(active_workspaces.len(), 2);
    assert!(!active_workspaces.contains(&workspace_id));
  }
}
