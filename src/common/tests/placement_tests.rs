use crate::api::{MockWindowsApi, WindowsApi};
use crate::common::placement::DWM_TOLERANCE_IN_PX;
use crate::common::{MonitorHandle, MonitorInfo, Placement, Rect, Sizing, WindowHandle, WindowPlacement};

fn is_of_expected_size(
  placement_manager: &Placement,
  handle: WindowHandle,
  placement: &WindowPlacement,
  sizing: &Sizing,
  margin: i32,
) -> bool {
  placement_manager.is_of_expected_size(&MockWindowsApi, handle, placement, sizing, margin)
}

fn near_maximise_window(placement: &Placement, handle: WindowHandle, monitor_info: MonitorInfo, margin: i32) {
  placement.near_maximise(&MockWindowsApi, handle, monitor_info, margin);
}

#[test]
fn is_of_expected_size_test() {
  let handle = WindowHandle::new(1);
  let placement = WindowPlacement::new_from_sizing(Sizing::new(0, 0, 100, 100));
  let sizing = Sizing::new(0, 0, 100, 100);
  let placement_manager = Placement::default();
  assert!(is_of_expected_size(&placement_manager, handle, &placement, &sizing, 20));

  let placement = WindowPlacement::new_from_sizing(Sizing::new(1, 0, 101, 100));
  assert!(!is_of_expected_size(&placement_manager, handle, &placement, &sizing, 20));
}

#[test]
fn near_maximise_window_when_window_is_not_near_maximised() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let sizing = Sizing::new(0, 0, 100, 100);
  let initial_placement = WindowPlacement::new_from_sizing(sizing.clone());
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let monitor_info = MockWindowsApi
    .get_monitor_info_for_monitor(monitor_handle)
    .expect("Failed to get monitor info");
  let mut placement = Placement::default();

  placement.near_maximise_or_restore(&MockWindowsApi, window_handle, initial_placement.clone(), monitor_info, 20);

  let actual_placement = MockWindowsApi.get_window_placement(window_handle);
  let expected_placement = WindowPlacement::new_from_sizing(Sizing::new(20, 20, 160, 140));
  assert!(actual_placement.is_some());
  assert_eq!(actual_placement.unwrap(), expected_placement);
  assert!(placement.known_windows.contains_key(&format!("{:?}", window_handle.hwnd)));
  assert_eq!(
    *placement.known_windows.get(&format!("{:?}", window_handle.hwnd)).unwrap(),
    initial_placement
  );
}

#[test]
fn restore_window_when_window_is_near_maximised() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let sizing = Sizing::new(20, 20, 160, 140);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing.clone(), false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let monitor_info = MockWindowsApi
    .get_monitor_info_for_monitor(monitor_handle)
    .expect("Failed to get monitor info");
  let current_placement = WindowPlacement::new_from_sizing(sizing);
  let previous_placement = WindowPlacement::new_test();
  let mut placement = Placement::default();
  placement
    .known_windows
    .insert(format!("{:?}", window_handle.hwnd), previous_placement.clone());

  placement.near_maximise_or_restore(&MockWindowsApi, window_handle, current_placement, monitor_info, 20);

  let actual_placement = MockWindowsApi.get_window_placement(window_handle);
  assert!(actual_placement.is_some());
  assert_eq!(actual_placement.unwrap(), previous_placement);
}

#[test]
fn near_maximise_window_with_margin_below_threshold_does_not_resize() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let initial_sizing = Sizing::new(50, 50, 100, 100);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), initial_sizing, false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let placement = Placement::default();
  let monitor_info = MockWindowsApi
    .get_monitor_info_for_monitor(monitor_handle)
    .expect("Failed to get monitor info");

  near_maximise_window(&placement, window_handle, monitor_info, 3);

  let expected_sizing = Sizing::near_maximised(monitor_info.work_area, 0);
  let expected_placement = WindowPlacement::new_from_sizing(expected_sizing);
  let actual_placement = MockWindowsApi.get_window_placement(window_handle);
  assert!(actual_placement.is_some());
  assert_eq!(actual_placement.unwrap(), expected_placement);
}

