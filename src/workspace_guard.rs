use crate::api::WindowsApi;
use crate::common::{MonitorHandle, PersistentWorkspaceId, TransientWorkspaceId, Window, Workspace};
use crate::workspace_manager::WorkspaceManager;
use std::collections::HashMap;

pub struct WorkspaceGuard<'a, T: WindowsApi + Clone> {
  pub(crate) manager: &'a mut WorkspaceManager<T>,
  id_map: HashMap<PersistentWorkspaceId, TransientWorkspaceId>,
}

impl<'a, T: WindowsApi + Clone> WorkspaceGuard<'a, T> {
  pub fn new(manager: &'a mut WorkspaceManager<T>) -> Self {
    let monitors = manager.windows_api.get_all_monitors();
    for workspace in manager.workspaces.values_mut() {
      if let Some(monitor) = monitors.get_by_id(&workspace.id.monitor_id) {
        workspace.update_handle(monitor.handle);
      }
    }
    let id_map = manager.create_workspace_id_map(monitors);

    Self { manager, id_map }
  }

  pub fn resolve_to_transient(&self, persistent_id: PersistentWorkspaceId) -> Option<TransientWorkspaceId> {
    if let Some(value) = self.id_map.get(&persistent_id).copied() {
      Some(value)
    } else {
      warn!(
        "Failed to resolve workspace ID [{}]: No matching transient ID found",
        persistent_id
      );
      None
    }
  }

