use crate::api::{MockWindowsApi, WindowsApi};
use crate::common::{Direction, MonitorHandle, Point, Rect, Sizing, WindowHandle, WindowPlacement};
use crate::utils::MINIMUM_WINDOW_DIMENSION;
use crate::window_manager::WindowManager;

#[test]
fn move_window_on_the_same_monitor() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let sizing = Sizing::new(20, 20, 160, 160);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let mock_api = MockWindowsApi;
  let mut manager = WindowManager::default(mock_api);

  manager.move_window(Direction::Right);

  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  let monitor_info = mock_api
    .get_monitor_info_for_monitor(monitor_handle)
    .expect("Failed to get monitor info");
  let expected_sizing = Sizing::right_half_of_screen(monitor_info.work_area, 20);
  let expected_cursor_position = Point::from_center_of_sizing(&expected_sizing);
  let expected_placement = WindowPlacement::new_from_sizing(expected_sizing);
  assert!(actual_placement.is_some());
  assert_eq!(actual_placement.unwrap(), expected_placement);
  assert_eq!(manager.windows_api.get_cursor_position(), expected_cursor_position)
}

#[test]
fn move_window_when_window_is_already_at_target_location() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let sizing = Sizing::left_half_of_screen(Rect::new(0, 0, 200, 180), 20);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing.clone(), false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.move_window(Direction::Left);

  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  let expected_placement = WindowPlacement::new_from_sizing(sizing);
  assert!(actual_placement.is_some());
  assert_eq!(actual_placement.unwrap(), expected_placement);
  assert_eq!(manager.windows_api.get_cursor_position(), Point::default())
}

#[test]
fn move_window_to_another_monitor() {
  let monitor_handle_1 = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let sizing = Sizing::right_half_of_screen(Rect::new(0, 0, 200, 180), 20);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing.clone(), false, false, true);
  MockWindowsApi::add_monitor(monitor_handle_1, Rect::new(0, 0, 200, 200), true);
  MockWindowsApi::add_monitor(2.into(), Rect::new(200, 0, 400, 200), false);
  MockWindowsApi::place_window(window_handle, monitor_handle_1);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.move_window(Direction::Right);

  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  let expected_placement = WindowPlacement::new_from_sizing(Sizing::near_maximised(Rect::new(200, 0, 400, 180), 20));
  assert!(actual_placement.is_some());
  assert_eq!(actual_placement.unwrap(), expected_placement);
  assert_eq!(manager.windows_api.get_cursor_position(), Point::new(300, 100))
}

#[test]
fn resize_spatial_window_steps_three_quarter_left_down_to_left_half_of_screen() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area = Rect::new(0, 0, 2000, 1000);
  let sizing = Sizing::three_quarter_near_maximised(work_area, Direction::Left, 20);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Left);

  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  let expected_sizing = Sizing::left_half_of_screen(work_area, 20);
  let expected_placement = WindowPlacement::new_from_sizing(expected_sizing.clone());
  assert!(actual_placement.is_some());
  assert_eq!(actual_placement.unwrap(), expected_placement);
  assert_eq!(
    manager.windows_api.get_cursor_position(),
    Point::from_center_of_sizing(&expected_sizing)
  );
}

#[test]
fn resize_spatial_window_steps_three_quarter_down_down_to_bottom_half_of_screen() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area = Rect::new(0, 0, 2000, 1000);
  let sizing = Sizing::three_quarter_near_maximised(work_area, Direction::Down, 20);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Down);

  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  let expected_sizing = Sizing::bottom_half_of_screen(work_area, 20);
  let expected_placement = WindowPlacement::new_from_sizing(expected_sizing.clone());
  assert!(actual_placement.is_some());
  assert_eq!(actual_placement.unwrap(), expected_placement);
  assert_eq!(
    manager.windows_api.get_cursor_position(),
    Point::from_center_of_sizing(&expected_sizing)
  );
}

