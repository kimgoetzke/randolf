use crate::api::WindowsApi;
use crate::common::{Monitors, PersistentWorkspaceId, TransientWorkspaceId, Workspace};
use crate::workspace_guard::WorkspaceGuard;
use std::collections::HashMap;

pub struct WorkspaceManager<T: WindowsApi> {
  pub(crate) workspaces: HashMap<PersistentWorkspaceId, Workspace>,
  pub(crate) windows_api: T,
  margin: i32,
  additional_workspace_count: i32,
}

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

  pub(crate) fn create_workspace_id_map(&self, monitors: Monitors) -> HashMap<PersistentWorkspaceId, TransientWorkspaceId> {
    let mut workspace_id_map = HashMap::new();
    for workspace in self.workspaces.values() {
      if let Some(monitor) = monitors.get_by_id(&workspace.id.monitor_id) {
        workspace_id_map.insert(workspace.id, TransientWorkspaceId::from(workspace.id, monitor.handle));
      }
    }

    workspace_id_map
  }

  fn initialise_workspaces(&mut self) {
    let mut workspaces = HashMap::new();
    let all_monitors = self.windows_api.get_all_monitors();
    for monitor in all_monitors.get_all() {
      if monitor.is_primary {
        for layer in 1..=self.additional_workspace_count + 1 {
          let id = PersistentWorkspaceId::new(monitor.id, layer as usize);
          let workspace = if layer == 1 {
            Workspace::new_active(id, monitor, self.margin)
          } else {
            Workspace::new_inactive(id, monitor, self.margin)
          };
          workspaces.insert(id, workspace);
        }
      } else {
        let id = PersistentWorkspaceId::new(monitor.id, 1);
        workspaces.insert(id, Workspace::new_active(id, monitor, self.margin));
      }
    }
    self.workspaces = workspaces;
  }

  pub fn get_ordered_permanent_workspace_ids(&mut self) -> Vec<PersistentWorkspaceId> {
    let guard = WorkspaceGuard::new(self);
    guard.get_ordered_workspace_ids()
  }

  pub fn switch_workspace(&mut self, target_workspace_id: PersistentWorkspaceId) {
    let mut guard = WorkspaceGuard::new(self);
    guard.switch_workspace(target_workspace_id);
  }

  pub fn move_window_to_workspace(&mut self, target_workspace_id: PersistentWorkspaceId) {
    let mut guard = WorkspaceGuard::new(self);
    guard.move_window_to_workspace(target_workspace_id);
  }

  pub fn restore_all_managed_windows(&mut self) {
    let mut guard = WorkspaceGuard::new(self);
    guard.restore_all_managed_windows();
  }
}

#[cfg(test)]
pub mod tests {
  use super::*;
  use crate::api::MockWindowsApi;
  use crate::common::{Monitor, MonitorHandle, Point, Rect, Sizing, TransientWorkspaceId, Window, WindowHandle, Workspace};
  use std::sync::OnceLock;

  static PRIMARY_MONITOR: OnceLock<Monitor> = OnceLock::new();
  static SECONDARY_MONITOR: OnceLock<Monitor> = OnceLock::new();
  static PRIMARY_INACTIVE_WORKSPACE: OnceLock<TransientWorkspaceId> = OnceLock::new();
  static PRIMARY_ACTIVE_WORKSPACE: OnceLock<TransientWorkspaceId> = OnceLock::new();
  static SECONDARY_ACTIVE_WORKSPACE: OnceLock<TransientWorkspaceId> = OnceLock::new();
  static SECONDARY_INACTIVE_WORKSPACE: OnceLock<TransientWorkspaceId> = OnceLock::new();

