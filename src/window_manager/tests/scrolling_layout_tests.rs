use crate::api::{MockWindowsApi, WindowsApi};
use crate::common::{Direction, Point, Rect, Sizing, WindowHandle, WindowPlacement};
use crate::window_manager::WindowManager;
use crate::window_manager::tests::test_support::scrolling_manager;

#[test]
fn disabled_scrolling_layout_does_not_reposition_windows() {
  MockWindowsApi::reset();
  let handle = WindowHandle::new(1);
  let sizing = Sizing::new(5, 10, 100, 80);
  MockWindowsApi::add_or_update_window(handle, "Window".to_string(), sizing.clone(), false, false, true);
  MockWindowsApi::add_monitor(1.into(), Rect::new(0, 0, 200, 200), true);
  MockWindowsApi::place_window(handle, 1.into());
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.reconcile_layouts();

  assert_eq!(
    manager.windows_api.get_window_placement(handle).unwrap(),
    WindowPlacement::new_from_sizing(sizing)
  );
}

#[test]
fn scrolling_reconciliation_ignores_non_manageable_windows() {
  let (mut manager, _directory) = scrolling_manager();
  let tray_menu = WindowHandle::new(2);
  let original = Sizing::new(300, 300, 200, 100);
  MockWindowsApi::add_or_update_window(tray_menu, "Tray menu".to_string(), original.clone(), false, false, false);
  MockWindowsApi::place_window(tray_menu, 1.into());
  MockWindowsApi::mark_window_unmanageable(tray_menu);

  manager.reconcile_layouts();

  assert_eq!(
    manager.windows_api.get_window_placement(tray_menu).unwrap(),
    WindowPlacement::new_from_sizing(original)
  );
}

#[test]
fn scrolling_reconciliation_does_not_steal_focus_or_mouse_from_unmanaged_popup() {
  let (mut manager, _directory) = scrolling_manager();
  manager.reconcile_layouts();
  let tray_popup = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    tray_popup,
    "Tray popup".to_string(),
    Sizing::new(1500, 900, 200, 100),
    false,
    false,
    true,
  );
  MockWindowsApi::place_window(tray_popup, 1.into());
  MockWindowsApi::mark_window_unmanageable(tray_popup);
  let click_position = Point::new(1800, 1000);
  MockWindowsApi::set_cursor_position(click_position);
  MockWindowsApi::clear_position_batches();

  manager.reconcile_layouts();

  assert!(MockWindowsApi::position_batches().is_empty());
  assert_eq!(manager.windows_api.get_foreground_window(), Some(tray_popup));
  assert_eq!(manager.windows_api.get_cursor_position(), click_position);
}

#[test]
fn off_screen_member_keeps_its_stored_workspace_when_windows_reports_another_monitor() {
  let (mut manager, _directory) = scrolling_manager();
  let second = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    second,
    "Second".to_string(),
    Sizing::new(500, 50, 100, 100),
    false,
    false,
    false,
  );
  MockWindowsApi::place_window(second, 1.into());
  manager.reconcile_layouts();
  MockWindowsApi::assign_window_to_monitor(second, 2.into());

  manager.reconcile_layouts();

  assert_eq!(
    manager.windows_api.get_window_placement(second).unwrap().normal_position.left,
    1215
  );
}

#[test]
fn unchanged_scrolling_reconciliation_does_not_reposition_windows() {
  let (mut manager, _directory) = scrolling_manager();
  manager.reconcile_layouts();
  MockWindowsApi::clear_position_batches();

  manager.reconcile_layouts();

  assert!(MockWindowsApi::position_batches().is_empty());
}

#[test]
fn scrolling_reconciliation_does_not_reset_mouse_position() {
  let (mut manager, _directory) = scrolling_manager();
  manager.reconcile_layouts();
  let mouse_position = Point::new(300, 400);
  MockWindowsApi::set_cursor_position(mouse_position);

  manager.reconcile_layouts();

  assert_eq!(manager.windows_api.get_cursor_position(), mouse_position);
}

#[test]
fn scrolling_layout_skips_unpositionable_members_without_blocking_or_retrying_the_strip() {
  let (mut manager, _directory) = scrolling_manager();
  let blocked = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    blocked,
    "Elevated".to_string(),
    Sizing::new(500, 50, 100, 100),
    false,
    false,
    false,
  );
  MockWindowsApi::place_window(blocked, 1.into());
  MockWindowsApi::fail_deferred_positioning(blocked);

  manager.reconcile_layouts();
  manager.reconcile_layouts();

  assert_eq!(MockWindowsApi::deferred_positioning_attempts(blocked), 1);
  assert_eq!(
    manager.windows_api.get_window_placement(1.into()).unwrap(),
    WindowPlacement::new_from_sizing(Sizing::new(725, 20, 470, 990))
  );
}

