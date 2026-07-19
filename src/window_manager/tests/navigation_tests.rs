use crate::api::{MockWindowsApi, WindowsApi};
use crate::common::{Direction, MonitorHandle, Point, Rect, Sizing, Window, WindowHandle};
use crate::window_manager::WindowManager;
use crate::window_manager::navigation::find_closest_window as super_find_closest_window;
use crate::window_manager::navigation::select_window_in_direction;

#[cfg(test)]
fn find_closest_window(
  manager: &WindowManager<MockWindowsApi>,
  cursor_position: Point,
  ignored_window: Option<WindowHandle>,
) -> Option<WindowHandle> {
  super_find_closest_window(&manager.windows_api, cursor_position, ignored_window)
}

#[test]
fn select_window_in_direction_cycles_through_all_windows() {
  let rect = Rect::new(0, 0, 100, 100);
  let first = Window::new_test(1, rect);
  let second = Window::new_test(2, rect);
  let third = Window::new_test(3, rect);
  let windows = [&third, &first, &second];

  assert_eq!(
    select_window_in_direction(&first.center, Direction::Right, &windows, Some(&first), true).map(|window| window.handle),
    Some(second.handle)
  );
  assert_eq!(
    select_window_in_direction(&second.center, Direction::Right, &windows, Some(&second), true).map(|window| window.handle),
    Some(third.handle)
  );
  assert_eq!(
    select_window_in_direction(&third.center, Direction::Right, &windows, Some(&third), true).map(|window| window.handle),
    Some(first.handle)
  );
}

#[test]
fn select_window_in_direction_uses_direction_when_disabled() {
  let reference = Window::new_test(1, Rect::new(0, 0, 100, 100));
  let same_center = Window::new_test(2, Rect::new(0, 0, 100, 100));
  let right = Window::new_test(3, Rect::new(100, 0, 200, 100));
  let windows = [&reference, &same_center, &right];

  let selected = select_window_in_direction(&reference.center, Direction::Right, &windows, Some(&reference), false);

  assert_eq!(selected.map(|window| window.handle), Some(right.handle));
}

#[test]
fn select_window_in_direction_falls_back_to_closest_window_in_direction() {
  let reference = Window::new_test(1, Rect::new(0, 0, 100, 100));
  let closest_right = Window::new_test(2, Rect::new(100, 0, 200, 100));
  let furthest_right = Window::new_test(3, Rect::new(200, 0, 300, 100));
  let windows = [&reference, &furthest_right, &closest_right];

  let selected = select_window_in_direction(&reference.center, Direction::Right, &windows, Some(&reference), true);

  assert_eq!(selected.map(|window| window.handle), Some(closest_right.handle));
}

