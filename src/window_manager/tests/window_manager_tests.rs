use crate::api::{MockWindowsApi, WindowsApi};
use crate::common::{
  Direction, Monitor, MonitorHandle, PersistentWorkspaceId, Point, Rect, Sizing, WindowHandle, WindowPlacement, Workspace,
};
use crate::configuration_provider::{ConfigurationProvider, Layout};
use crate::utils::create_temp_directory;
use crate::window_manager::WindowManager;
use crate::window_manager::tests::test_support::scrolling_manager;
use crate::workspace_manager::WorkspaceManager;
use std::sync::{Arc, Mutex};

fn vertical_mixed_layout_manager(direction: Direction, target_layout: Layout) -> (WindowManager<MockWindowsApi>, Monitor) {
  vertical_mixed_layout_manager_with_widths(direction, target_layout, 1_000, 1_000)
}

fn vertical_mixed_layout_manager_with_widths(
  direction: Direction,
  target_layout: Layout,
  source_width: i32,
  target_width: i32,
) -> (WindowManager<MockWindowsApi>, Monitor) {
  MockWindowsApi::reset();
  let (source_area, target_area) = match direction {
    Direction::Up => (Rect::new(0, 1_000, source_width, 2_000), Rect::new(0, 0, target_width, 1_000)),
    Direction::Down => (Rect::new(0, 0, source_width, 1_000), Rect::new(0, 1_000, target_width, 2_000)),
    Direction::Left | Direction::Right => panic!("vertical fixture requires Up or Down"),
  };
  let mut source_monitor = Monitor::new_test(1, source_area);
  source_monitor.is_primary = true;
  let target_monitor = Monitor::new_test(2, target_area);
  for monitor in [&source_monitor, &target_monitor] {
    MockWindowsApi::add_monitor_with_full_details(
      monitor.id,
      monitor.handle,
      monitor.monitor_area,
      monitor.work_area,
      monitor.is_primary,
    );
  }
  let source_workspace_id = PersistentWorkspaceId::new(source_monitor.id, 1, true);
  let target_workspace_id = PersistentWorkspaceId::new(target_monitor.id, 1, false);
  let source_workspace = Workspace::new_active(source_workspace_id, &source_monitor, 20);
  let target_workspace = Workspace::new_active(target_workspace_id, &target_monitor, 20);
  let workspace_manager = WorkspaceManager::from_workspaces(&[&source_workspace, &target_workspace], 20);
  let handle = WindowHandle::new(1);
  MockWindowsApi::add_or_update_window(
    handle,
    "Source".to_string(),
    Sizing::new(source_area.left + 100, source_area.top + 100, 400, 700),
    false,
    false,
    true,
  );
  MockWindowsApi::place_window(handle, source_monitor.handle);
  MockWindowsApi::set_cursor_position(source_monitor.center);
  let configuration_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
  configuration_provider.lock().unwrap().set_default_layout(Layout::Scrolling);
  configuration_provider
    .lock()
    .unwrap()
    .set_monitor_layout(&target_monitor.id_to_string(), target_layout);
  let mut manager = WindowManager {
    configuration_provider,
    placement: Default::default(),
    allow_moving_cursor_after_close_or_minimise: true,
    scrolling: Default::default(),
    spatial: Default::default(),
    workspace_manager,
    virtual_desktop_manager: None,
    windows_api: MockWindowsApi,
  };
  manager.reconcile_layouts();
  (manager, target_monitor)
}

#[test]
fn resize_scrolling_window_command_no_ops_for_spatial_layout() {
  MockWindowsApi::reset();
  let handle = WindowHandle::new(1);
  let sizing = Sizing::new(50, 50, 400, 300);
  MockWindowsApi::add_or_update_window(handle, "Spatial".to_string(), sizing.clone(), false, false, true);
  MockWindowsApi::add_monitor(1.into(), Rect::new(0, 0, 1920, 1080), true);
  MockWindowsApi::place_window(handle, 1.into());
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_scrolling_window(Direction::Right);
  manager.finish_mouse_resize(handle);

  assert_eq!(
    manager.windows_api.get_window_placement(handle).unwrap(),
    WindowPlacement::new_from_sizing(sizing)
  );
}