#[test]
fn scrolling_layout_adopts_and_places_existing_windows() {
  let (mut manager, _directory) = scrolling_manager();
  let second = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    second,
    "Second".to_string(),
    Sizing::new(500, 50, 100, 100),
    false,
    false,
    false,
  );
  MockWindowsApi::place_window(second, 1.into());

  manager.reconcile_layouts();

  assert_eq!(
    manager.windows_api.get_window_placement(1.into()).unwrap(),
    WindowPlacement::new_from_sizing(Sizing::new(725, 20, 470, 990))
  );
  assert_eq!(
    manager.windows_api.get_window_placement(second).unwrap(),
    WindowPlacement::new_from_sizing(Sizing::new(1215, 20, 470, 990))
  );
}

#[test]
fn scrolling_layout_inserts_new_foreground_before_previous_foreground() {
  let (mut manager, _directory) = scrolling_manager();
  let second = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    second,
    "Second".to_string(),
    Sizing::new(500, 50, 100, 100),
    false,
    false,
    false,
  );
  MockWindowsApi::place_window(second, 1.into());
  manager.reconcile_layouts();
  let new_window = WindowHandle::new(3);
  MockWindowsApi::add_or_update_window(
    new_window,
    "New".to_string(),
    Sizing::new(300, 50, 100, 100),
    false,
    false,
    true,
  );
  MockWindowsApi::place_window(new_window, 1.into());

  manager.reconcile_layouts();

  assert_eq!(manager.windows_api.get_foreground_window(), Some(new_window));
  assert_eq!(
    manager
      .windows_api
      .get_window_placement(new_window)
      .unwrap()
      .normal_position
      .left,
    725
  );
  assert_eq!(
    manager
      .windows_api
      .get_window_placement(1.into())
      .unwrap()
      .normal_position
      .left,
    1215
  );
  assert_eq!(
    manager.windows_api.get_window_placement(second).unwrap().normal_position.left,
    1705
  );
}

#[test]
fn scrolling_layout_focuses_new_window_even_before_it_becomes_foreground() {
  let (mut manager, _directory) = scrolling_manager();
  manager.reconcile_layouts();
  let new_window = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    new_window,
    "New".to_string(),
    Sizing::new(300, 50, 100, 100),
    false,
    false,
    false,
  );
  MockWindowsApi::place_window(new_window, 1.into());

  manager.reconcile_layouts();

  assert_eq!(manager.windows_api.get_foreground_window(), Some(new_window));
  assert_eq!(
    manager
      .windows_api
      .get_window_placement(new_window)
      .unwrap()
      .normal_position
      .left,
    725
  );
}

#[test]
fn scrolling_focus_scrolls_strip_and_scrolling_move_reorders_it() {
  let (mut manager, _directory) = scrolling_manager();
  let second = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    second,
    "Second".to_string(),
    Sizing::new(500, 50, 100, 100),
    false,
    false,
    false,
  );
  MockWindowsApi::place_window(second, 1.into());
  manager.reconcile_layouts();

  manager.move_cursor(Direction::Right);
  assert_eq!(manager.windows_api.get_foreground_window(), Some(second));
  assert_eq!(
    manager.windows_api.get_window_placement(second).unwrap().normal_position.left,
    725
  );

  manager.move_window(Direction::Left);
  assert_eq!(
    manager.windows_api.get_window_placement(second).unwrap().normal_position.left,
    725
  );
  assert_eq!(
    manager
      .windows_api
      .get_window_placement(1.into())
      .unwrap()
      .normal_position
      .left,
    1215
  );
}

#[test]
fn scrolling_focus_animates_batched_intermediate_positions() {
  let (mut manager, _directory) = scrolling_manager();
  let second = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    second,
    "Second".to_string(),
    Sizing::new(500, 50, 100, 100),
    false,
    false,
    false,
  );
  MockWindowsApi::place_window(second, 1.into());
  manager.reconcile_layouts();
  MockWindowsApi::clear_position_batches();

  manager.move_cursor(Direction::Right);

  let batches = MockWindowsApi::position_batches();
  assert!(batches.len() > 2);
  let first_incoming = batches[0]
    .iter()
    .find(|(handle, _)| *handle == second)
    .map(|(_, rect)| *rect)
    .unwrap();
  assert!(first_incoming.left < 1215 && first_incoming.left > 725);
  assert_eq!(batches.last().unwrap()[0].0, second, "Focused window should be topmost");
  assert_eq!(
    manager.windows_api.get_window_placement(second).unwrap().normal_position.left,
    725
  );
}

