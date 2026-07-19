use crate::api::{MockWindowsApi, WindowsApi};
use crate::common::{Direction, MonitorHandle, Point, Rect, Sizing, WindowHandle, WindowPlacement};
use crate::utils::{MINIMUM_WINDOW_DIMENSION, MINIMUM_WINDOW_DIMENSION_DIVISOR};
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
fn resize_spatial_window_does_nothing_when_dynamic_minimum_exceeds_constant_and_result_is_below_it() {
  // Use a screen wide enough that W/DIVISOR > MINIMUM_WINDOW_DIMENSION, so the dynamic minimum
  // takes over. Work area width = MINIMUM * DIVISOR * 2, giving dynamic_min = MINIMUM * 2.
  // The window is sized so its halved width falls between the constant and the dynamic minimum.
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area_width = MINIMUM_WINDOW_DIMENSION * MINIMUM_WINDOW_DIMENSION_DIVISOR * 2;
  // dynamic_min = work_area_width / DIVISOR = MINIMUM_WINDOW_DIMENSION * 2
  let sizing = Sizing::new(20, 20, MINIMUM_WINDOW_DIMENSION * 3, 600);
  // halved width = (MINIMUM * 3) / 2 - half_margin (with default margin 20)
  // = 375 - 10 = 365, which is > MINIMUM (250) but < dynamic_min (500) → blocked
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
fn resize_spatial_window_allows_resize_when_constant_is_larger_than_quarter_screen() {
  // Dynamic min = max(MINIMUM_WINDOW_DIMENSION, work_area/DIVISOR). On a 1000px-wide screen,
  // work_area/DIVISOR = 980/8 = 122 < MINIMUM_WINDOW_DIMENSION (250), so the constant wins.
  // A window whose halved width (365px) exceeds the constant should be allowed to resize.
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  // Monitor 1000px wide -> work_area 1000px wide -> quarter = 250 < MINIMUM_WINDOW_DIMENSION (350)
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
  // With DIVISOR=8, dynamic_min = max(MINIMUM, W/8). On a 2000px screen, dynamic_min = max(250, 250) = 250.
  // Halving left_half produces ~475px > 250 -> resize succeeds.
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area = Rect::new(0, 0, MINIMUM_WINDOW_DIMENSION * MINIMUM_WINDOW_DIMENSION_DIVISOR, 1000);
  let left_half = Sizing::left_half_of_screen(work_area, 20);
  MockWindowsApi::add_or_update_window(
    window_handle,
    "Test Window".to_string(),
    left_half.clone(),
    false,
    false,
    true,
  );
  MockWindowsApi::add_monitor(
    monitor_handle,
    Rect::new(0, 0, MINIMUM_WINDOW_DIMENSION * MINIMUM_WINDOW_DIMENSION_DIVISOR, 1020),
    true,
  );
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
fn resize_spatial_window_halves_right_half_of_screen_in_right_direction() {
  // Mirror of the Left case: with DIVISOR=8, dynamic_min = max(250, 250) = 250.
  // Halving right_half produces ~475px > 250 -> resize succeeds.
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area = Rect::new(0, 0, MINIMUM_WINDOW_DIMENSION * MINIMUM_WINDOW_DIMENSION_DIVISOR, 1000);
  let right_half = Sizing::right_half_of_screen(work_area, 20);
  MockWindowsApi::add_or_update_window(
    window_handle,
    "Test Window".to_string(),
    right_half.clone(),
    false,
    false,
    true,
  );
  MockWindowsApi::add_monitor(
    monitor_handle,
    Rect::new(0, 0, MINIMUM_WINDOW_DIMENSION * MINIMUM_WINDOW_DIMENSION_DIVISOR, 1020),
    true,
  );
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
  // Down halved height = 300/2 - half_margin = 140 < MINIMUM_WINDOW_DIMENSION (250) -> blocked.
  // On this screen, dynamic_min_height = max(250, 980/8) = 250, so the constant is the threshold.
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
fn resize_spatial_window_does_nothing_when_dynamic_minimum_height_blocks_resize() {
  // Work area height = MINIMUM * DIVISOR * 2 -> dynamic_min_height = MINIMUM * 2.
  // Window height = MINIMUM * 3; halved = MINIMUM * 3 / 2 - half_margin, which is
  // > MINIMUM (constant) but < dynamic_min (MINIMUM * 2) -> blocked.
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let work_area_height = MINIMUM_WINDOW_DIMENSION * MINIMUM_WINDOW_DIMENSION_DIVISOR * 2;
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
  // Width must be > 2 * dynamic_min (2 * W/4 = 1000) so that halved width (590) > dynamic_min (500)
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
