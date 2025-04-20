use crate::api::WindowsApi;
use crate::utils::{MonitorHandle, Window, Workspace, WorkspaceId};
use std::collections::HashMap;

pub struct WorkspaceManager<T: WindowsApi> {
  workspaces: HashMap<WorkspaceId, Workspace>,
  windows_api: T,
  margin: i32,
  additional_workspace_count: i32,
}

const MAX_RECURSION_DEPTH: usize = 1;

impl<T: WindowsApi + Copy> WorkspaceManager<T> {
  pub fn new(additional_workspace_count: i32, margin: i32, api: T) -> Self {
    let mut workspace_manager = Self {
      workspaces: HashMap::new(),
      windows_api: api,
      margin,
      additional_workspace_count,
    };
    workspace_manager.initialise_workspaces();

    workspace_manager
  }

  fn initialise_workspaces(&mut self) {
    let mut workspaces = HashMap::new();
    let all_monitors = self.windows_api.get_all_monitors();
    for monitor in all_monitors.get_all() {
      let monitor_handle = monitor.handle;
      if monitor.is_primary {
        for layer in 1..=self.additional_workspace_count + 1 {
          let id = WorkspaceId::new(monitor_handle, layer as usize);
          let workspace = if layer == 1 {
            Workspace::new_active(id, monitor, self.margin)
          } else {
            Workspace::new_inactive(id, monitor, self.margin)
          };
          workspaces.insert(id, workspace);
        }
      } else {
        let id = WorkspaceId::new(monitor_handle, 1);
        workspaces.insert(id, Workspace::new_active(id, monitor, self.margin));
      }
    }
    self.workspaces = workspaces;
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
      "Found [{}] workspaces (ordered): [{}] of which [{}] are active",
      result.len(),
      result.iter().map(|id| format!("{}", id)).collect::<Vec<_>>().join(", "),
      self
        .workspaces
        .iter()
        .filter(|(_, workspace)| workspace.is_active())
        .map(|(id, _)| *id)
        .map(|id| format!("{}", id))
        .collect::<Vec<_>>()
        .join(", "),
    );