#[test]
fn move_cursor_moves_cursor_to_center_of_closest_window_on_other_monitor() {
  let current_monitor_handle = MonitorHandle::from(1);
  let target_monitor_handle = MonitorHandle::from(2);
  let target_work_area = Rect::new(200, 0, 400, 200);
  let window_handle = WindowHandle::new(1);
  let sizing = Sizing::right_half_of_screen(target_work_area, 20);
  MockWindowsApi::set_cursor_position(Point::new(0, 0));
  MockWindowsApi::add_or_update_window(window_handle, "Test".to_string(), sizing, false, false, true);
  MockWindowsApi::add_monitor(current_monitor_handle, Rect::new(0, 0, 200, 200), true);
  MockWindowsApi::add_monitor(target_monitor_handle, target_work_area, true);
  MockWindowsApi::place_window(window_handle, target_monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.move_cursor(Direction::Right);

  assert_eq!(manager.windows_api.get_foreground_window(), Some(window_handle));
  assert_eq!(manager.windows_api.get_cursor_position(), target_work_area.center());
}

#[test]
fn move_cursor_does_nothing_when_there_is_no_window_or_monitor_to_move_to() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle_1 = WindowHandle::new(1);
  let left_sizing = Sizing::left_half_of_screen(Rect::new(0, 0, 200, 180), 20);
  let initial_cursor_position = Point::new(0, 0);
  MockWindowsApi::set_cursor_position(initial_cursor_position);
  MockWindowsApi::add_or_update_window(window_handle_1, "Test".to_string(), left_sizing, false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
  MockWindowsApi::place_window(window_handle_1, monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.move_cursor(Direction::Right);

  assert_eq!(manager.windows_api.get_cursor_position(), initial_cursor_position);
}

#[test]
fn close_window_does_not_move_cursor_to_closest_window_when_disabled() {
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
  manager.allow_moving_cursor_after_close_or_minimise = false;

  manager.close_window();

  assert!(
    !manager
      .windows_api
      .get_all_visible_windows()
      .iter()
      .any(|w| w.handle == window_handle_1)
  );
  assert_eq!(manager.windows_api.get_cursor_position(), Point::default());
}

#[test]
fn find_closest_window_returns_none_when_no_windows_are_visible() {
  let cursor_position = Point::new(100, 100);
  let manager = WindowManager::default(MockWindowsApi);

  let result = find_closest_window(&manager, cursor_position, None);

  assert!(result.is_none());
}

#[test]
fn find_closest_window_returns_window_under_cursor() {
  let cursor_position = Point::new(50, 50);
  let window_handle = WindowHandle::new(1);
  MockWindowsApi::set_cursor_position(cursor_position);
  MockWindowsApi::add_or_update_window(
    window_handle,
    "Test Window".to_string(),
    Rect::new(0, 0, 100, 100).into(),
    false,
    false,
    true,
  );
  let manager = WindowManager::default(MockWindowsApi);

  let result = find_closest_window(&manager, cursor_position, None);

  assert_eq!(result, Some(window_handle));
}

#[test]
fn find_closest_window_returns_smallest_window_when_multiple_windows_overlap() {
  let cursor_position = Point::new(50, 50);
  let expected_window_handle = WindowHandle::new(2);
  MockWindowsApi::set_cursor_position(cursor_position);
  MockWindowsApi::add_or_update_window(
    WindowHandle::new(1),
    "Window 1".to_string(),
    Rect::new(40, 40, 60, 60).into(),
    false,
    false,
    true,
  );
  MockWindowsApi::add_or_update_window(
    expected_window_handle,
    "Window 2".to_string(),
    Rect::new(45, 45, 55, 55).into(),
    false,
    false,
    false,
  );
  let manager = WindowManager::default(MockWindowsApi);

  let result = find_closest_window(&manager, cursor_position, None);

  assert_eq!(result, Some(expected_window_handle));
}

#[test]
fn find_closest_window_returns_closest_window_when_cursor_is_outside_all_windows() {
  let cursor_position = Point::new(1000, 1000);
  let window_handle = WindowHandle::new(1);
  MockWindowsApi::set_cursor_position(cursor_position);
  MockWindowsApi::add_or_update_window(
    window_handle,
    "Test Window".to_string(),
    Rect::new(0, 0, 100, 100).into(),
    false,
    false,
    false,
  );
  let manager = WindowManager::default(MockWindowsApi);

  let result = find_closest_window(&manager, cursor_position, None);

  assert_eq!(result, Some(window_handle));
}

#[test]
fn find_closest_window_ignores_minimised_or_hidden_windows() {
  let cursor_position = Point::new(50, 50);
  let expected_window_handle = WindowHandle::new(2);
  MockWindowsApi::set_cursor_position(cursor_position);
  MockWindowsApi::add_or_update_window(
    WindowHandle::new(1),
    "Window 1".to_string(),
    Rect::new(40, 40, 60, 60).into(),
    true,
    false,
    true,
  );
  MockWindowsApi::add_or_update_window(
    expected_window_handle,
    "Window 2".to_string(),
    Rect::new(45, 45, 55, 55).into(),
    false,
    true,
    false,
  );
  let manager = WindowManager::default(MockWindowsApi);

  let result = find_closest_window(&manager, cursor_position, None);

  assert!(result.is_none());
}

#[test]
fn find_closest_window_ignores_provided_window() {
  let cursor_position = Point::new(50, 50);
  let expected_window_handle = WindowHandle::new(2);
  MockWindowsApi::set_cursor_position(cursor_position);
  MockWindowsApi::add_or_update_window(
    WindowHandle::new(1),
    "Window 1".to_string(),
    Rect::new(0, 0, 60, 60).into(),
    true,
    false,
    true,
  );
  let manager = WindowManager::default(MockWindowsApi);

  let result = find_closest_window(&manager, cursor_position, Some(expected_window_handle));

  assert!(result.is_none());
}