#[test]
fn scrolling_layout_disables_vertical_move_and_resize() {
  let (mut manager, _directory) = scrolling_manager();
  manager.reconcile_layouts();
  let initial = manager.windows_api.get_window_placement(1.into()).unwrap();

  manager.move_window(Direction::Up);
  manager.resize_spatial_window(Direction::Left);

  assert_eq!(manager.windows_api.get_window_placement(1.into()).unwrap(), initial);
}

#[test]
fn scrolling_workspace_switch_preserves_off_screen_strip_members() {
  let (mut manager, _directory) = scrolling_manager();
  let second = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    second,
    "Second".to_string(),
    Sizing::new(500, 50, 100, 100),
    false,
    false,
    false,
  );
  MockWindowsApi::place_window(second, 1.into());
  manager.reconcile_layouts();
  let first_workspace = crate::common::PersistentWorkspaceId::from(*crate::workspace_manager::tests::primary_active_ws_id());
  let second_workspace = manager
    .workspace_manager
    .workspaces
    .keys()
    .find(|id| id.monitor_id == first_workspace.monitor_id && id.workspace == 2)
    .copied()
    .unwrap();

  manager.switch_workspace(second_workspace);
  assert!(manager.windows_api.get_all_visible_windows().is_empty());
  manager.switch_workspace(first_workspace);

  assert_eq!(manager.windows_api.get_all_visible_windows().len(), 2);
  assert_eq!(
    manager
      .windows_api
      .get_window_placement(1.into())
      .unwrap()
      .normal_position
      .left,
    725
  );
  assert_eq!(
    manager.windows_api.get_window_placement(second).unwrap().normal_position.left,
    1215
  );
}

#[test]
fn restoring_scrolling_layout_moves_off_screen_members_onto_their_monitor() {
  let (mut manager, _directory) = scrolling_manager();
  let second = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    second,
    "Second".to_string(),
    Sizing::new(500, 50, 100, 100),
    false,
    false,
    false,
  );
  MockWindowsApi::place_window(second, 1.into());
  manager.reconcile_layouts();
  let first_workspace = crate::common::PersistentWorkspaceId::from(*crate::workspace_manager::tests::primary_active_ws_id());
  let empty_workspace = manager
    .workspace_manager
    .workspaces
    .keys()
    .find(|id| id.monitor_id == first_workspace.monitor_id && id.workspace == 2)
    .copied()
    .unwrap();
  manager.switch_workspace(empty_workspace);

  manager.restore_all_managed_windows();

  let visible_windows = manager.windows_api.get_all_visible_windows();
  assert_eq!(visible_windows.len(), 2);
  assert!(visible_windows.iter().all(|window| {
    window
      .rect
      .intersects(&crate::workspace_manager::tests::primary_monitor().monitor_area)
  }));
}

#[test]
fn scrolling_move_to_workspace_updates_both_strips() {
  let (mut manager, _directory) = scrolling_manager();
  let second = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    second,
    "Second".to_string(),
    Sizing::new(500, 50, 100, 100),
    false,
    false,
    false,
  );
  MockWindowsApi::place_window(second, 1.into());
  manager.reconcile_layouts();
  let first_workspace = crate::common::PersistentWorkspaceId::from(*crate::workspace_manager::tests::primary_active_ws_id());
  let second_workspace = manager
    .workspace_manager
    .workspaces
    .keys()
    .find(|id| id.monitor_id == first_workspace.monitor_id && id.workspace == 2)
    .copied()
    .unwrap();

  manager.move_window_to_workspace(second_workspace);
  assert_eq!(manager.windows_api.get_foreground_window(), Some(second));
  manager.switch_workspace(second_workspace);

  assert_eq!(manager.windows_api.get_foreground_window(), Some(1.into()));
  assert_eq!(
    manager
      .windows_api
      .get_window_placement(1.into())
      .unwrap()
      .normal_position
      .left,
    725
  );
}