#[test]
fn resize_spatial_window_three_quarter_left_halves_normally_in_down_direction() {
  // A 75%-wide window (from Left resize) should NOT trigger the 3/4 rule when pressing Down
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area = Rect::new(0, 0, 2000, 1000);
  let sizing = Sizing::three_quarter_near_maximised(work_area, Direction::Left, 20);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing.clone(), false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Down);

  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  let expected_sizing = sizing.halved(Direction::Down, 20);
  let expected_placement = WindowPlacement::new_from_sizing(expected_sizing.clone());
  assert!(actual_placement.is_some());
  assert_eq!(actual_placement.unwrap(), expected_placement);
  assert_eq!(
    manager.windows_api.get_cursor_position(),
    Point::from_center_of_sizing(&expected_sizing)
  );
}

#[test]
fn resize_spatial_window_does_nothing_when_quarter_floor_exceeds_pixel_floor_and_result_is_below_it() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area_width = 4000;
  let sizing = Sizing::new(20, 20, MINIMUM_WINDOW_DIMENSION * 3, 600);
  // Halved width is 365px: above the 250px floor but below this monitor's margin-aware quarter step.
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing.clone(), false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, work_area_width, 1020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let initial_cursor = Point::default();
  MockWindowsApi::set_cursor_position(initial_cursor);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Left);

  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  assert!(actual_placement.is_some());
  assert_eq!(
    actual_placement.unwrap(),
    WindowPlacement::new_from_sizing(sizing),
    "Window should not have been resized"
  );
  assert_eq!(
    manager.windows_api.get_cursor_position(),
    initial_cursor,
    "Cursor should not have moved"
  );
}

#[test]
fn resize_spatial_window_allows_resize_above_pixel_floor_when_quarter_step_is_smaller() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  // Margin-aware quarter width is 225px, so the 250px floor applies; the 365px target remains valid.
  let sizing = Sizing::new(20, 20, 750, 560);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing.clone(), false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 1000, 620), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Left);

  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  let expected_sizing = sizing.halved(Direction::Left, 20);
  let expected_placement = WindowPlacement::new_from_sizing(expected_sizing.clone());
  assert!(actual_placement.is_some());
  assert_eq!(
    actual_placement.unwrap(),
    expected_placement,
    "Window should have been resized"
  );
  assert_eq!(
    manager.windows_api.get_cursor_position(),
    Point::from_center_of_sizing(&expected_sizing)
  );
}

#[test]
fn resize_spatial_window_halves_left_half_of_screen_in_left_direction() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area = Rect::new(0, 0, 2000, 1000);
  let left_half = Sizing::left_half_of_screen(work_area, 20);
  MockWindowsApi::add_or_update_window(
    window_handle,
    "Test Window".to_string(),
    left_half.clone(),
    false,
    false,
    true,
  );
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let initial_cursor = Point::default();
  MockWindowsApi::set_cursor_position(initial_cursor);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Left);

  let expected = left_half.halved(Direction::Left, 20);
  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  assert!(actual_placement.is_some());
  assert_eq!(
    actual_placement.unwrap(),
    WindowPlacement::new_from_sizing(expected),
    "Window should have been halved"
  );
  assert_ne!(
    manager.windows_api.get_cursor_position(),
    initial_cursor,
    "Cursor should have moved"
  );
}

#[test]
fn resize_spatial_window_restores_bottom_half_when_application_rejects_quarter_height() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area = Rect::new(0, 0, 2000, 2000);
  let bottom_half = Sizing::bottom_half_of_screen(work_area, 20);
  MockWindowsApi::add_or_update_window(
    window_handle,
    "Test Window".to_string(),
    bottom_half.clone(),
    false,
    false,
    true,
  );
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 2020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  MockWindowsApi::set_window_position_minimum_dimensions(window_handle, 0, 600);
  let initial_cursor = Point::new(40, 50);
  MockWindowsApi::set_cursor_position(initial_cursor);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Down);

  assert_eq!(
    manager.windows_api.get_window_placement(window_handle),
    Some(WindowPlacement::new_from_sizing(bottom_half))
  );
  assert_eq!(manager.windows_api.get_cursor_position(), initial_cursor);
}

#[test]
fn resize_spatial_window_restores_right_half_when_application_rejects_quarter_width() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area = Rect::new(0, 0, 2000, 1000);
  let right_half = Sizing::right_half_of_screen(work_area, 20);
  MockWindowsApi::add_or_update_window(
    window_handle,
    "Test Window".to_string(),
    right_half.clone(),
    false,
    false,
    true,
  );
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  MockWindowsApi::set_window_position_minimum_dimensions(window_handle, 600, 0);
  let initial_cursor = Point::new(40, 50);
  MockWindowsApi::set_cursor_position(initial_cursor);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Right);

  assert_eq!(
    manager.windows_api.get_window_placement(window_handle),
    Some(WindowPlacement::new_from_sizing(right_half))
  );
  assert_eq!(manager.windows_api.get_cursor_position(), initial_cursor);
}