#[test]
fn move_window_with_mixed_layout_routes_move_by_foreground_monitor() {
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
    spatial: Default::default(),
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
fn move_window_with_scrolling_window_can_move_down_to_spatial_monitor() {
  let (mut manager, target_monitor) = vertical_mixed_layout_manager(Direction::Down, Layout::Spatial);
  let handle = WindowHandle::new(1);

  manager.move_window(Direction::Down);

  assert!(manager.scrolling.get_workspace_containing(handle).is_none());
  assert_eq!(
    manager.windows_api.get_window_placement(handle).unwrap(),
    WindowPlacement::new_from_sizing(Sizing::near_maximised(target_monitor.work_area, 20))
  );
  assert_eq!(manager.windows_api.get_foreground_window(), Some(handle));
  assert_eq!(manager.windows_api.get_cursor_position(), target_monitor.center);
}

#[test]
fn move_window_with_scrolling_window_can_move_up_to_spatial_monitor() {
  let (mut manager, target_monitor) = vertical_mixed_layout_manager(Direction::Up, Layout::Spatial);
  let handle = WindowHandle::new(1);

  manager.move_window(Direction::Up);

  assert!(manager.scrolling.get_workspace_containing(handle).is_none());
  assert_eq!(
    manager.windows_api.get_window_placement(handle).unwrap(),
    WindowPlacement::new_from_sizing(Sizing::near_maximised(target_monitor.work_area, 20))
  );
  assert_eq!(manager.windows_api.get_foreground_window(), Some(handle));
  assert_eq!(manager.windows_api.get_cursor_position(), target_monitor.center);
}

#[test]
fn move_window_with_scrolling_source_reflows_without_stealing_focus_after_spatial_transfer() {
  let (mut manager, target_monitor) = vertical_mixed_layout_manager(Direction::Down, Layout::Spatial);
  let moved_handle = WindowHandle::new(1);
  let source_workspace = manager.scrolling.get_workspace_containing(moved_handle).unwrap();
  let source_monitor = manager.workspace_manager.monitor_for_workspace(source_workspace).unwrap();
  let remaining_handle = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    remaining_handle,
    "Remaining".to_string(),
    Sizing::new(600, 100, 300, 700),
    false,
    false,
    false,
  );
  MockWindowsApi::place_window(remaining_handle, source_monitor.handle);
  manager.reconcile_layouts();
  MockWindowsApi::set_foreground_window(moved_handle);
  MockWindowsApi::set_cursor_position(source_monitor.center);

  manager.move_window(Direction::Down);

  let remaining_placement = manager.windows_api.get_window_placement(remaining_handle).unwrap();
  assert_eq!(
    Point::from_center_of_rect(&remaining_placement.normal_position),
    source_monitor.center
  );
  assert_eq!(
    manager.scrolling.get_workspace_containing(remaining_handle),
    Some(source_workspace)
  );
  assert_eq!(manager.windows_api.get_foreground_window(), Some(moved_handle));
  assert_eq!(manager.windows_api.get_cursor_position(), target_monitor.center);
}

#[test]
fn move_window_with_scrolling_window_can_move_down_to_scrolling_monitor() {
  let (mut manager, target_monitor) = vertical_mixed_layout_manager(Direction::Down, Layout::Scrolling);
  let handle = WindowHandle::new(1);
  let source_workspace = manager.scrolling.get_workspace_containing(handle).unwrap();
  let source_width = manager
    .windows_api
    .get_window_placement(handle)
    .unwrap()
    .normal_position
    .width();

  manager.move_window(Direction::Down);

  let target_workspace = manager.scrolling.get_workspace_containing(handle).unwrap();
  let target_placement = manager.windows_api.get_window_placement(handle).unwrap();
  assert_ne!(target_workspace, source_workspace);
  assert_eq!(manager.scrolling.get_members(target_workspace), vec![handle]);
  assert_eq!(target_placement.normal_position.width(), source_width);
  assert_eq!(target_placement.normal_position.top, target_monitor.work_area.top + 20);
  assert_eq!(target_placement.normal_position.bottom, target_monitor.work_area.bottom - 20);
  assert_eq!(manager.windows_api.get_foreground_window(), Some(handle));
  assert_eq!(manager.windows_api.get_cursor_position(), target_monitor.center);
}