#[test]
fn scrolling_reconciliation_removes_externally_closed_member() {
  let (mut manager, _directory) = scrolling_manager();
  let second = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    second,
    "Second".to_string(),
    Sizing::new(500, 50, 100, 100),
    false,
    false,
    false,
  );
  MockWindowsApi::place_window(second, 1.into());
  manager.reconcile_layouts();
  manager.windows_api.do_close_window(1.into());

  manager.reconcile_layouts();

  assert_eq!(manager.windows_api.get_foreground_window(), Some(second));
  assert_eq!(
    manager.windows_api.get_window_placement(second).unwrap().normal_position.left,
    725
  );
}

#[test]
fn scrolling_layout_removes_closed_member_and_focuses_neighbour() {
  let (mut manager, _directory) = scrolling_manager();
  let second = WindowHandle::new(2);
  MockWindowsApi::add_or_update_window(
    second,
    "Second".to_string(),
    Sizing::new(500, 50, 100, 100),
    false,
    false,
    false,
  );
  MockWindowsApi::place_window(second, 1.into());
  manager.reconcile_layouts();

  manager.close_window();

  assert_eq!(manager.windows_api.get_foreground_window(), Some(second));
  assert_eq!(
    manager.windows_api.get_window_placement(second).unwrap().normal_position.left,
    725
  );
}

#[test]
fn scrolling_layout_adopts_nearest_width_preset_and_centres_focus() {
  let (mut manager, _directory) = scrolling_manager();
  MockWindowsApi::add_or_update_window(
    1.into(),
    "Test Window".to_string(),
    Sizing::new(50, 50, 1000, 400),
    false,
    false,
    true,
  );

  manager.reconcile_layouts();

  assert_eq!(
    manager.windows_api.get_window_placement(1.into()).unwrap(),
    WindowPlacement::new_from_sizing(Sizing::new(490, 20, 940, 990))
  );
  assert_eq!(manager.windows_api.get_cursor_position(), Point::new(50, 50));
}

#[test]
fn scrolling_keyboard_resize_traverses_presets_and_stops_at_boundary() {
  let (mut manager, _directory) = scrolling_manager();
  manager.reconcile_layouts();
  MockWindowsApi::clear_position_batches();

  manager.resize_scrolling_window(Direction::Left);
  assert!(MockWindowsApi::position_batches().is_empty());

  manager.resize_scrolling_window(Direction::Right);
  assert_eq!(
    manager.windows_api.get_window_placement(1.into()).unwrap(),
    WindowPlacement::new_from_sizing(Sizing::new(647, 20, 626, 990))
  );
  assert_eq!(manager.windows_api.get_cursor_position(), Point::new(960, 515));
}

#[test]
fn completed_mouse_resize_snaps_height_and_reflows_even_when_preset_is_unchanged() {
  let (mut manager, _directory) = scrolling_manager();
  manager.reconcile_layouts();
  manager
    .windows_api
    .set_window_position(1.into(), Rect::new(100, 100, 560, 500));
  MockWindowsApi::clear_position_batches();

  manager.finish_mouse_resize(1.into());

  assert!(!MockWindowsApi::position_batches().is_empty());
  assert_eq!(
    manager.windows_api.get_window_placement(1.into()).unwrap(),
    WindowPlacement::new_from_sizing(Sizing::new(725, 20, 470, 990))
  );
}

#[test]
fn completed_mouse_resize_selects_nearest_new_preset() {
  let (mut manager, _directory) = scrolling_manager();
  manager.reconcile_layouts();
  manager
    .windows_api
    .set_window_position(1.into(), Rect::new(100, 100, 1_000, 500));

  manager.finish_mouse_resize(1.into());

  assert_eq!(
    manager.windows_api.get_window_placement(1.into()).unwrap(),
    WindowPlacement::new_from_sizing(Sizing::new(490, 20, 940, 990))
  );
}

#[test]
fn scrolling_preset_follows_window_to_different_sized_scrolling_monitor() {
  let (mut manager, _directory) = scrolling_manager();
  manager.reconcile_layouts();
  manager.resize_scrolling_window(Direction::Right);
  let primary_workspace =
    crate::common::PersistentWorkspaceId::from(*crate::workspace_manager::tests::primary_active_ws_id());
  let secondary_workspace = manager
    .workspace_manager
    .active_workspace_ids()
    .into_iter()
    .find(|workspace| workspace.monitor_id != primary_workspace.monitor_id)
    .unwrap();

  manager.move_window_to_workspace(secondary_workspace);

  assert_eq!(
    manager.windows_api.get_window_placement(1.into()).unwrap(),
    WindowPlacement::new_from_sizing(Sizing::new(-527, 20, 253, 510))
  );
}