  pub fn primary_monitor() -> &'static Monitor {
    PRIMARY_MONITOR.get_or_init(Monitor::mock_1)
  }

  fn secondary_monitor() -> &'static Monitor {
    SECONDARY_MONITOR.get_or_init(Monitor::mock_2)
  }

  pub fn primary_active_ws_id() -> &'static TransientWorkspaceId {
    PRIMARY_ACTIVE_WORKSPACE.get_or_init(|| TransientWorkspaceId::new(primary_monitor().id, primary_monitor().handle, 1))
  }

  fn primary_inactive_ws_id() -> &'static TransientWorkspaceId {
    PRIMARY_INACTIVE_WORKSPACE.get_or_init(|| TransientWorkspaceId::new(primary_monitor().id, primary_monitor().handle, 2))
  }

  fn secondary_active_ws_id() -> &'static TransientWorkspaceId {
    SECONDARY_ACTIVE_WORKSPACE
      .get_or_init(|| TransientWorkspaceId::new(secondary_monitor().id, secondary_monitor().handle, 1))
  }

  fn secondary_inactive_ws_id() -> &'static TransientWorkspaceId {
    SECONDARY_INACTIVE_WORKSPACE
      .get_or_init(|| TransientWorkspaceId::new(secondary_monitor().id, secondary_monitor().handle, 2))
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
      MockWindowsApi::add_monitor_with_full_details(
        primary_monitor.id,
        primary_monitor.handle,
        primary_monitor.monitor_area,
        primary_monitor.work_area,
        true,
      );
      MockWindowsApi::add_monitor_with_full_details(
        secondary_monitor.id,
        secondary_monitor.handle,
        secondary_monitor.monitor_area,
        secondary_monitor.work_area,
        false,
      );

      let mock_api = MockWindowsApi;
      let primary_active_workspace_id = *primary_active_ws_id();
      let primary_inactive_workspace_id = *primary_inactive_ws_id();
      let secondary_active_workspace_id = *secondary_active_ws_id();
      let secondary_inactive_workspace_id = *secondary_inactive_ws_id();
      let margin = 10;

      WorkspaceManager {
        workspaces: HashMap::from([
          (
            primary_active_workspace_id.into(),
            Workspace::new_active(
              PersistentWorkspaceId::from(primary_active_workspace_id),
              primary_monitor,
              margin,
            ),
          ),
          (
            primary_inactive_workspace_id.into(),
            Workspace::new_inactive(primary_inactive_workspace_id.into(), primary_monitor, margin),
          ),
          (
            secondary_active_workspace_id.into(),
            Workspace::new_active(secondary_active_workspace_id.into(), secondary_monitor, margin),
          ),
          (
            secondary_inactive_workspace_id.into(),
            Workspace::new_inactive(secondary_inactive_workspace_id.into(), secondary_monitor, margin),
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

    fn active_workspaces(&self) -> Vec<TransientWorkspaceId> {
      self
        .workspaces
        .iter()
        .filter(|(_, workspace)| workspace.is_active())
        .map(|(id, ws)| TransientWorkspaceId::new(id.monitor_id, MonitorHandle::from(ws.monitor_handle), id.workspace))
        .collect()
    }
  }

  #[test]
  fn get_ordered_workspace_ids_left_to_right() {
    let left_monitor = Monitor::new_test(1, Rect::new(0, 0, 99, 100));
    let center_monitor = Monitor::new_test(2, Rect::new(100, 0, 199, 100));
    let right_monitor = Monitor::new_test(3, Rect::new(200, 0, 299, 100));
    let left_workspace = Workspace::new_test(PersistentWorkspaceId::new(left_monitor.id, 1), &left_monitor);
    let center_workspace = Workspace::new_test(PersistentWorkspaceId::new(center_monitor.id, 1), &center_monitor);
    let right_workspace = Workspace::new_test(PersistentWorkspaceId::new(right_monitor.id, 1), &right_monitor);
    let mut workspace_manager =
      WorkspaceManager::from_workspaces(&[&left_workspace, &center_workspace, &right_workspace], 0);

    let ordered_workspaces = workspace_manager.get_ordered_permanent_workspace_ids();

    assert_eq!(ordered_workspaces.len(), 3);
    assert_eq!(ordered_workspaces[0], left_workspace.id);
    assert_eq!(ordered_workspaces[1], center_workspace.id,);
    assert_eq!(ordered_workspaces[2], right_workspace.id,);
  }

  #[test]
  fn get_ordered_workspace_ids_top_to_bottom() {
    let top_monitor = Monitor::new_test(1, Rect::new(0, 0, 100, 99));
    let center_monitor = Monitor::new_test(2, Rect::new(0, 100, 100, 199));
    let bottom_monitor = Monitor::new_test(3, Rect::new(0, 200, 100, 299));
    let top_workspace = Workspace::new_test(PersistentWorkspaceId::new(top_monitor.id, 1), &top_monitor);
    let center_workspace = Workspace::new_test(PersistentWorkspaceId::new(center_monitor.id, 1), &center_monitor);
    let bottom_workspace = Workspace::new_test(PersistentWorkspaceId::new(bottom_monitor.id, 1), &bottom_monitor);
    let mut workspace_manager =
      WorkspaceManager::from_workspaces(&[&top_workspace, &center_workspace, &bottom_workspace], 0);

    let ordered_workspaces = workspace_manager.get_ordered_permanent_workspace_ids();

    assert_eq!(ordered_workspaces.len(), 3);
    assert_eq!(ordered_workspaces[0], top_workspace.id,);
    assert_eq!(ordered_workspaces[1], center_workspace.id);
    assert_eq!(ordered_workspaces[2], bottom_workspace.id,);
  }

  #[test]
  fn get_ordered_workspace_ids_with_multiple_workspaces_on_same_monitor() {
    let top_monitor = Monitor::new_test(1, Rect::new(0, 0, 100, 99));
    let bottom_monitor = Monitor::new_test(3, Rect::new(0, 200, 100, 299));
    let top_workspace_1 = Workspace::new_test(PersistentWorkspaceId::new(top_monitor.id, 1), &top_monitor);
    let top_workspace_2 = Workspace::new_test(PersistentWorkspaceId::new(top_monitor.id, 2), &top_monitor);
    let bottom_workspace_1 = Workspace::new_test(PersistentWorkspaceId::new(bottom_monitor.id, 1), &bottom_monitor);
    let bottom_workspace_2 = Workspace::new_test(PersistentWorkspaceId::new(bottom_monitor.id, 2), &bottom_monitor);
    let mut workspace_manager = WorkspaceManager::from_workspaces(
      &[&top_workspace_1, &top_workspace_2, &bottom_workspace_1, &bottom_workspace_2],
      0,
    );

    let ordered_workspaces = workspace_manager.get_ordered_permanent_workspace_ids();

    assert_eq!(ordered_workspaces.len(), 4);
    assert_eq!(ordered_workspaces[0], top_workspace_1.id);
    assert_eq!(ordered_workspaces[1], top_workspace_2.id,);
    assert_eq!(ordered_workspaces[2], bottom_workspace_1.id);
    assert_eq!(ordered_workspaces[3], bottom_workspace_2.id);
  }

  #[test]
  fn switch_workspace_when_target_workspace_has_no_windows() {
    // Given the current workspace has one window and target workspace is not active
    let mut workspace_manager = WorkspaceManager::new_test(true);
    let transient_target_ws_id = primary_inactive_ws_id();
    let active_workspaces = workspace_manager.active_workspaces();
    assert_eq!(active_workspaces.len(), 2);
    assert!(active_workspaces.contains(primary_active_ws_id()));
    assert!(active_workspaces.contains(secondary_active_ws_id()));
    let persistent_target_ws_id = PersistentWorkspaceId::from(*transient_target_ws_id);

    // When the user switches to the target workspace
    workspace_manager.switch_workspace(persistent_target_ws_id);

    // Then the active workspace for the relevant monitor is updated
    let active_workspaces = workspace_manager.active_workspaces();
    assert_eq!(active_workspaces.len(), 2, "The number of active workspaces shouldn't change");
    assert!(active_workspaces.contains(transient_target_ws_id));
    assert!(active_workspaces.contains(secondary_active_ws_id()));
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
      .get(&(*primary_active_ws_id()).into())
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
    let small_window = Window::new_test(2, Rect::new(0, 0, 50, 50));
    let large_window = Window::new_test(3, Rect::new(0, 0, 500, 500));
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
    let target_workspace_id = primary_inactive_ws_id();
    if let Some(target_workspace) = workspace_manager.workspaces.get_mut(&(*target_workspace_id).into()) {
      target_workspace.store_and_hide_windows(vec![small_window, large_window], 1.into(), &workspace_manager.windows_api);
    }
    let active_workspaces = workspace_manager.active_workspaces();
    assert_eq!(active_workspaces.len(), 2);
    assert!(!active_workspaces.contains(target_workspace_id));

    // When the user switches to the target workspace
    workspace_manager.switch_workspace(PersistentWorkspaceId::from(*target_workspace_id));

    // Then the active workspace for the relevant monitor is updated and the large window is brought to the foreground
    let active_workspaces = workspace_manager.active_workspaces();
    assert_eq!(active_workspaces.len(), 2, "The number of active workspaces shouldn't change");
    assert!(active_workspaces.contains(target_workspace_id));
    assert!(active_workspaces.contains(secondary_active_ws_id()));
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
    let workspace_id = primary_inactive_ws_id();
    let mut workspace_manager = WorkspaceManager::new_test(true);

    // When the user moves a window to a different workspace on the same monitor
    workspace_manager.move_window_to_workspace(PersistentWorkspaceId::from(*workspace_id));

    // Then the window appears in the target workspace
    let target_workspace = workspace_manager
      .workspaces
      .get(&(*workspace_id).into())
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
    assert!(active_workspaces.contains(primary_active_ws_id()));
    assert!(active_workspaces.contains(secondary_active_ws_id()));
  }

  #[test]
  fn move_window_to_active_workspace_on_different_monitor() {
    // Given the primary monitor has an active workspace with one, visible foreground window
    MockWindowsApi::place_window(WindowHandle::new(1), primary_monitor().handle);
    let mut workspace_manager = WorkspaceManager::new_test(true);

    // When the user moves a window to a different workspace on a different monitor
    let target_workspace_id = secondary_active_ws_id();
    workspace_manager.move_window_to_workspace(PersistentWorkspaceId::from(*target_workspace_id));

    // Then the window is not stored in the target workspace
    let target_workspace = workspace_manager
      .workspaces
      .get(&(*target_workspace_id).into())
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
    assert!(active_workspaces.contains(primary_active_ws_id()));
  }

  #[test]
  fn move_window_to_inactive_workspace_on_different_monitor() {
    // Given the primary monitor has an active workspace with one, visible foreground window
    MockWindowsApi::place_window(WindowHandle::new(1), primary_monitor().handle);
    let mut workspace_manager = WorkspaceManager::new_test(true);
    assert_eq!(workspace_manager.windows_api.get_all_visible_windows().len(), 1);

    // When the user moves a window to a different workspace on a different monitor
    let target_workspace_id = secondary_inactive_ws_id();
    workspace_manager.move_window_to_workspace(PersistentWorkspaceId::from(*target_workspace_id));

    // Then the window appears in the target workspace
    let target_workspace = workspace_manager
      .workspaces
      .get(&(*target_workspace_id).into())
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
    assert!(active_workspaces.contains(primary_active_ws_id()));
    assert!(active_workspaces.contains(secondary_active_ws_id()));
  }

  #[test]
  fn move_window_clamps_size_of_large_window_when_moving_to_another_active_workspace() {
    // Given the primary monitor has an active workspace with two, visible windows, one of which is the foreground
    // window, and it is too large to fit in the target workspace
    let large_window = Window::new_test(2, Rect::new(0, 0, 1920, 1080));
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
    let target_workspace_id = secondary_active_ws_id();
    workspace_manager.move_window_to_workspace(PersistentWorkspaceId::from(*target_workspace_id));

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

  #[test]
  fn restore_all_managed_windows_restores_windows_for_all_workspaces() {
    let w_2 = Window::new_test(2, Rect::new(0, 0, 100, 100));
    let w_3 = Window::new_test(3, Rect::new(100, 100, 200, 200));
    let w_4 = Window::new_test(4, Rect::new(0, 0, 300, 300));
    MockWindowsApi::add_or_update_window(w_2.handle, w_2.title.clone(), w_2.rect.into(), false, false, false);
    MockWindowsApi::add_or_update_window(w_3.handle, w_3.title.clone(), w_3.rect.into(), false, false, false);
    MockWindowsApi::add_or_update_window(w_4.handle, w_4.title.clone(), w_4.rect.into(), false, false, false);
    let mut workspace_manager = WorkspaceManager::new_test(false);
    if let Some(workspace) = workspace_manager.workspaces.get_mut(&(*primary_inactive_ws_id()).into()) {
      workspace.store_and_hide_windows(
        vec![w_2, w_3],
        primary_active_ws_id().monitor_handle,
        &workspace_manager.windows_api,
      );
    }
    if let Some(workspace) = workspace_manager.workspaces.get_mut(&(*secondary_inactive_ws_id()).into()) {
      workspace.store_and_hide_windows(
        vec![w_4],
        primary_active_ws_id().monitor_handle,
        &workspace_manager.windows_api,
      );
    }
    assert_eq!(workspace_manager.windows_api.get_all_visible_windows().len(), 1);

    workspace_manager.restore_all_managed_windows();

    assert_eq!(workspace_manager.windows_api.get_all_visible_windows().len(), 4);
    workspace_manager.workspaces.iter().for_each(|(_, workspace)| {
      assert_eq!(workspace.get_windows().len(), 0);
      assert_eq!(workspace.get_window_state_info().len(), 0);
    });
  }

  #[test]
  fn restore_all_managed_windows_does_nothing_if_no_windows_are_stored() {
    let mut workspace_manager = WorkspaceManager::new_test(false);
    workspace_manager.workspaces.iter().for_each(|(_, workspace)| {
      assert_eq!(workspace.get_windows().len(), 0);
    });

    workspace_manager.restore_all_managed_windows();

    assert_eq!(workspace_manager.windows_api.get_all_visible_windows().len(), 1);
  }
}