#[test]
fn move_window_between_scrolling_monitors_retains_width_preset() {
  let (mut manager, target_monitor) =
    vertical_mixed_layout_manager_with_widths(Direction::Down, Layout::Scrolling, 1_000, 2_000);
  let handle = WindowHandle::new(1);
  assert_eq!(
    manager
      .windows_api
      .get_window_placement(handle)
      .unwrap()
      .normal_position
      .width(),
    320
  );

  manager.move_window(Direction::Down);

  let target_width = target_monitor.work_area.width() - 40;
  assert_eq!(
    manager
      .windows_api
      .get_window_placement(handle)
      .unwrap()
      .normal_position
      .width(),
    target_width / 3
  );
}

#[test]
fn move_window_between_scrolling_monitors_reflows_source_without_stealing_focus() {
  let (mut manager, target_monitor) = vertical_mixed_layout_manager(Direction::Down, Layout::Scrolling);
  let moved_handle = WindowHandle::new(1);
  let source_workspace = manager.scrolling.get_workspace_containing(moved_handle).unwrap();
  let source_monitor = manager.workspace_manager.monitor_for_workspace(source_workspace).unwrap();
  let remaining_handle = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    remaining_handle,
    "Remaining".to_string(),
    Sizing::new(600, 100, 300, 700),
    false,
    false,
    false,
  );
  MockWindowsApi::place_window(remaining_handle, source_monitor.handle);
  manager.reconcile_layouts();
  MockWindowsApi::set_foreground_window(moved_handle);
  MockWindowsApi::set_cursor_position(source_monitor.center);

  manager.move_window(Direction::Down);

  let remaining_placement = manager.windows_api.get_window_placement(remaining_handle).unwrap();
  assert_eq!(
    Point::from_center_of_rect(&remaining_placement.normal_position),
    source_monitor.center
  );
  assert_eq!(
    manager.scrolling.get_workspace_containing(remaining_handle),
    Some(source_workspace)
  );
  assert_eq!(manager.windows_api.get_foreground_window(), Some(moved_handle));
  assert_eq!(manager.windows_api.get_cursor_position(), target_monitor.center);
}

#[test]
fn move_window_to_scrolling_monitor_appends_to_target_strip() {
  let (mut manager, target_monitor) = vertical_mixed_layout_manager(Direction::Down, Layout::Scrolling);
  let moved_handle = WindowHandle::new(1);
  for (handle, left) in [(WindowHandle::new(2), 100), (WindowHandle::new(3), 600)] {
    MockWindowsApi::add_or_update_window(
      handle,
      format!("Target {}", handle.hwnd),
      Sizing::new(left, target_monitor.work_area.top + 100, 300, 700),
      false,
      false,
      false,
    );
    MockWindowsApi::place_window(handle, target_monitor.handle);
  }
  manager.reconcile_layouts();
  MockWindowsApi::set_foreground_window(moved_handle);

  manager.move_window(Direction::Down);

  let target_workspace = manager.scrolling.get_workspace_containing(moved_handle).unwrap();
  assert_eq!(
    manager.scrolling.get_members(target_workspace),
    vec![2.into(), 3.into(), moved_handle]
  );
}

