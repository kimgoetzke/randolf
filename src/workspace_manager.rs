use crate::native_api;
use crate::utils::{Window, Workspace, WorkspaceId};
use std::collections::HashMap;

pub struct WorkspaceManager {
  active_workspaces: Vec<WorkspaceId>,
  workspaces: HashMap<WorkspaceId, Workspace>,
}

impl WorkspaceManager {
  pub fn new(additional_workspace_count: i32) -> Self {
    let (workspaces, active_workspaces) = Self::initialise_workspaces(additional_workspace_count);
    Self {
      workspaces,
      active_workspaces,
    }
  }

  fn initialise_workspaces(additional_workspace_count: i32) -> (HashMap<WorkspaceId, Workspace>, Vec<WorkspaceId>) {
    let mut workspaces = HashMap::new();
    let mut active_workspace_ids = Vec::new();
    let all_monitors = native_api::get_all_monitors();
    for monitor in all_monitors.get_all().iter() {
      let monitor_id = monitor.handle;
      if monitor.is_primary {
        for layer in 1..=additional_workspace_count as usize {
          let id = WorkspaceId::from(monitor_id, layer);
          let container = Workspace::from(id, monitor);
          if layer == 1 {
            active_workspace_ids.push(id);
          }
          workspaces.insert(id, container);
        }
      } else {
        let id = WorkspaceId::from(monitor_id, 1);
        workspaces.insert(id, Workspace::from(id, monitor));
        active_workspace_ids.push(id);
      }
    }
    debug!(
      "Found [{}] workspaces (additional_workspace_count={additional_workspace_count}): {:?}",
      workspaces.len(),
      workspaces.keys()
    );
    debug!(
      "Set [{}] active workspaces: {:?}",
      active_workspace_ids.len(),
      active_workspace_ids
    );

    (workspaces, active_workspace_ids)
  }

  // TODO: Sort by monitor ID + direction + distance (left -> top -> middle -> right -> bottom)
  /// Returns the unique IDs for all workspaces across all monitors in their natural order.
  pub fn get_ordered_workspace_ids(&self) -> Vec<WorkspaceId> {
    let mut ids: Vec<WorkspaceId> = self.workspaces.keys().cloned().collect();
    ids.sort();

    ids
  }

  pub fn switch_workspace(&mut self, target_workspace_id: WorkspaceId) {
    let current_workspace_id = match self.get_current_workspace(target_workspace_id) {
      Some(id) => id,
      None => return,
    };

    let target_monitor_active_workspace_id = if let Some(workspace) = self.remove_active_workspace(&target_workspace_id) {
      workspace
    } else {
      if target_workspace_id.monitor_id != current_workspace_id.monitor_id {
        panic!(
          "Failed to switch workspace because: The target workspace ({}) does not exist",
          target_workspace_id
        );
      }
      trace!(
        "Expecting target monitor workspace ({}) and current workspace ({}) to be on the same monitor",
        target_workspace_id.monitor_id, current_workspace_id.monitor_id
      );
      current_workspace_id
    };

    // Hide and store all windows in the target workspace, if required
    if !target_workspace_id.is_same_workspace(&target_monitor_active_workspace_id) {
      if let Some(target_monitor_active_workspace) = self.workspaces.get_mut(&target_monitor_active_workspace_id) {
        let current_windows =
          native_api::get_all_visible_windows_within_area(target_monitor_active_workspace.monitor.monitor_area);
        target_monitor_active_workspace.store_and_hide_windows(current_windows);
      } else {
        warn!(
          "Failed to switch workspace because: The workspace to store ({}) doesn't exist",
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
      new_workspace.restore_windows();
      native_api::set_cursor_position(&new_workspace.monitor.center);
    } else {
      // Restore the original workspace
      warn!(
        "Failed to switch workspace because: The target workspace ({}) does not exist",
        target_workspace_id
      );
      if let Some(original_workspace) = self.workspaces.get_mut(&current_workspace_id) {
        original_workspace.restore_windows();
        native_api::set_cursor_position(&original_workspace.monitor.center);
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

  fn get_current_workspace(&mut self, target_workspace_id: WorkspaceId) -> Option<WorkspaceId> {
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
    let cursor_position = native_api::get_cursor_position();
    let monitor_handle = native_api::get_monitor_for_point(&cursor_position);
    let Some(position_in_vec) = self.active_workspaces.iter().position(|id| id.monitor_id == monitor_handle) else {
      warn!("Unable to find the active workspace for monitor [{monitor_handle}]");

      return None;
    };

    Some(self.active_workspaces.remove(position_in_vec))
  }

  fn remove_active_workspace(&mut self, workspace_id: &WorkspaceId) -> Option<WorkspaceId> {
    self
      .active_workspaces
      .iter()
      .position(|id| id.monitor_id == workspace_id.monitor_id)
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
    let current_workspace_id = match self.get_current_workspace(target_workspace_id) {
      Some(id) => id,
      None => return,
    };
    self.add_active_workspace(current_workspace_id);

    let Some(foreground_window) = native_api::get_foreground_window() else {
      debug!("Ignored request to move window to workspace because there is no foreground window");
      return;
    };
    let Some(window_placement) = native_api::get_window_placement(foreground_window) else {
      debug!("Ignored request to move window to workspace because the window is not visible");
      return;
    };
    let window_title = native_api::get_window_title(&foreground_window);
    let window = Window::new(window_title, window_placement.normal_position, foreground_window.as_hwnd());

    if let Some(target_workspace) = self.workspaces.get_mut(&target_workspace_id) {
      target_workspace.add_window(window.clone());
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