    result
  }

  pub fn switch_workspace(&mut self, target_workspace_id: WorkspaceId) {
    let current_workspace_id = match self.get_current_workspace_id_if_different(target_workspace_id) {
      Some(id) => id,
      None => return,
    };

    // Identify the active workspace on the target monitor
    let target_monitor_active_workspace_id = if let Some(workspace) = self.get_active_workspace(&target_workspace_id) {
      *workspace
    } else {
      if target_workspace_id.monitor_handle != current_workspace_id.monitor_handle {
        error!(
          "Failed to switch workspace because: The target workspace ({}) does not exist",
          target_workspace_id.clone()
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
        let current_monitor = MonitorHandle::from(target_monitor_active_workspace.monitor_handle);
        target_monitor_active_workspace.store_and_hide_windows(current_windows, current_monitor, &self.windows_api);
      } else {
        warn!(
          "Failed to switch workspace because: The workspace ({}) to store the window doesn't exist",
          target_monitor_active_workspace_id
        );
        self.log_active_workspaces();
        return;
      };
    }

    // Attempt to find the largest window on the target workspace
    let largest_window = if let Some(new_workspace) = self.workspaces.get(&target_workspace_id) {
      let visible_windows = self
        .windows_api
        .get_all_visible_windows_within_area(new_workspace.monitor.work_area);
      let mut windows: Vec<Window> = visible_windows
        .iter()
        .filter(|w| !self.workspaces.values().any(|workspace| workspace.stores(&w.handle)))
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
    if let Some(new_workspace) = self.workspaces.get_mut(&target_workspace_id) {
      new_workspace.restore_windows(&self.windows_api);
      if let Some(largest_window) = largest_window {
        trace!(
          "Setting foreground window to {} \"{}\"",
          largest_window.handle,
          largest_window.title_trunc()
        );
        self.windows_api.set_foreground_window(largest_window.handle);
        self.windows_api.set_cursor_position(&largest_window.center);
      } else {
        self.windows_api.set_cursor_position(&new_workspace.monitor.center);
      }
    } else {
      // Restore the original workspace if the target workspace doesn't exist
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

    info!("Switched workspace from {} to {}", current_workspace_id, target_workspace_id);
  }

  pub fn move_window_to_workspace(&mut self, target_workspace_id: WorkspaceId) {
    // Guard against moving a window to the same workspace
    match self.get_current_workspace_id_if_different(target_workspace_id) {
      Some(_) => {}
      None => return,
    };

    // Collect all relevant information
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
    let current_monitor = self.windows_api.get_monitor_for_window_handle(window.handle);

    // Move or store the window
    if let Some(target_workspace) = self.workspaces.get_mut(&target_workspace_id) {
      target_workspace.move_or_store_and_hide_window(window.clone(), current_monitor, &self.windows_api);
    } else {
      warn!(
        "Failed to move window to workspace because: The target workspace ({}) does not exist",
        target_workspace_id
      );
    }

    info!(
      "Moved {} \"{}\" to workspace {}",
      window.handle,
      window.title_trunc(),
      target_workspace_id
    );
  }

  fn get_current_workspace_id_if_different(&mut self, target_workspace_id: WorkspaceId) -> Option<WorkspaceId> {
    let Some(current_workspace_id) = self.get_active_workspace_for_cursor_position(None) else {
      warn!("Failed to complete request: Unable to find the active workspace");
      return None;
    };

    if target_workspace_id == current_workspace_id {
      info!(
        "Ignored request because current and target workspaces are the same: {}",
        target_workspace_id
      );
      return None;
    }

    Some(current_workspace_id)
  }

  fn get_active_workspace_for_cursor_position(&mut self, recursion_depth: Option<usize>) -> Option<WorkspaceId> {
    let cursor_position = self.windows_api.get_cursor_position();
    let monitor_handle = self.windows_api.get_monitor_for_point(&cursor_position);
    let recursion_depth = recursion_depth.unwrap_or(0);
    if recursion_depth > MAX_RECURSION_DEPTH {
      error!("Failed to find active workspace for {monitor_handle} after [{recursion_depth}] attempts");
      return None;
    }

    let workspace_ids = self
      .workspaces
      .iter()
      .filter(|(id, workspace)| workspace.is_active() && id.monitor_handle == monitor_handle)
      .map(|(id, _)| id)
      .collect::<Vec<_>>();

    match workspace_ids.len() {
      0 => {
        warn!("No active workspace found for {monitor_handle}, will reinitialise workspaces...");
        self.initialise_workspaces();

        Some(self.get_active_workspace_for_cursor_position(Some(recursion_depth))?)
      }
      1 => Some(*workspace_ids[0]),
      _ => {
        error!(
          "Data inconsistency detected: Found multiple active workspaces for monitor [{monitor_handle}]: {:?}",
          workspace_ids
        );

        None
      }
    }
  }

  fn active_workspaces(&self) -> Vec<WorkspaceId> {
    self
      .workspaces
      .iter()
      .filter(|(_, workspace)| workspace.is_active())
      .map(|(id, _)| *id)
      .collect()
  }

  fn get_active_workspace(&mut self, workspace_id: &WorkspaceId) -> Option<&WorkspaceId> {
    self
      .workspaces
      .iter()
      .filter(|(_, workspace)| workspace.is_active())
      .map(|(id, _)| id)
      .find(|id| id.monitor_handle == workspace_id.monitor_handle)
  }

  fn set_active_workspace(&mut self, workspace_id: &WorkspaceId, is_active: bool) {
    self
      .workspaces
      .iter_mut()
      .filter(|(_, workspace)| workspace.id == *workspace_id)
      .for_each(|(_, workspace)| {
        workspace.set_active(is_active);
      });
  }

  fn log_active_workspaces(&self) {
    let workspaces = self.active_workspaces();
    debug!(
      "Found [{}] active workspaces: {}",
      workspaces.len(),
      workspaces.iter().map(|id| format!("{}", id)).collect::<Vec<_>>().join(", ")
    );
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api::MockWindowsApi;
  use crate::utils::{Monitor, Point, Rect, Workspace, WorkspaceId};
  use crate::utils::{Sizing, WindowHandle};
  use std::sync::OnceLock;

  static PRIMARY_MONITOR: OnceLock<Monitor> = OnceLock::new();
  static SECONDARY_MONITOR: OnceLock<Monitor> = OnceLock::new();
  static PRIMARY_INACTIVE_WORKSPACE: OnceLock<WorkspaceId> = OnceLock::new();
  static PRIMARY_ACTIVE_WORKSPACE: OnceLock<WorkspaceId> = OnceLock::new();
  static SECONDARY_ACTIVE_WORKSPACE: OnceLock<WorkspaceId> = OnceLock::new();
  static SECONDARY_INACTIVE_WORKSPACE: OnceLock<WorkspaceId> = OnceLock::new();

  fn primary_monitor() -> &'static Monitor {
    PRIMARY_MONITOR.get_or_init(Monitor::mock_1)
  }

  fn secondary_monitor() -> &'static Monitor {
    SECONDARY_MONITOR.get_or_init(Monitor::mock_2)
  }

  fn primary_active_workspace() -> &'static WorkspaceId {
    PRIMARY_ACTIVE_WORKSPACE.get_or_init(|| WorkspaceId::new(primary_monitor().handle, 1))
  }

  fn primary_inactive_workspace() -> &'static WorkspaceId {
    PRIMARY_INACTIVE_WORKSPACE.get_or_init(|| WorkspaceId::new(primary_monitor().handle, 2))
  }

  fn secondary_active_workspace() -> &'static WorkspaceId {
    SECONDARY_ACTIVE_WORKSPACE.get_or_init(|| WorkspaceId::new(secondary_monitor().handle, 1))
  }

  fn secondary_inactive_workspace() -> &'static WorkspaceId {
    SECONDARY_INACTIVE_WORKSPACE.get_or_init(|| WorkspaceId::new(secondary_monitor().handle, 2))
  }

  impl WorkspaceManager<MockWindowsApi> {
    pub fn default() -> Self {
      Self {
        workspaces: HashMap::new(),
        windows_api: MockWindowsApi::new(),
        margin: 10,
        additional_workspace_count: 0,
      }
    }

    /// Creates a new `WorkspaceManager` with a test window, two monitors, and two workspaces on each monitor.
    pub fn new_test(is_test_window_in_foreground: bool) -> Self {
      let window_handle = WindowHandle::new(1);
      let sizing = Sizing::new(50, 50, 50, 50);
      MockWindowsApi::add_or_update_window(
        window_handle,
        "Test Window".to_string(),
        sizing,
        false,
        false,
        is_test_window_in_foreground,
      );

      let primary_monitor = primary_monitor();
      let secondary_monitor = secondary_monitor();
      MockWindowsApi::place_window(window_handle, primary_monitor.handle);
      MockWindowsApi::set_cursor_position(Point::new(50, 50));
      MockWindowsApi::add_or_update_monitor(primary_monitor.handle, primary_monitor.monitor_area, true);
      MockWindowsApi::add_or_update_monitor(secondary_monitor.handle, secondary_monitor.monitor_area, false);

      let mock_api = MockWindowsApi;
      let primary_active_workspace_id = *primary_active_workspace();
      let primary_inactive_workspace_id = *primary_inactive_workspace();
      let secondary_active_workspace_id = *secondary_active_workspace();
      let secondary_inactive_workspace_id = *secondary_inactive_workspace();
      let margin = 10;

      WorkspaceManager {
        workspaces: HashMap::from([
          (
            primary_active_workspace_id,
            Workspace::new_active(primary_active_workspace_id, primary_monitor, margin),
          ),
          (
            primary_inactive_workspace_id,
            Workspace::new_inactive(primary_inactive_workspace_id, primary_monitor, margin),
          ),
          (
            secondary_active_workspace_id,
            Workspace::new_active(secondary_active_workspace_id, secondary_monitor, margin),
          ),
          (
            secondary_inactive_workspace_id,
            Workspace::new_inactive(secondary_inactive_workspace_id, secondary_monitor, margin),
          ),
        ]),
        windows_api: mock_api,
        margin,
        additional_workspace_count: 1,
      }
    }

    pub fn from_workspaces(workspaces: &[&Workspace], margin: i32) -> Self {
      let mut workspace_map = HashMap::new();
      for workspace in workspaces {
        workspace_map.insert(workspace.id, workspace.to_owned().clone());
      }

      Self {
        workspaces: workspace_map,
        windows_api: MockWindowsApi::new(),
        margin,
        additional_workspace_count: 1,
      }
    }
  }

  #[test]
  fn get_active_workspace_for_cursor_position_returns_workspace_if_one_active_workspace_found() {
    let mut workspace_manager = WorkspaceManager::new_test(false);
    // Cursor on primary monitor
    MockWindowsApi::set_cursor_position(Point::new(50, 50));

    let result = workspace_manager.get_active_workspace_for_cursor_position(None);

    assert_eq!(result, Some(*primary_active_workspace()));
  }

  #[test]
  fn get_active_workspace_for_cursor_position_reinitialises_workspaces_to_detect_monitor_changes() {
    let mut workspace_manager = WorkspaceManager::default();
    MockWindowsApi::set_cursor_position(Point::new(100, 100));
    // Monitor 5 is added after initialisation and can only be detected once workspaces for all known monitors
    // have been reinitialised
    let monitor_handle = MonitorHandle::from(5);
    MockWindowsApi::add_or_update_monitor(monitor_handle, Rect::new(100, 0, 200, 200), true);

    let result = workspace_manager.get_active_workspace_for_cursor_position(None);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), WorkspaceId::new(monitor_handle, 1));
  }

  #[test]
  fn get_active_workspace_for_cursor_position_returns_none_if_recursion_depth_exceeded() {
    let mut workspace_manager = WorkspaceManager::default();
    MockWindowsApi::set_cursor_position(Point::new(100, 100));
    // Monitor 5 is added after initialisation and can only be detected once workspaces for all known monitors
    // have been reinitialised which must not happen in this case
    MockWindowsApi::add_or_update_monitor(MonitorHandle::from(5), Rect::new(0, 0, 200, 200), true);

    let result = workspace_manager.get_active_workspace_for_cursor_position(Some(MAX_RECURSION_DEPTH + 1));

    assert!(result.is_none());
  }

  #[test]
  fn get_ordered_workspace_ids_left_to_right() {
    let left_monitor = Monitor::new_test(1, Rect::new(0, 0, 99, 100));
    let center_monitor = Monitor::new_test(2, Rect::new(100, 0, 199, 100));
    let right_monitor = Monitor::new_test(3, Rect::new(200, 0, 299, 100));
    let left_workspace = Workspace::new_test(WorkspaceId::new(left_monitor.handle, 1), &left_monitor);
    let center_workspace = Workspace::new_test(WorkspaceId::new(center_monitor.handle, 1), &center_monitor);
    let right_workspace = Workspace::new_test(WorkspaceId::new(right_monitor.handle, 1), &right_monitor);
    let workspace_manager = WorkspaceManager::from_workspaces(&[&left_workspace, &center_workspace, &right_workspace], 0);

    let ordered_workspaces = workspace_manager.get_ordered_workspace_ids();

    assert_eq!(ordered_workspaces.len(), 3);
    assert_eq!(ordered_workspaces[0], left_workspace.id);
    assert_eq!(ordered_workspaces[1], center_workspace.id);
    assert_eq!(ordered_workspaces[2], right_workspace.id);
  }

  #[test]
  fn get_ordered_workspace_ids_top_to_bottom() {
    let top_monitor = Monitor::new_test(1, Rect::new(0, 0, 100, 99));
    let center_monitor = Monitor::new_test(2, Rect::new(0, 100, 100, 199));
    let bottom_monitor = Monitor::new_test(3, Rect::new(0, 200, 100, 299));
    let top_workspace = Workspace::new_test(WorkspaceId::new(top_monitor.handle, 1), &top_monitor);
    let center_workspace = Workspace::new_test(WorkspaceId::new(center_monitor.handle, 1), &center_monitor);
    let bottom_workspace = Workspace::new_test(WorkspaceId::new(bottom_monitor.handle, 1), &bottom_monitor);
    let workspace_manager = WorkspaceManager::from_workspaces(&[&top_workspace, &center_workspace, &bottom_workspace], 0);

    let ordered_workspaces = workspace_manager.get_ordered_workspace_ids();

    assert_eq!(ordered_workspaces.len(), 3);
    assert_eq!(ordered_workspaces[0], top_workspace.id);
    assert_eq!(ordered_workspaces[1], center_workspace.id);
    assert_eq!(ordered_workspaces[2], bottom_workspace.id);
  }

  #[test]
  fn get_ordered_workspace_ids_with_multiple_workspaces_on_same_monitor() {
    let top_monitor = Monitor::new_test(1, Rect::new(0, 0, 100, 99));
    let bottom_monitor = Monitor::new_test(3, Rect::new(0, 200, 100, 299));
    let top_workspace_1 = Workspace::new_test(WorkspaceId::new(top_monitor.handle, 1), &top_monitor);
    let top_workspace_2 = Workspace::new_test(WorkspaceId::new(top_monitor.handle, 2), &top_monitor);
    let bottom_workspace_1 = Workspace::new_test(WorkspaceId::new(bottom_monitor.handle, 1), &bottom_monitor);
    let bottom_workspace_2 = Workspace::new_test(WorkspaceId::new(bottom_monitor.handle, 2), &bottom_monitor);
    let workspace_manager = WorkspaceManager::from_workspaces(
      &[&top_workspace_1, &top_workspace_2, &bottom_workspace_1, &bottom_workspace_2],
      0,
    );

    let ordered_workspaces = workspace_manager.get_ordered_workspace_ids();

    assert_eq!(ordered_workspaces.len(), 4);
    assert_eq!(ordered_workspaces[0], top_workspace_1.id);
    assert_eq!(ordered_workspaces[1], top_workspace_2.id);
    assert_eq!(ordered_workspaces[2], bottom_workspace_1.id);
    assert_eq!(ordered_workspaces[3], bottom_workspace_2.id);
  }

  #[test]
  fn switch_workspace_when_target_workspace_has_no_windows() {
    // Given the current workspace has one window and target workspace is not active
    let mut workspace_manager = WorkspaceManager::new_test(true);
    let target_workspace_id = primary_inactive_workspace();
    let active_workspaces = workspace_manager.active_workspaces();
    assert_eq!(active_workspaces.len(), 2);
    assert!(active_workspaces.contains(primary_active_workspace()));
    assert!(active_workspaces.contains(secondary_active_workspace()));

    // When the user switches to the target workspace
    workspace_manager.switch_workspace(*target_workspace_id);

    // Then the active workspace for the relevant monitor is updated
    let active_workspaces = workspace_manager.active_workspaces();
    assert_eq!(active_workspaces.len(), 2, "The number of active workspaces shouldn't change");
    assert!(active_workspaces.contains(target_workspace_id));
    assert!(active_workspaces.contains(secondary_active_workspace()));
    assert_eq!(
      workspace_manager.windows_api.get_foreground_window().unwrap(),
      WindowHandle::new(1),
      "The foreground window should not be changed because the target workspace doesn't have any windows"
    );
    assert_eq!(
      workspace_manager.windows_api.get_cursor_position(),
      Point::new(960, 540),
      "The cursor position should be set to the center of the monitor because the target workspace doesn't have any windows"
    );

    // And the window on the original workspace has been stored
    let original_workspace = workspace_manager
      .workspaces
      .get(primary_active_workspace())
      .expect("Original workspace not found");
    assert_eq!(
      original_workspace.get_windows().len(),
      1,
      "The original workspace should still have one window"
    );
  }

  #[test]
  fn switch_workspace_sets_largest_target_workspace_window_as_foreground_window() {
    // Given the current workspace has one window and the target workspace, which has two windows, is not active
    let small_window = Window::from(2, "Small Window".to_string(), Rect::new(0, 0, 50, 50));
    let large_window = Window::from(3, "Large Window".to_string(), Rect::new(0, 0, 500, 500));
    MockWindowsApi::add_or_update_window(
      small_window.handle,
      small_window.title.clone(),
      small_window.rect.into(),
      false,
      false,
      true,
    );
    MockWindowsApi::add_or_update_window(
      large_window.handle,
      large_window.title.clone(),
      large_window.rect.into(),
      false,
      false,
      true,
    );
    let mut workspace_manager = WorkspaceManager::new_test(true);
    let target_workspace_id = primary_inactive_workspace();
    if let Some(target_workspace) = workspace_manager.workspaces.get_mut(target_workspace_id) {
      target_workspace.store_and_hide_windows(vec![small_window, large_window], 1.into(), &workspace_manager.windows_api);
    }
    let active_workspaces = workspace_manager.active_workspaces();
    assert_eq!(active_workspaces.len(), 2);
    assert!(!active_workspaces.contains(target_workspace_id));

    // When the user switches to the target workspace
    workspace_manager.switch_workspace(*target_workspace_id);

    // Then the active workspace for the relevant monitor is updated and the large window is brought to the foreground
    let active_workspaces = workspace_manager.active_workspaces();
    assert_eq!(active_workspaces.len(), 2, "The number of active workspaces shouldn't change");
    assert!(active_workspaces.contains(target_workspace_id));
    assert!(active_workspaces.contains(secondary_active_workspace()));
    assert_eq!(
      workspace_manager.windows_api.get_foreground_window().unwrap(),
      WindowHandle::new(3),
      "The foreground window should change to the largest window on the target workspace"
    );
    assert_eq!(
      workspace_manager.windows_api.get_cursor_position(),
      Point::new(250, 250),
      "The cursor position should be set to the center of the largest window"
    );
  }

  #[test]
  fn move_window_to_different_workspace_on_same_monitor() {
    // Given the primary monitor has an active workspace with one, visible foreground window
    MockWindowsApi::place_window(WindowHandle::new(1), primary_monitor().handle);
    let workspace_id = primary_inactive_workspace();
    let mut workspace_manager = WorkspaceManager::new_test(true);

    // When the user moves a window to a different workspace on the same monitor
    workspace_manager.move_window_to_workspace(*workspace_id);

    // Then the window appears in the target workspace
    let target_workspace = workspace_manager
      .workspaces
      .get(workspace_id)
      .expect("Target workspace not found");
    assert_eq!(target_workspace.get_windows().len(), 1);
    assert_eq!(target_workspace.get_window_state_info().len(), 1);
    let windows = target_workspace.get_windows();
    let window = windows.first().expect("Failed to retrieve window title");
    assert_eq!(window.title, "Test Window");
    assert_eq!(
      window.center,
      Point::new(75, 75),
      "Window center should not be updated since it wasn't moved to a different monitor"
    );

    // But the active workspace has not changed
    let active_workspaces = workspace_manager.active_workspaces();
    assert_eq!(active_workspaces.len(), 2);
    assert!(!active_workspaces.contains(workspace_id));
    assert!(active_workspaces.contains(primary_active_workspace()));
    assert!(active_workspaces.contains(secondary_active_workspace()));
  }

  #[test]
  fn move_window_to_active_workspace_on_different_monitor() {
    // Given the primary monitor has an active workspace with one, visible foreground window
    MockWindowsApi::place_window(WindowHandle::new(1), primary_monitor().handle);
    let mut workspace_manager = WorkspaceManager::new_test(true);

    // When the user moves a window to a different workspace on a different monitor
    let target_workspace_id = secondary_active_workspace();
    workspace_manager.move_window_to_workspace(*target_workspace_id);

    // Then the window is not stored in the target workspace
    let target_workspace = workspace_manager
      .workspaces
      .get(target_workspace_id)
      .expect("Target workspace not found");
    assert_eq!(target_workspace.get_windows().len(), 0);
    assert_eq!(target_workspace.get_window_state_info().len(), 0);

    // But the window is still in the foreground and was moved to the target workspace
    let active_window = workspace_manager
      .windows_api
      .get_foreground_window()
      .expect("Failed to retrieve window");
    let window_placement = workspace_manager
      .windows_api
      .get_window_placement(active_window)
      .expect("Failed to retrieve window placement");
    assert_eq!(active_window, WindowHandle::new(1));
    assert_eq!(
      window_placement.normal_position,
      Rect::new(-425, 250, -375, 300),
      "Window placement should be updated since it was moved to a different monitor"
    );

    // And the cursor position is set to the center of the target workspace (excl. taskbar)
    assert_eq!(workspace_manager.windows_api.get_cursor_position(), Point::new(-400, 275));

    // And the active workspaces have not changed
    let active_workspaces = workspace_manager.active_workspaces();
    assert_eq!(active_workspaces.len(), 2);
    assert!(active_workspaces.contains(target_workspace_id));
    assert!(active_workspaces.contains(primary_active_workspace()));
  }

  #[test]
  fn move_window_to_inactive_workspace_on_different_monitor() {
    // Given the primary monitor has an active workspace with one, visible foreground window
    MockWindowsApi::place_window(WindowHandle::new(1), primary_monitor().handle);
    let mut workspace_manager = WorkspaceManager::new_test(true);
    assert_eq!(workspace_manager.windows_api.get_all_visible_windows().len(), 1);

    // When the user moves a window to a different workspace on a different monitor
    let target_workspace_id = secondary_inactive_workspace();
    workspace_manager.move_window_to_workspace(*target_workspace_id);

    // Then the window appears in the target workspace
    let target_workspace = workspace_manager
      .workspaces
      .get(target_workspace_id)
      .expect("Target workspace not found");
    assert_eq!(target_workspace.get_windows().len(), 1);
    assert_eq!(target_workspace.get_window_state_info().len(), 1);
    let windows = target_workspace.get_windows();
    let window = windows.first().expect("Failed to retrieve window title");
    assert_eq!(window.title, "Test Window");
    assert_eq!(window.center, Point::new(-400, 275));

    // And the window is no longer visible
    assert!(workspace_manager.windows_api.get_all_visible_windows().is_empty());

    // But the active workspace has not changed
    let active_workspaces = workspace_manager.active_workspaces();
    assert_eq!(active_workspaces.len(), 2);
    assert!(active_workspaces.contains(primary_active_workspace()));
    assert!(active_workspaces.contains(secondary_active_workspace()));
  }

  #[test]
  fn move_window_clamps_size_of_large_window_when_moving_to_another_active_workspace() {
    // Given the primary monitor has an active workspace with two, visible windows, one of which is the foreground
    // window, and it is too large to fit in the target workspace
    let large_window = Window::from(2, "Large Window".to_string(), Rect::new(0, 0, 1920, 1080));
    MockWindowsApi::add_or_update_window(
      large_window.handle,
      large_window.title.clone(),
      large_window.rect.into(),
      false,
      false,
      true,
    );
    MockWindowsApi::place_window(large_window.handle, primary_monitor().handle);
    let mut workspace_manager = WorkspaceManager::new_test(false);

    // When the user moves a window to a different workspace on a different monitor
    let target_workspace_id = secondary_active_workspace();
    workspace_manager.move_window_to_workspace(*target_workspace_id);

    // Then the window was moved to the target workspace, is still in the foreground, and its size was clamped to
    // fit within the target workspace
    let active_window = workspace_manager
      .windows_api
      .get_foreground_window()
      .expect("Failed to retrieve window");
    let window_title = workspace_manager.windows_api.get_window_title(&active_window);
    let window_placement = workspace_manager
      .windows_api
      .get_window_placement(active_window)
      .expect("Failed to retrieve window placement");
    assert_eq!(window_title, large_window.title);
    assert_eq!(active_window, large_window.handle);
    assert_eq!(
      window_placement.normal_position,
      Rect::new(-790, 10, -10, 540),
      "Window placement should be updated since it was moved to a different monitor"
    );

    // And the cursor position is set to the center of the target workspace (excl. taskbar)
    assert_eq!(workspace_manager.windows_api.get_cursor_position(), Point::new(-400, 275));
  }
}