#[test]
fn move_window_between_scrolling_monitors_does_not_restore_former_strip_position() {
  let (mut manager, _target_monitor) = vertical_mixed_layout_manager(Direction::Down, Layout::Scrolling);
  let stationary_handle = WindowHandle::new(1);
  let source_workspace = manager.scrolling.get_workspace_containing(stationary_handle).unwrap();
  let source_monitor = manager.workspace_manager.monitor_for_workspace(source_workspace).unwrap();
  let moved_handle = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    moved_handle,
    "Moved".to_string(),
    Sizing::new(100, 100, 300, 700),
    false,
    false,
    true,
  );
  MockWindowsApi::place_window(moved_handle, source_monitor.handle);
  manager.reconcile_layouts();
  assert_eq!(
    manager.scrolling.get_members(source_workspace),
    vec![moved_handle, stationary_handle]
  );

  manager.move_window(Direction::Down);
  manager.move_window(Direction::Up);

  assert_eq!(
    manager.scrolling.get_members(source_workspace),
    vec![stationary_handle, moved_handle] // Window is appended instead of restored
  );
}

#[test]
fn move_window_with_scrolling_window_can_move_up_to_scrolling_monitor() {
  let (mut manager, target_monitor) = vertical_mixed_layout_manager(Direction::Up, Layout::Scrolling);
  let handle = WindowHandle::new(1);

  manager.move_window(Direction::Up);

  let target_workspace = manager.scrolling.get_workspace_containing(handle).unwrap();
  assert_eq!(target_workspace.monitor_id, target_monitor.id);
  assert_eq!(manager.windows_api.get_foreground_window(), Some(handle));
  assert_eq!(manager.windows_api.get_cursor_position(), target_monitor.center);
}

#[test]
fn move_window_with_spatial_monitor_crossing_is_adopted_by_scrolling_reconciliation() {
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
    spatial: Default::default(),
    workspace_manager,
    virtual_desktop_manager: None,
    windows_api: MockWindowsApi,
  };

  manager.move_window(Direction::Left);

  assert!(manager.scrolling.get_workspace_containing(handle).is_none());

  MockWindowsApi::assign_window_to_monitor(handle, 2.into());
  manager.reconcile_layouts();

  assert!(manager.scrolling.get_workspace_containing(handle).is_some());
}

#[test]
fn move_window_with_scrolling_horizontal_move_does_not_enter_spatial_monitor() {
  let (mut manager, _directory) = scrolling_manager();
  manager
    .configuration_provider
    .lock()
    .unwrap()
    .set_monitor_layout("DISPLAY2", Layout::Spatial);
  manager.reconcile_layouts();
  let handle = WindowHandle::new(1);
  let source_workspace = manager.scrolling.get_workspace_containing(handle).unwrap();
  let before = manager.windows_api.get_window_placement(handle).unwrap();

  manager.move_window(Direction::Left);

  assert_eq!(manager.windows_api.get_window_placement(handle).unwrap(), before);
  assert_eq!(manager.scrolling.get_workspace_containing(handle), Some(source_workspace));
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
fn reconcile_layouts_only_manages_scrolling_monitors() {
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
    spatial: Default::default(),
    workspace_manager,
    virtual_desktop_manager: None,
    windows_api: MockWindowsApi,
  };

  manager.reconcile_layouts();

  assert_eq!(
    manager.windows_api.get_window_placement(1.into()).unwrap(),
    WindowPlacement::new_from_sizing(Sizing::new(725, 20, 470, 990))
  );
  assert_eq!(
    manager.windows_api.get_window_placement(secondary).unwrap(),
    WindowPlacement::new_from_sizing(original)
  );
  assert!(manager.scrolling.get_workspace_containing(secondary).is_none());
}

