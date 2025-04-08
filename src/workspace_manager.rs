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
      "Found [{}] workspaces (additional_workspace_count={additional_workspace_count}): {}",
      workspaces.len(),
      workspaces.keys().map(|id| format!("{}", id)).collect::<Vec<_>>().join(", ")
    );
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

  // TODO: Sort by monitor ID + direction + distance (left -> top -> middle -> right -> bottom)
  /// Returns the unique IDs for all workspaces across all monitors in their natural order.
  pub fn get_ordered_workspace_ids(&self) -> Vec<WorkspaceId> {
    let mut ids: Vec<WorkspaceId> = self.workspaces.keys().cloned().collect();
    ids.sort();

    ids
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
  use crate::utils::{Monitor, Point, Rect, WindowHandle, WindowPlacement, Workspace, WorkspaceId};

  impl WorkspaceManager<MockWindowsApi> {
    pub fn for_testing(target_workspace_id: WorkspaceId) -> Self {
      let foreground_window_handle = WindowHandle::new(1);
      let foreground_window_placement = WindowPlacement::new_from_rect(Rect::new(50, 50, 100, 100));
      let foreground_window = Window::new(
        foreground_window_handle.as_hwnd(),
        "Test Window".to_string(),
        foreground_window_placement.normal_position,
      );
      MockWindowsApi::set_foreground_window(foreground_window_handle);
      MockWindowsApi::set_window_placement(Some(foreground_window_placement));
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
  }

  #[test]
  fn test_switch_workspace() {
    MockWindowsApi::reset();

    // Given the current workspace has one window and target workspace is not active
    let target_workspace_id = WorkspaceId::from(1, 2);
    let mut workspace_manager = WorkspaceManager::for_testing(target_workspace_id);
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
  fn test_move_window_to_different_workspace_on_same_monitor() {
    MockWindowsApi::reset();

    // Given the target workspace as one window and is not active
    let workspace_id = WorkspaceId::from(1, 2);
    let mut workspace_manager = WorkspaceManager::for_testing(workspace_id);

    // When the user moves a window to a different workspace on the same monitor
    workspace_manager.move_window_to_workspace(workspace_id);

    // Then the window appear in the target workspace
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
