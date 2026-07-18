use crate::api::{MockWindowsApi, WindowsApi};
use crate::common::{MonitorHandle, MonitorInfo, Rect, Sizing, WindowHandle, WindowPlacement};
use crate::configuration_provider::WINDOW_MARGIN;
use crate::utils::create_temp_directory;
use crate::window_manager::WindowManager;
use crate::window_manager::placement::DWM_TOLERANCE_IN_PX;
use crate::window_manager::tests::test_support::set_margin as with_margin_set_to;

fn is_of_expected_size(
  manager: &WindowManager<MockWindowsApi>,
  handle: WindowHandle,
  placement: &WindowPlacement,
  sizing: &Sizing,
) -> bool {
  let margin = manager.configuration_provider.lock().unwrap().get_i32(WINDOW_MARGIN);
  let margin = if margin >= crate::utils::MINIMUM_WINDOW_MARGIN {
    margin
  } else {
    0
  };
  manager
    .placement
    .is_of_expected_size(&manager.windows_api, handle, placement, sizing, margin)
}

fn near_maximise_window(
  manager: &WindowManager<MockWindowsApi>,
  handle: WindowHandle,
  monitor_info: MonitorInfo,
  margin: i32,
) {
  manager
    .placement
    .near_maximise(&manager.windows_api, handle, monitor_info, margin);
}

#[test]
fn is_of_expected_size_test() {
  let handle = WindowHandle::new(1);
  let placement = WindowPlacement::new_from_sizing(Sizing::new(0, 0, 100, 100));
  let sizing = Sizing::new(0, 0, 100, 100);
  let manager = WindowManager::default(MockWindowsApi);
  assert!(is_of_expected_size(&manager, handle, &placement, &sizing));

  let placement = WindowPlacement::new_from_sizing(Sizing::new(1, 0, 101, 100));
  assert!(!is_of_expected_size(&manager, handle, &placement, &sizing));
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
  let mut manager = WindowManager::default(MockWindowsApi);

  manager.near_maximise_or_restore();

  let actual_placement = manager.windows_api.get_window_placement(window_handle);
  let expected_placement = WindowPlacement::new_from_sizing(Sizing::new(20, 20, 160, 140));
  assert!(actual_placement.is_some());
  assert_eq!(actual_placement.unwrap(), expected_placement);
  assert!(
    manager
      .placement
      .known_windows
      .contains_key(&format!("{:?}", window_handle.hwnd))
  );
  assert_eq!(
    *manager
      .placement
      .known_windows
      .get(&format!("{:?}", window_handle.hwnd))
      .unwrap(),
    initial_placement
  );
}

#[test]
fn restore_window_when_window_is_near_maximised() {
  let monitor_handle = MonitorHandle::from(1);
  let window_handle = WindowHandle::new(1);
  let sizing = Sizing::new(20, 20, 160, 140);
  MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
  MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
  MockWindowsApi::place_window(window_handle, monitor_handle);
  let mut manager = WindowManager::default(MockWindowsApi);
  let previous_placement = WindowPlacement::new_test();
  manager
    .placement
    .known_windows
    .insert(format!("{:?}", window_handle.hwnd), previous_placement.clone());

  manager.near_maximise_or_restore();

  let actual_placement = manager.windows_api.get_window_placement(window_handle);
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
  let manager = WindowManager::default(MockWindowsApi);
  let monitor_info = manager
    .windows_api
    .get_monitor_info_for_monitor(monitor_handle)
    .expect("Failed to get monitor info");

  near_maximise_window(&manager, window_handle, monitor_info, 3);

  let expected_sizing = Sizing::near_maximised(monitor_info.work_area, 0);
  let expected_placement = WindowPlacement::new_from_sizing(expected_sizing);
  let actual_placement = manager.windows_api.get_window_placement(window_handle);
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
  let manager = WindowManager::default(MockWindowsApi);
  let monitor_info = manager
    .windows_api
    .get_monitor_info_for_monitor(monitor_handle)
    .expect("Failed to get monitor info");
  let margin = 10;

  near_maximise_window(&manager, window_handle, monitor_info, margin);

  let actual_placement = manager.windows_api.get_window_placement(window_handle);
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
  let mut manager = WindowManager::default(MockWindowsApi);
  let monitor_info = manager
    .windows_api
    .get_monitor_info_for_monitor(monitor_handle)
    .expect("Failed to get monitor info");
  with_margin_set_to(0, &mut manager);

  manager.near_maximise_or_restore();

  let expected_sizing = Sizing::near_maximised(monitor_info.work_area, 0);
  let maximised_placement = WindowPlacement::new_from_sizing(expected_sizing);
  let current_placement = manager
    .windows_api
    .get_window_placement(window_handle)
    .expect("Failed to get placement after maximise");
  assert_eq!(current_placement, maximised_placement, "Window should be maximised");

  manager.near_maximise_or_restore();

  let current_placement = manager
    .windows_api
    .get_window_placement(window_handle)
    .expect("Failed to get placement after restore");
  assert_eq!(
    current_placement, initial_placement,
    "Window should be restored to its initial placement"
  );
}

#[test]
fn is_of_expected_size_returns_true_when_same_size() {
  let directory = create_temp_directory();
  let path = directory.path().join("file.toml");
  let mut manager = WindowManager::new_test(MockWindowsApi, path);
  let window_handle = WindowHandle::new(1);
  let rect = Rect::new(0, 0, 1280, 720);
  let placement = WindowPlacement::new_from_rect(rect);
  let sizing = rect.into();
  with_margin_set_to(10, &mut manager);

  assert!(is_of_expected_size(&manager, window_handle, &placement, &sizing));
}

#[test]
fn is_of_expected_size_returns_false_when_different_size() {
  let directory = create_temp_directory();
  let path = directory.path().join("file.toml");
  let mut manager = WindowManager::new_test(MockWindowsApi, path);
  let window_handle = WindowHandle::new(1);
  let rect = Rect::new(0, 0, 1280, 720);
  let placement = WindowPlacement::new_from_rect(rect);
  let mut sizing: Sizing = rect.into();
  sizing.x += 5; // Introducing a discrepancy here
  with_margin_set_to(10, &mut manager);

  assert!(!is_of_expected_size(&manager, window_handle, &placement, &sizing));
}

#[test]
fn is_of_expected_size_returns_true_when_difference_is_within_dwm_tolerance() {
  let directory = create_temp_directory();
  let path = directory.path().join("file.toml");
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
  let mut manager = WindowManager::new_test(MockWindowsApi, path);
  with_margin_set_to(0, &mut manager);

  assert!(is_of_expected_size(&manager, window_handle, &placement, &expected_sizing));
}

#[test]
fn is_of_expected_size_returns_false_when_difference_is_outside_dwm_tolerance() {
  let directory = create_temp_directory();
  let path = directory.path().join("file.toml");
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
  let mut manager = WindowManager::new_test(MockWindowsApi, path);
  with_margin_set_to(0, &mut manager);

  assert!(!is_of_expected_size(&manager, window_handle, &placement, &expected_sizing));
}
