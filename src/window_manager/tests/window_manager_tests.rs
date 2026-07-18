use crate::api::{MockWindowsApi, WindowsApi};
use crate::common::{Direction, MonitorHandle, PersistentWorkspaceId, Point, Rect, Sizing, WindowHandle, WindowPlacement};
use crate::configuration_provider::{ConfigurationProvider, Layout};
use crate::utils::create_temp_directory;
use crate::window_manager::WindowManager;
use crate::window_manager::tests::test_support::scrolling_manager;
use crate::workspace_manager::WorkspaceManager;
use std::sync::{Arc, Mutex};

#[test]
fn mixed_layout_routes_move_by_foreground_monitor() {
  MockWindowsApi::reset();
  let directory = create_temp_directory();
  let workspace_manager = WorkspaceManager::new_test(true, directory.path().join("workspaces.toml"));
  let configuration_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
  configuration_provider
    .lock()
    .unwrap()
    .set_monitor_layout("DISPLAY1", Layout::Scrolling);
  let secondary = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    secondary,
    "Secondary".to_string(),
    Sizing::new(-700, 50, 400, 300),
    false,
    false,
    true,
  );
  MockWindowsApi::place_window(secondary, 2.into());
  let mut manager = WindowManager {
    configuration_provider,
    placement: Default::default(),
    allow_moving_cursor_after_close_or_minimise: true,
    scrolling: Default::default(),
    workspace_manager,
    virtual_desktop_manager: None,
    windows_api: MockWindowsApi,
  };

  manager.move_window(Direction::Up);
  assert_eq!(
    manager.windows_api.get_window_placement(secondary).unwrap(),
    WindowPlacement::new_from_sizing(Sizing::top_half_of_screen(Rect::new(-800, 0, 0, 550), 20))
  );
  MockWindowsApi::set_foreground_window(1.into());
  let primary_before = manager.windows_api.get_window_placement(1.into()).unwrap();
  manager.move_window(Direction::Up);
  assert_eq!(manager.windows_api.get_window_placement(1.into()).unwrap(), primary_before);
}