  /// Returns the unique IDs for all workspaces across all monitors. Ordered by monitor position. Returned in ascending
  /// order from top-left to bottom-right.
  pub fn get_ordered_workspace_ids(&self) -> Vec<PersistentWorkspaceId> {
    let mut workspaces_by_monitor: HashMap<i64, Vec<&Workspace>> = HashMap::new();
    for workspace in self.manager.workspaces.values() {
      workspaces_by_monitor
        .entry(workspace.monitor_handle)
        .or_default()
        .push(workspace);
    }

    let mut monitor_handles: Vec<i64> = workspaces_by_monitor.keys().cloned().collect();
    monitor_handles.sort_by(|a, b| {
      let monitor_a = self
        .manager
        .workspaces
        .values()
        .find(|w| w.monitor_handle == *a)
        .map(|w| &w.monitor)
        .expect("Monitor not found");

      let monitor_b = self
        .manager
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
      "Ordered workspaces: [{}]",
      result.iter().map(|id| format!("{}", id)).collect::<Vec<_>>().join(", ")
    );

    result
  }

  pub fn switch_workspace(&mut self, target_workspace_id: PersistentWorkspaceId) {
    if self.resolve_to_transient(target_workspace_id).is_none() {
      return;
    }
    let current_workspace_id = match self.get_current_workspace_id_if_different_to(target_workspace_id) {
      Some(id) => id,
      None => return,
    };

    // Identify the active workspace on the target monitor
    let target_monitor_active_workspace_id = if let Some(workspace) = self.get_active_workspace(&target_workspace_id) {
      workspace
    } else {
      if target_workspace_id.monitor_id != current_workspace_id.monitor_id {
        error!(
          "Failed to switch workspace because: The target workspace ({}) does not exist",
          target_workspace_id.clone()
        );
        return;
      }
      trace!(
        "Expecting target monitor workspace ({}) and current workspace ({}) to be on the same monitor",
        target_workspace_id, current_workspace_id
      );
      current_workspace_id
    };

    // Hide and store all windows in the target workspace, if required
    if !target_workspace_id.is_same_workspace(&target_monitor_active_workspace_id) {
      if let Some(target_monitor_active_workspace) =
        self.manager.workspaces.get_mut(&target_monitor_active_workspace_id.into())
      {
        let current_windows = self
          .manager
          .windows_api
          .get_all_visible_windows_within_area(target_monitor_active_workspace.monitor.monitor_area);
        let current_monitor = MonitorHandle::from(target_monitor_active_workspace.monitor_handle);
        target_monitor_active_workspace.store_and_hide_windows(current_windows, current_monitor, &self.manager.windows_api);
      } else {
        warn!(
          "Failed to switch workspace because: The workspace ({}) to store the window doesn't exist",
          target_monitor_active_workspace_id
        );
        self.log_initialised_workspaces();
        return;
      };
    }

    // Attempt to find the largest window on the target workspace
    let largest_window = if let Some(new_workspace) = self.manager.workspaces.get(&target_workspace_id.into()) {
      let visible_windows = self
        .manager
        .windows_api
        .get_all_visible_windows_within_area(new_workspace.monitor.work_area);
      let mut windows: Vec<Window> = visible_windows
        .iter()
        .filter(|w| !self.manager.workspaces.values().any(|workspace| workspace.stores(&w.handle)))
        .cloned()
        .collect();
      if let Some(window) = new_workspace.get_largest_window() {
        windows.push(window);
      }
      windows.iter().max_by_key(|w| w.rect.area()).cloned().to_owned()
    } else {
      None
    };

    // Restore windows for the new workspace and set the cursor position
    if let Some(new_workspace) = self.manager.workspaces.get_mut(&target_workspace_id.into()) {
      new_workspace.restore_windows(&self.manager.windows_api);
      if let Some(largest_window) = largest_window {
        trace!(
          "Setting foreground window to {} \"{}\"",
          largest_window.handle,
          largest_window.title_trunc()
        );
        self.manager.windows_api.set_foreground_window(largest_window.handle);
        self.manager.windows_api.set_cursor_position(&largest_window.center);
      } else {
        self.manager.windows_api.set_cursor_position(&new_workspace.monitor.center);
      }
    } else {
      // Restore the original workspace if the target workspace doesn't exist
      warn!(
        "Failed to switch workspace because: The target workspace ({}) does not exist",
        target_workspace_id
      );
      if let Some(original_workspace) = self.manager.workspaces.get_mut(&current_workspace_id.into()) {
        original_workspace.restore_windows(&self.manager.windows_api);
        self
          .manager
          .windows_api
          .set_cursor_position(&original_workspace.monitor.center);
        debug!(
          "Restored original workspace [{}] due to earlier failures",
          current_workspace_id
        );
      } else {
        error!(
          "Failed to restore original workspace [{}] because it does not exist",
          current_workspace_id
        );
        panic!(
          "Failed to restore original workspace [{}] because it does not exist",
          current_workspace_id
        );
      }
      return;
    };

    // Update the active workspaces
    if !target_workspace_id.is_same_workspace(&target_monitor_active_workspace_id) {
      self.set_active_workspace(&target_workspace_id, true);
      self.set_active_workspace(&target_monitor_active_workspace_id, false);
    }

    info!(
      "Switched workspace from [{}] to [{}]",
      current_workspace_id, target_workspace_id
    );
  }

  pub fn move_window_to_workspace(&mut self, target_workspace_id: PersistentWorkspaceId) {
    if self.resolve_to_transient(target_workspace_id).is_none()
      && self.get_current_workspace_id_if_different_to(target_workspace_id).is_none()
    {
      return;
    }

    // Collect all relevant information
    let Some(foreground_window) = self.manager.windows_api.get_foreground_window() else {
      debug!("Ignored request to move window to workspace because there is no foreground window");
      return;
    };
    let Some(window_placement) = self.manager.windows_api.get_window_placement(foreground_window) else {
      debug!("Ignored request to move window to workspace because the window is not visible");
      return;
    };
    let window_title = self.manager.windows_api.get_window_title(&foreground_window);
    let window = Window::new(foreground_window.as_hwnd(), window_title, window_placement.normal_position);
    let current_monitor = self.manager.windows_api.get_monitor_handle_for_window_handle(window.handle);

    // Move or store the window
    if let Some(target_workspace) = self.manager.workspaces.get_mut(&target_workspace_id.into()) {
      target_workspace.move_or_store_and_hide_window(window.clone(), current_monitor, &self.manager.windows_api);
    } else {
      warn!(
        "Failed to move window to workspace because: The target workspace ({}) does not exist",
        target_workspace_id
      );
    }

    info!(
      "Moved {} \"{}\" to workspace [{}]",
      window.handle,
      window.title_trunc(),
      target_workspace_id
    );
  }

  pub fn restore_all_managed_windows(&mut self) {
    for workspace in self.manager.workspaces.values_mut() {
      workspace.restore_windows(&self.manager.windows_api);
    }
  }

  pub(crate) fn get_current_workspace_id_if_different_to(
    &mut self,
    other: PersistentWorkspaceId,
  ) -> Option<PersistentWorkspaceId> {
    let Some(current_workspace_id) = self.get_active_workspace_for_cursor_position() else {
      warn!("Failed to complete request: Unable to find the active workspace");
      return None;
    };

    if other == current_workspace_id {
      info!(
        "Ignored request because current and target workspaces are the same: {}",
        other
      );
      return None;
    }

    Some(current_workspace_id)
  }

  fn get_active_workspace_for_cursor_position(&mut self) -> Option<PersistentWorkspaceId> {
    let cursor_position = self.manager.windows_api.get_cursor_position();
    let monitor_handle = self.manager.windows_api.get_monitor_handle_for_point(&cursor_position);
    let monitor_id = self
      .manager
      .windows_api
      .get_monitor_id_for_handle(monitor_handle)
      .expect("Cannot find monitor for handle");

    let workspace_ids = self
      .manager
      .workspaces
      .iter()
      .filter(|(id, workspace)| workspace.is_active() && id.monitor_id == monitor_id)
      .map(|(id, _)| id)
      .collect::<Vec<_>>();

    if workspace_ids.len() == 1 {
      Some(*workspace_ids[0])
    } else {
      error!(
        "Data inconsistency detected: Found {} active workspaces for monitor [{monitor_handle}]: {:?}",
        workspace_ids.len(),
        workspace_ids,
      );

      None
    }
  }

  fn get_active_workspace(&mut self, workspace_id: &PersistentWorkspaceId) -> Option<PersistentWorkspaceId> {
    self
      .manager
      .workspaces
      .iter()
      .filter(|(_, workspace)| workspace.is_active())
      .map(|(id, _)| *id)
      .find(|id| id.monitor_id == workspace_id.monitor_id)
  }

  fn set_active_workspace(&mut self, workspace_id: &PersistentWorkspaceId, is_active: bool) {
    self
      .manager
      .workspaces
      .iter_mut()
      .filter(|(id, _)| id.monitor_id == workspace_id.monitor_id && id.workspace == workspace_id.workspace)
      .for_each(|(_, workspace)| {
        workspace.set_active(is_active);
      });
  }

  fn log_initialised_workspaces(&mut self) {
    let ordered_workspaces = self.manager.get_ordered_permanent_workspace_ids();
    debug!(
      "Found [{}] workspaces (ordered): [{}] of which [{}] are active",
      ordered_workspaces.len(),
      ordered_workspaces
        .iter()
        .map(|id| format!("{}", id))
        .collect::<Vec<_>>()
        .join(", "),
      self
        .manager
        .workspaces
        .iter()
        .filter(|(_, workspace)| workspace.is_active())
        .map(|(id, _)| *id)
        .map(|id| format!("{}", id))
        .collect::<Vec<_>>()
        .join(", "),
    );
  }
}

#[cfg(test)]
mod tests {
  use crate::api::MockWindowsApi;
  use crate::common::{MonitorHandle, Point, Rect};
  use crate::workspace_guard::WorkspaceGuard;
  use crate::workspace_manager::WorkspaceManager;

  #[test]
  fn get_active_workspace_for_cursor_position_returns_workspace_if_one_active_workspace_found() {
    let mut workspace_manager = WorkspaceManager::new_test(false);
    let mut guard = WorkspaceGuard::new(&mut workspace_manager);
    // Cursor on primary monitor
    MockWindowsApi::set_cursor_position(Point::new(50, 50));

    let result = guard.get_active_workspace_for_cursor_position();

    assert_eq!(
      result,
      Some((*crate::workspace_manager::tests::primary_active_ws_id()).into())
    );
  }

  #[test]
  fn get_active_workspace_for_cursor_position_returns_none_when_no_matches() {
    let mut workspace_manager = WorkspaceManager::default();
    let mut guard = WorkspaceGuard::new(&mut workspace_manager);
    MockWindowsApi::set_cursor_position(Point::new(100, 100));
    MockWindowsApi::add_monitor(MonitorHandle::from(5), Rect::new(0, 0, 200, 200), true);

    let result = guard.get_active_workspace_for_cursor_position();

    assert!(result.is_none());
  }
}