#[test]
fn resize_spatial_window_halves_right_half_of_screen_in_right_direction() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area = Rect::new(0, 0, 2000, 1000);
  let right_half = Sizing::right_half_of_screen(work_area, 20);
  MockWindowsApi::add_or_update_window(
    window_handle,
    "Test Window".to_string(),
    right_half.clone(),
    false,
    false,
    true,
  );
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let initial_cursor = Point::default();
  MockWindowsApi::set_cursor_position(initial_cursor);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Right);

  let expected = right_half.halved(Direction::Right, 20);
  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  assert!(actual_placement.is_some());
  assert_eq!(
    actual_placement.unwrap(),
    WindowPlacement::new_from_sizing(expected),
    "Window should have been halved"
  );
  assert_ne!(
    manager.windows_api.get_cursor_position(),
    initial_cursor,
    "Cursor should have moved"
  );
}

#[test]
fn resize_spatial_window_keeps_half_size_when_quarter_is_below_pixel_floor() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area = Rect::new(0, 0, 800, 1000);
  let right_half = Sizing::right_half_of_screen(work_area, 20);
  MockWindowsApi::add_or_update_window(
    window_handle,
    "Test Window".to_string(),
    right_half.clone(),
    false,
    false,
    true,
  );
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 800, 1020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let initial_cursor = Point::new(40, 50);
  MockWindowsApi::set_cursor_position(initial_cursor);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Right);

  assert_eq!(
    manager.windows_api.get_window_placement(window_handle),
    Some(WindowPlacement::new_from_sizing(right_half))
  );
  assert_eq!(manager.windows_api.get_cursor_position(), initial_cursor);
}

#[test]
fn resize_spatial_window_does_nothing_when_halving_would_fall_below_quarter_step() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let sizing = Sizing::new(20, 20, 800, 960);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing.clone(), false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let initial_cursor = Point::new(40, 50);
  MockWindowsApi::set_cursor_position(initial_cursor);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Left);

  assert_eq!(
    manager.windows_api.get_window_placement(window_handle),
    Some(WindowPlacement::new_from_sizing(sizing))
  );
  assert_eq!(manager.windows_api.get_cursor_position(), initial_cursor);
}

#[test]
fn resize_spatial_window_does_nothing_when_below_minimum_dimension() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let small_sizing = Sizing::new(20, 20, 200, 200);
  MockWindowsApi::add_or_update_window(
    window_handle,
    "Test Window".to_string(),
    small_sizing.clone(),
    false,
    false,
    true,
  );
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1000), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let initial_cursor = Point::default();
  MockWindowsApi::set_cursor_position(initial_cursor);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Left);

  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  let expected_placement = WindowPlacement::new_from_sizing(small_sizing);
  assert!(actual_placement.is_some());
  assert_eq!(
    actual_placement.unwrap(),
    expected_placement,
    "Window should not have been resized"
  );
  assert_eq!(
    manager.windows_api.get_cursor_position(),
    initial_cursor,
    "Cursor should not have moved"
  );
}

#[test]
fn resize_spatial_window_does_nothing_when_no_foreground_window() {
  // No window is added or focused -> get_window_and_monitor_info returns None -> early return.
  let initial_cursor = Point::default();
  MockWindowsApi::set_cursor_position(initial_cursor);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Left);

  assert_eq!(
    manager.windows_api.get_cursor_position(),
    initial_cursor,
    "Cursor should not have moved"
  );
}

#[test]
fn resize_spatial_window_does_nothing_when_result_height_falls_below_constant_minimum() {
  // Down halved height is 140px, below the 250px floor.
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let sizing = Sizing::new(20, 20, 500, 300);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing.clone(), false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1000), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let initial_cursor = Point::default();
  MockWindowsApi::set_cursor_position(initial_cursor);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Down);

  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  assert!(actual_placement.is_some());
  assert_eq!(
    actual_placement.unwrap(),
    WindowPlacement::new_from_sizing(sizing),
    "Window should not have been resized"
  );
  assert_eq!(
    manager.windows_api.get_cursor_position(),
    initial_cursor,
    "Cursor should not have moved"
  );
}