#[test]
fn close_window_does_close_window() {
  let window_handle = WindowHandle::new(1);
  let monitor_handle = MonitorHandle::from(1);
  MockWindowsApi::add_or_update_window(window_handle, "Test".to_string(), Sizing::default(), false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.close_window();

  assert!(
    !manager
      .windows_api
      .get_all_visible_windows()
      .iter()
      .any(|w| w.handle == window_handle)
  );
}

#[test]
fn close_window_moves_cursor_to_closest_window_when_enabled() {
  let window_handle_1 = WindowHandle::new(1);
  let window_handle_2 = WindowHandle::new(2);
  let monitor_handle = MonitorHandle::from(1);
  MockWindowsApi::add_or_update_window(window_handle_1, "Test 1".to_string(), Sizing::default(), false, false, true);
  let sizing = Sizing::new(0, 0, 100, 100);
  MockWindowsApi::add_or_update_window(window_handle_2, "Test 2".to_string(), sizing, false, false, false);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
  MockWindowsApi::place_window(window_handle_1, monitor_handle);
  MockWindowsApi::place_window(window_handle_2, monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.close_window();

  assert!(
    !manager
      .windows_api
      .get_all_visible_windows()
      .iter()
      .any(|w| w.handle == window_handle_1)
  );
  assert_eq!(manager.windows_api.get_cursor_position(), Point::new(50, 50));
}

#[test]
fn close_window_fails_silently() {
  let mut manager = WindowManager::default(MockWindowsApi);
  assert!(manager.windows_api.get_all_visible_windows().is_empty());

  manager.close_window();

  assert!(manager.windows_api.get_all_visible_windows().is_empty());
}

#[test]
fn minimise_window_when_no_other_window_present() {
  let window_handle = WindowHandle::new(1);
  MockWindowsApi::add_or_update_window(window_handle, "Test".to_string(), Sizing::default(), false, false, true);
  let mut manager = WindowManager::default(MockWindowsApi);
  assert!(!manager.windows_api.is_window_minimised(window_handle));

  manager.minimise_window();

  assert!(manager.windows_api.is_window_minimised(window_handle));
  assert_eq!(manager.windows_api.get_cursor_position(), Point::default());
  assert_eq!(manager.windows_api.get_foreground_window(), None);
}

#[test]
fn minimise_window_when_another_window_is_present() {
  let window_handle = WindowHandle::new(1);
  let sizing = Sizing::new(50, 50, 100, 100);
  let other_window_handle = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(window_handle, "Test".to_string(), Sizing::default(), false, false, true);
  MockWindowsApi::add_or_update_window(other_window_handle, "Other".to_string(), sizing.clone(), false, false, false);
  let mut manager = WindowManager::default(MockWindowsApi);
  assert!(!manager.windows_api.is_window_minimised(window_handle));

  manager.minimise_window();

  assert!(manager.windows_api.is_window_minimised(window_handle));
  assert_eq!(
    manager.windows_api.get_cursor_position(),
    Point::from_center_of_sizing(&sizing),
    "Cursor position should be set to the center of the closest, non-minimised window"
  );
  assert_eq!(manager.windows_api.get_foreground_window(), Some(other_window_handle));
}
#[test]
fn reconciliation_only_manages_scrolling_monitors() {
  MockWindowsApi::reset();
  let directory = create_temp_directory();
  let workspace_manager = WorkspaceManager::new_test(true, directory.path().join("workspaces.toml"));
  let configuration_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
  configuration_provider
    .lock()
    .unwrap()
    .set_monitor_layout("DISPLAY1", Layout::Scrolling);
  let secondary = WindowHandle::new(2);
  let original = Sizing::new(-700, 50, 100, 100);
  MockWindowsApi::add_or_update_window(secondary, "Secondary".to_string(), original.clone(), false, false, false);
  MockWindowsApi::place_window(secondary, 2.into());
  let mut manager = WindowManager {
    configuration_provider,
    placement: Default::default(),
    allow_moving_cursor_after_close_or_minimise: true,
    scrolling: Default::default(),
    workspace_manager,
    virtual_desktop_manager: None,
    windows_api: MockWindowsApi,
  };

  manager.reconcile_layouts();

  assert_eq!(
    manager.windows_api.get_window_placement(1.into()).unwrap(),
    WindowPlacement::new_from_sizing(Sizing::new(20, 20, 1880, 990))
  );
  assert_eq!(
    manager.windows_api.get_window_placement(secondary).unwrap(),
    WindowPlacement::new_from_sizing(original)
  );
  assert!(manager.scrolling.workspace_containing(secondary).is_none());
}

#[test]
fn spatial_monitor_crossing_is_adopted_by_scrolling_reconciliation() {
  MockWindowsApi::reset();
  let directory = create_temp_directory();
  let workspace_manager = WorkspaceManager::new_test(true, directory.path().join("workspaces.toml"));
  let configuration_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
  configuration_provider
    .lock()
    .unwrap()
    .set_monitor_layout("DISPLAY2", Layout::Scrolling);
  let handle = WindowHandle::new(1);
  MockWindowsApi::add_or_update_window(
    handle,
    "Primary".to_string(),
    Sizing::left_half_of_screen(Rect::new(0, 0, 1920, 1030), 20),
    false,
    false,
    true,
  );
  MockWindowsApi::place_window(handle, 1.into());
  let mut manager = WindowManager {
    configuration_provider,
    placement: Default::default(),
    allow_moving_cursor_after_close_or_minimise: true,
    scrolling: Default::default(),
    workspace_manager,
    virtual_desktop_manager: None,
    windows_api: MockWindowsApi,
  };

  manager.move_window(Direction::Left);

  assert!(manager.scrolling.workspace_containing(handle).is_none());

  MockWindowsApi::assign_window_to_monitor(handle, 2.into());
  manager.reconcile_layouts();

  assert!(manager.scrolling.workspace_containing(handle).is_some());
}

#[test]
fn moving_from_scrolling_to_spatial_removes_strip_membership() {
  let (mut manager, _directory) = scrolling_manager();
  manager
    .configuration_provider
    .lock()
    .unwrap()
    .set_monitor_layout("DISPLAY2", Layout::Spatial);
  manager.reconcile_layouts();
  let primary_workspace = PersistentWorkspaceId::from(*crate::workspace_manager::tests::primary_active_ws_id());
  let secondary_workspace = manager
    .workspace_manager
    .active_workspace_ids()
    .into_iter()
    .find(|workspace| workspace.monitor_id != primary_workspace.monitor_id)
    .unwrap();

  manager.move_window_to_workspace(secondary_workspace);

  assert!(manager.scrolling.workspace_containing(1.into()).is_none());
}

#[test]
fn moving_from_spatial_to_scrolling_inserts_strip_membership() {
  MockWindowsApi::reset();
  let directory = create_temp_directory();
  let workspace_manager = WorkspaceManager::new_test(false, directory.path().join("workspaces.toml"));
  let configuration_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
  configuration_provider
    .lock()
    .unwrap()
    .set_monitor_layout("DISPLAY1", Layout::Scrolling);
  let secondary = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    secondary,
    "Secondary".to_string(),
    Sizing::new(-700, 50, 400, 300),
    false,
    false,
    true,
  );
  MockWindowsApi::place_window(secondary, 2.into());
  let primary_workspace = *crate::workspace_manager::tests::primary_active_ws_id();
  let mut manager = WindowManager {
    configuration_provider,
    placement: Default::default(),
    allow_moving_cursor_after_close_or_minimise: true,
    scrolling: Default::default(),
    workspace_manager,
    virtual_desktop_manager: None,
    windows_api: MockWindowsApi,
  };
  manager.reconcile_layouts();

  manager.move_window_to_workspace(primary_workspace.into());

  assert_eq!(
    manager.scrolling.workspace_containing(secondary),
    Some(primary_workspace.into())
  );
}