#[test]
fn near_maximise_window_with_margin_above_threshold_resizes() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let initial_sizing = Sizing::new(50, 50, 100, 100);
  let initial_placement = WindowPlacement::new_from_sizing(initial_sizing.clone());
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), initial_sizing, false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let placement = Placement::default();
  let monitor_info = MockWindowsApi
    .get_monitor_info_for_monitor(monitor_handle)
    .expect("Failed to get monitor info");
  let margin = 10;

  near_maximise_window(&placement, window_handle, monitor_info, margin);

  let actual_placement = MockWindowsApi.get_window_placement(window_handle);
  let expected_sizing = Sizing::near_maximised(monitor_info.work_area, margin);
  let expected_placement = WindowPlacement::new_from_sizing(expected_sizing);
  assert!(actual_placement.is_some());
  assert_eq!(actual_placement.clone().unwrap(), expected_placement);
  assert_ne!(actual_placement.unwrap(), initial_placement);
}

#[test]
fn near_maximise_or_restore_with_zero_margin_can_restore_initial_position() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let initial_sizing = Sizing::new(50, 50, 100, 100);
  let initial_placement = WindowPlacement::new_from_sizing(initial_sizing.clone());
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), initial_sizing, false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let monitor_info = MockWindowsApi
    .get_monitor_info_for_monitor(monitor_handle)
    .expect("Failed to get monitor info");
  let mut placement = Placement::default();

  placement.near_maximise_or_restore(&MockWindowsApi, window_handle, initial_placement.clone(), monitor_info, 0);

  let expected_sizing = Sizing::near_maximised(monitor_info.work_area, 0);
  let maximised_placement = WindowPlacement::new_from_sizing(expected_sizing);
  let current_placement = MockWindowsApi
    .get_window_placement(window_handle)
    .expect("Failed to get placement after maximise");
  assert_eq!(current_placement, maximised_placement, "Window should be maximised");

  placement.near_maximise_or_restore(&MockWindowsApi, window_handle, current_placement, monitor_info, 0);

  let current_placement = MockWindowsApi
    .get_window_placement(window_handle)
    .expect("Failed to get placement after restore");
  assert_eq!(
    current_placement, initial_placement,
    "Window should be restored to its initial placement"
  );
}

#[test]
fn is_of_expected_size_returns_true_when_same_size() {
  let placement_manager = Placement::default();
  let window_handle = WindowHandle::new(1);
  let rect = Rect::new(0, 0, 1280, 720);
  let placement = WindowPlacement::new_from_rect(rect);
  let sizing = rect.into();

  assert!(is_of_expected_size(
    &placement_manager,
    window_handle,
    &placement,
    &sizing,
    10
  ));
}

#[test]
fn is_of_expected_size_returns_false_when_different_size() {
  let placement_manager = Placement::default();
  let window_handle = WindowHandle::new(1);
  let rect = Rect::new(0, 0, 1280, 720);
  let placement = WindowPlacement::new_from_rect(rect);
  let mut sizing: Sizing = rect.into();
  sizing.x += 5; // Introducing a discrepancy here

  assert!(!is_of_expected_size(
    &placement_manager,
    window_handle,
    &placement,
    &sizing,
    10
  ));
}

#[test]
fn is_of_expected_size_returns_true_when_difference_is_within_dwm_tolerance() {
  let placement_manager = Placement::default();
  let window_handle = WindowHandle::new(1);
  let rect = Rect::new(0, 0, 1280, 720);
  let placement = WindowPlacement::new_from_rect(rect);
  let expected_sizing = Sizing::new(0, 0, 640, 720);
  // Windows API returns a different sizing that is within the tolerance for a no-margin configuration
  let api_sizing = Sizing::new(DWM_TOLERANCE_IN_PX - 1, DWM_TOLERANCE_IN_PX - 1, 640, 720);
  MockWindowsApi::add_or_update_window(
    window_handle,
    "Test Window".to_string(),
    api_sizing.clone(),
    false,
    false,
    true,
  );

  assert!(is_of_expected_size(
    &placement_manager,
    window_handle,
    &placement,
    &expected_sizing,
    0
  ));
}

#[test]
fn is_of_expected_size_returns_false_when_difference_is_outside_dwm_tolerance() {
  let placement_manager = Placement::default();
  let window_handle = WindowHandle::new(1);
  let rect = Rect::new(0, 0, 1280, 720);
  let placement = WindowPlacement::new_from_rect(rect);
  let expected_sizing = Sizing::new(0, 0, 640, 720);
  // Windows API returns a different sizing that is outside the tolerance for a no-margin configuration
  let api_sizing = Sizing::new(15, 0, 640, 720);
  MockWindowsApi::add_or_update_window(
    window_handle,
    "Test Window".to_string(),
    api_sizing.clone(),
    false,
    false,
    true,
  );

  assert!(!is_of_expected_size(
    &placement_manager,
    window_handle,
    &placement,
    &expected_sizing,
    0
  ));
}