#[test]
fn reconcile_layouts_when_changing_default_from_spatial_to_scrolling_adopts_active_windows() {
  MockWindowsApi::reset();
  let directory = create_temp_directory();
  let workspace_manager = WorkspaceManager::new_test(true, directory.path().join("workspaces.toml"));
  let configuration_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
  let mut manager = WindowManager {
    configuration_provider: configuration_provider.clone(),
    placement: Default::default(),
    allow_moving_cursor_after_close_or_minimise: true,
    scrolling: Default::default(),
    spatial: Default::default(),
    workspace_manager,
    virtual_desktop_manager: None,
    windows_api: MockWindowsApi,
  };
  assert!(manager.scrolling.get_workspace_containing(1.into()).is_none());

  configuration_provider.lock().unwrap().set_default_layout(Layout::Scrolling);
  manager.reconcile_layouts();

  assert!(manager.scrolling.get_workspace_containing(1.into()).is_some());
}

#[test]
fn reconcile_layouts_when_changing_default_from_scrolling_to_spatial_restores_and_releases_active_windows() {
  let (mut manager, _directory) = scrolling_manager();
  let second = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    second,
    "Second".to_string(),
    Sizing::new(200, 50, 100, 100),
    false,
    false,
    false,
  );
  MockWindowsApi::place_window(second, 1.into());
  manager.reconcile_layouts();
  assert!(manager.scrolling.get_workspace_containing(second).is_some());

  manager
    .configuration_provider
    .lock()
    .unwrap()
    .set_default_layout(Layout::Spatial);
  manager.reconcile_layouts();

  assert!(manager.scrolling.get_workspace_containing(1.into()).is_none());
  assert!(manager.scrolling.get_workspace_containing(second).is_none());
  assert_eq!(
    manager.windows_api.get_window_placement(second).unwrap(),
    WindowPlacement::new_from_sizing(Sizing::new(1215, 20, 470, 990))
  );
}

#[test]
fn reconcile_layouts_when_changing_default_to_spatial_keeps_overridden_scrolling_monitor_managed() {
  let (mut manager, _directory) = scrolling_manager();
  let secondary = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    secondary,
    "Secondary".to_string(),
    Sizing::new(-700, 50, 100, 100),
    false,
    false,
    false,
  );
  MockWindowsApi::place_window(secondary, 2.into());
  manager
    .configuration_provider
    .lock()
    .unwrap()
    .set_monitor_layout("DISPLAY2", Layout::Scrolling);
  manager.reconcile_layouts();
  assert!(manager.scrolling.get_workspace_containing(secondary).is_some());

  manager
    .configuration_provider
    .lock()
    .unwrap()
    .set_default_layout(Layout::Spatial);
  manager.reconcile_layouts();

  assert!(manager.scrolling.get_workspace_containing(1.into()).is_none());
  assert!(manager.scrolling.get_workspace_containing(secondary).is_some());
}

#[test]
fn move_window_to_workspace_when_moving_from_scrolling_to_spatial_removes_strip_membership() {
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

  assert!(manager.scrolling.get_workspace_containing(1.into()).is_none());
}

#[test]
fn move_window_to_workspace_when_moving_from_spatial_to_scrolling_inserts_strip_membership() {
  MockWindowsApi::reset();
  let directory = create_temp_directory();
  let workspace_manager = WorkspaceManager::new_test(false, directory.path().join("workspaces.toml"));
  let configuration_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
  configuration_provider
    .lock()
    .unwrap()
    .set_monitor_layout("DISPLAY1", Layout::Scrolling);
  let secondary_handle = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    secondary_handle,
    "Secondary".to_string(),
    Sizing::new(-700, 50, 400, 300),
    false,
    false,
    true,
  );
  MockWindowsApi::place_window(secondary_handle, 2.into());
  let primary_workspace = *crate::workspace_manager::tests::primary_active_ws_id();
  let mut manager = WindowManager {
    configuration_provider,
    placement: Default::default(),
    allow_moving_cursor_after_close_or_minimise: true,
    scrolling: Default::default(),
    spatial: Default::default(),
    workspace_manager,
    virtual_desktop_manager: None,
    windows_api: MockWindowsApi,
  };
  manager.reconcile_layouts();

  manager.move_window_to_workspace(primary_workspace.into());

  assert_eq!(
    manager.scrolling.get_workspace_containing(secondary_handle),
    Some(primary_workspace.into())
  );
}