#[test]
fn resize_spatial_window_does_nothing_when_quarter_height_floor_blocks_resize() {
  // Halved height is 365px: above the 250px floor but below this monitor's margin-aware quarter step.
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area_height = 4000;
  let sizing = Sizing::new(20, 20, 500, MINIMUM_WINDOW_DIMENSION * 3);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing.clone(), false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, work_area_height + 20), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let initial_cursor = Point::default();
  MockWindowsApi::set_cursor_position(initial_cursor);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Down);

  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  assert!(actual_placement.is_some());
  assert_eq!(
    actual_placement.unwrap(),
    WindowPlacement::new_from_sizing(sizing),
    "Window should not have been resized"
  );
  assert_eq!(
    manager.windows_api.get_cursor_position(),
    initial_cursor,
    "Cursor should not have moved"
  );
}

#[test]
fn resize_spatial_window_steps_three_quarter_right_down_to_right_half_of_screen() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area = Rect::new(0, 0, 2000, 1000);
  let sizing = Sizing::three_quarter_near_maximised(work_area, Direction::Right, 20);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Right);

  let expected_sizing = Sizing::right_half_of_screen(work_area, 20);
  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  assert!(actual_placement.is_some());
  assert_eq!(
    actual_placement.unwrap(),
    WindowPlacement::new_from_sizing(expected_sizing.clone())
  );
  assert_eq!(
    manager.windows_api.get_cursor_position(),
    Point::from_center_of_sizing(&expected_sizing)
  );
}

#[test]
fn resize_spatial_window_steps_three_quarter_up_down_to_top_half_of_screen() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area = Rect::new(0, 0, 2000, 1000);
  let sizing = Sizing::three_quarter_near_maximised(work_area, Direction::Up, 20);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Up);

  let expected_sizing = Sizing::top_half_of_screen(work_area, 20);
  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  assert!(actual_placement.is_some());
  assert_eq!(
    actual_placement.unwrap(),
    WindowPlacement::new_from_sizing(expected_sizing.clone())
  );
  assert_eq!(
    manager.windows_api.get_cursor_position(),
    Point::from_center_of_sizing(&expected_sizing)
  );
}

#[test]
fn resize_spatial_window_three_quarter_left_produces_centre_when_pressing_right() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area = Rect::new(0, 0, 2000, 1000);
  let sizing = Sizing::three_quarter_near_maximised(work_area, Direction::Left, 20);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Right);

  let expected_sizing = Sizing::centre_near_maximised(work_area, Direction::Right, 20);
  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  assert!(actual_placement.is_some());
  assert_eq!(
    actual_placement.unwrap(),
    WindowPlacement::new_from_sizing(expected_sizing.clone())
  );
  assert_eq!(
    manager.windows_api.get_cursor_position(),
    Point::from_center_of_sizing(&expected_sizing)
  );
}

#[test]
fn resize_spatial_window_three_quarter_right_produces_centre_when_pressing_left() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area = Rect::new(0, 0, 2000, 1000);
  let sizing = Sizing::three_quarter_near_maximised(work_area, Direction::Right, 20);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Left);

  let expected_sizing = Sizing::centre_near_maximised(work_area, Direction::Left, 20);
  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  assert!(actual_placement.is_some());
  assert_eq!(
    actual_placement.unwrap(),
    WindowPlacement::new_from_sizing(expected_sizing.clone())
  );
  assert_eq!(
    manager.windows_api.get_cursor_position(),
    Point::from_center_of_sizing(&expected_sizing)
  );
}

#[test]
fn resize_spatial_window_three_quarter_up_produces_centre_when_pressing_down() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area = Rect::new(0, 0, 2000, 1000);
  let sizing = Sizing::three_quarter_near_maximised(work_area, Direction::Up, 20);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Down);

  let expected_sizing = Sizing::centre_near_maximised(work_area, Direction::Down, 20);
  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  assert!(actual_placement.is_some());
  assert_eq!(
    actual_placement.unwrap(),
    WindowPlacement::new_from_sizing(expected_sizing.clone())
  );
  assert_eq!(
    manager.windows_api.get_cursor_position(),
    Point::from_center_of_sizing(&expected_sizing)
  );
}

#[test]
fn resize_spatial_window_three_quarter_down_produces_centre_when_pressing_up() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area = Rect::new(0, 0, 2000, 1000);
  let sizing = Sizing::three_quarter_near_maximised(work_area, Direction::Down, 20);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Up);

  let expected_sizing = Sizing::centre_near_maximised(work_area, Direction::Up, 20);
  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  assert!(actual_placement.is_some());
  assert_eq!(
    actual_placement.unwrap(),
    WindowPlacement::new_from_sizing(expected_sizing.clone())
  );
  assert_eq!(
    manager.windows_api.get_cursor_position(),
    Point::from_center_of_sizing(&expected_sizing)
  );
}
#[test]
fn resize_spatial_window_steps_near_maximised_down_to_three_quarter_in_direction() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  // Monitor area bottom is 20px more than work_area bottom (mock subtracts 20 for taskbar)
  let work_area = Rect::new(0, 0, 2000, 1000);
  let sizing = Sizing::near_maximised(work_area, 20);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Left);

  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  let expected_sizing = Sizing::three_quarter_near_maximised(work_area, Direction::Left, 20);
  let expected_placement = WindowPlacement::new_from_sizing(expected_sizing.clone());
  assert!(actual_placement.is_some());
  assert_eq!(actual_placement.unwrap(), expected_placement);
  assert_eq!(
    manager.windows_api.get_cursor_position(),
    Point::from_center_of_sizing(&expected_sizing)
  );
}

#[test]
fn resize_spatial_window_halves_non_near_maximised_window_in_direction() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  // Halved width is 590px, above this monitor's 475px margin-aware quarter step.
  let sizing = Sizing::new(20, 20, 1200, 960);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing.clone(), false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Left);

  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  let expected_sizing = sizing.halved(Direction::Left, 20);
  let expected_placement = WindowPlacement::new_from_sizing(expected_sizing.clone());
  assert!(actual_placement.is_some());
  assert_eq!(actual_placement.unwrap(), expected_placement);
  assert_eq!(
    manager.windows_api.get_cursor_position(),
    Point::from_center_of_sizing(&expected_sizing)
  );
}

#[test]
fn resize_spatial_window_steps_near_maximised_down_to_three_quarter_down() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area = Rect::new(0, 0, 2000, 1000);
  let sizing = Sizing::near_maximised(work_area, 20);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Down);

  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  let expected_sizing = Sizing::three_quarter_near_maximised(work_area, Direction::Down, 20);
  let expected_placement = WindowPlacement::new_from_sizing(expected_sizing.clone());
  assert!(actual_placement.is_some());
  assert_eq!(actual_placement.unwrap(), expected_placement);
  assert_eq!(
    manager.windows_api.get_cursor_position(),
    Point::from_center_of_sizing(&expected_sizing)
  );
}

#[test]
fn resize_spatial_window_steps_near_maximised_down_to_three_quarter_right() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area = Rect::new(0, 0, 2000, 1000);
  let sizing = Sizing::near_maximised(work_area, 20);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Right);

  let expected_sizing = Sizing::three_quarter_near_maximised(work_area, Direction::Right, 20);
  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  assert!(actual_placement.is_some());
  assert_eq!(
    actual_placement.unwrap(),
    WindowPlacement::new_from_sizing(expected_sizing.clone())
  );
  assert_eq!(
    manager.windows_api.get_cursor_position(),
    Point::from_center_of_sizing(&expected_sizing)
  );
}

#[test]
fn resize_spatial_window_steps_near_maximised_down_to_three_quarter_up() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area = Rect::new(0, 0, 2000, 1000);
  let sizing = Sizing::near_maximised(work_area, 20);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.resize_spatial_window(Direction::Up);

  let expected_sizing = Sizing::three_quarter_near_maximised(work_area, Direction::Up, 20);
  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  assert!(actual_placement.is_some());
  assert_eq!(
    actual_placement.unwrap(),
    WindowPlacement::new_from_sizing(expected_sizing.clone())
  );
  assert_eq!(
    manager.windows_api.get_cursor_position(),
    Point::from_center_of_sizing(&expected_sizing)
  );
}
