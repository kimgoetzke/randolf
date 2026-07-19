use crate::api::{MockWindowsApi, WindowsApi};
use crate::common::{Monitor, MonitorHandle, PersistentWorkspaceId, Rect, Sizing, Window, WindowHandle, Workspace};

impl Workspace {
  /// Creates a new workspace for testing purposes with margin set to 0 and inactive by default.
  pub fn new_test(id: PersistentWorkspaceId, monitor: &Monitor) -> Self {
    Self::new_inactive(id, monitor, 0)
  }

  pub fn get_windows(&self) -> Vec<Window> {
    self.windows.clone()
  }

  pub fn get_window_state_info(&self) -> Vec<(WindowHandle, bool)> {
    self.minimised_windows.clone()
  }
}

#[test]
fn update_window_rect_if_required_returns_window_unchanged_when_staying_on_same_monitor() {
  let monitor = Monitor::new_test(1, Rect::new(0, 0, 1920, 1080));
  let mut workspace = Workspace::new_test(PersistentWorkspaceId::new(monitor.id, 1, true), &monitor);
  let window = Window::new_test(1, Rect::new(10, 10, 110, 110));
  let mock_api = MockWindowsApi::new();

  let updated_window = workspace.update_window_rect_if_required(window.clone(), monitor.handle, &mock_api);

  assert_eq!(updated_window.rect, window.rect);
}

#[test]
fn update_window_rect_if_required_maintains_near_maximised_layout_when_changing_monitors() {
  let target_monitor = Monitor::new_test(2, Rect::new(0, 0, 1920, 1080));
  let mut workspace = Workspace::new_test(PersistentWorkspaceId::new(target_monitor.id, 1, true), &target_monitor);
  let current_monitor_handle = MonitorHandle::from(1);
  MockWindowsApi::add_monitor(current_monitor_handle, Rect::new(0, 0, 800, 600), true);
  let mock_api = MockWindowsApi::new();
  let current_monitor = mock_api.get_monitor_info_for_monitor(current_monitor_handle).unwrap();
  let current_sizing_near_maximised = Sizing::near_maximised(current_monitor.work_area, workspace.margin);
  let window = Window::new_test(1, current_sizing_near_maximised.into());

  let updated_window = workspace.update_window_rect_if_required(window, current_monitor_handle, &mock_api);

  let expected_sizing = Sizing::near_maximised(target_monitor.work_area, workspace.margin);
  assert_eq!(updated_window.rect, expected_sizing.into());
}

#[test]
fn update_window_rect_if_required_maintains_left_half_layout_when_changing_monitors() {
  let target_monitor = Monitor::new_test(2, Rect::new(0, 0, 1920, 1080));
  let mut workspace = Workspace::new_test(PersistentWorkspaceId::new(target_monitor.id, 1, true), &target_monitor);
  let current_monitor_handle = MonitorHandle::from(1);
  MockWindowsApi::add_monitor(current_monitor_handle, Rect::new(0, 0, 800, 600), true);
  let mock_api = MockWindowsApi::new();
  let current_monitor = mock_api.get_monitor_info_for_monitor(current_monitor_handle).unwrap();
  let current_sizing_left_half = Sizing::left_half_of_screen(current_monitor.work_area, workspace.margin);
  let window = Window::new_test(1, current_sizing_left_half.into());

  let updated_window = workspace.update_window_rect_if_required(window, current_monitor_handle, &mock_api);

  let expected_sizing = Sizing::left_half_of_screen(target_monitor.work_area, workspace.margin);
  assert_eq!(updated_window.rect, expected_sizing.into());
}

#[test]
fn update_window_rect_if_required_centers_normal_window_when_changing_monitors() {
  let source_monitor = Monitor::new_test(1, Rect::new(0, 0, 1000, 800));
  let target_monitor = Monitor::new_test(2, Rect::new(0, 0, 1920, 1080));
  let mut workspace = Workspace::new_test(PersistentWorkspaceId::new(target_monitor.id, 1, true), &target_monitor);
  let window = Window::new_test(1, Rect::new(100, 100, 300, 200));
  MockWindowsApi::add_monitor(source_monitor.handle, source_monitor.monitor_area, true);
  let mock_api = MockWindowsApi::new();

  let updated_window = workspace.update_window_rect_if_required(window, source_monitor.handle, &mock_api);

  assert_eq!(updated_window.rect.width(), 200);
  assert_eq!(updated_window.rect.height(), 100);
  assert_eq!(updated_window.center, target_monitor.work_area.center());
}

#[test]
fn update_window_rect_if_required_centers_window_when_monitor_info_missing() {
  let source_monitor = Monitor::new_test(1, Rect::new(0, 0, 1024, 768));
  let target_monitor = Monitor::new_test(2, Rect::new(0, 0, 1920, 1080));
  let mut workspace = Workspace::new_test(PersistentWorkspaceId::new(target_monitor.id, 1, true), &target_monitor);
  let current_sizing_near_maximised = Sizing::near_maximised(source_monitor.work_area, workspace.margin);
  let window = Window::new_test(1, current_sizing_near_maximised.into());
  let mock_api = MockWindowsApi::new();

  let updated_window = workspace.update_window_rect_if_required(window.clone(), source_monitor.handle, &mock_api);

  assert_eq!(updated_window.rect.width(), 1024);
  assert_eq!(updated_window.rect.height(), 768);
  assert_eq!(updated_window.center, target_monitor.work_area.center());
}

#[test]
fn move_or_store_and_hide_window_stores_window_if_workspace_is_inactive() {
  let monitor = Monitor::new_test(1, Rect::default());
  let workspace_id = PersistentWorkspaceId::new(monitor.id, 1, true);
  let mut workspace = Workspace::new_test(workspace_id, &monitor); // Inactive by default
  let window = Window::new_test(1, Rect::new(0, 0, 100, 100));
  MockWindowsApi::add_or_update_window(window.handle, window.title.clone(), window.rect.into(), false, false, true);
  let mock_api = MockWindowsApi::new();

  workspace.move_or_store_and_hide_window(window.clone(), monitor.handle, &mock_api);

  assert_eq!(mock_api.get_all_visible_windows().len(), 0);
  assert_eq!(workspace.windows.len(), 1);
  assert_eq!(workspace.minimised_windows.len(), 1);
}

#[test]
fn move_or_store_and_hide_window_moves_window_if_workspace_is_active() {
  let monitor = Monitor::new_test(1, Rect::default());
  let workspace_id = PersistentWorkspaceId::new(monitor.id, 1, true);
  let mut workspace = Workspace::new_active(workspace_id, &monitor, 20);
  let window = Window::new_test(1, Rect::new(0, 0, 100, 100));
  MockWindowsApi::add_or_update_window(window.handle, window.title.clone(), window.rect.into(), false, false, true);
  let mock_api = MockWindowsApi::new();

  workspace.move_or_store_and_hide_window(window.clone(), monitor.handle, &mock_api);

  let visible_windows = mock_api.get_all_visible_windows();
  assert_eq!(visible_windows.len(), 1);
  assert_eq!(visible_windows[0].handle, window.handle);
  assert!(workspace.windows.is_empty());
  assert_eq!(workspace.minimised_windows.len(), 0);
}

#[test]
fn store_and_hide_window_stores_and_hide_window() {
  let monitor = Monitor::new_test(1, Rect::default());
  let mut workspace = Workspace::new_test(PersistentWorkspaceId::new(monitor.id, 1, true), &monitor);
  let window = Window::new_test(1, Rect::new(0, 0, 100, 100));
  MockWindowsApi::add_or_update_window(window.handle, window.title.clone(), window.rect.into(), false, false, true);
  let mock_api = MockWindowsApi::new();

  workspace.store_and_hide_window(window.clone(), monitor.handle, &mock_api);

  assert_eq!(mock_api.get_all_visible_windows().len(), 0);
  assert_eq!(workspace.windows.len(), 1);
  assert_eq!(workspace.windows[0].title, window.title);
  assert_eq!(workspace.windows[0].handle, window.handle);
  assert_eq!(workspace.windows[0].rect, Rect::new(0, 0, 100, 100));
  assert_eq!(workspace.minimised_windows[0].0, window.handle);
  assert!(!workspace.minimised_windows[0].1);
}

#[test]
fn store_and_hide_window_does_not_add_duplicate_window_but_hides_it() {
  let mut workspace = Workspace::new_test(PersistentWorkspaceId::new_test(1), &Monitor::mock_1());
  let window = Window::new_test(1, Rect::new(0, 0, 100, 100));
  MockWindowsApi::add_or_update_window(window.handle, window.title.clone(), window.rect.into(), false, false, true);
  let mock_api = MockWindowsApi;

  workspace.store_and_hide_window(window.clone(), 1.into(), &mock_api);
  workspace.store_and_hide_window(window.clone(), 1.into(), &mock_api);

  assert_eq!(workspace.get_windows().len(), 1);
  assert!(mock_api.is_window_hidden(&window.handle));
}

#[test]
fn store_and_hide_windows_adds_windows_to_workspace() {
  let mut workspace = Workspace::new_test(PersistentWorkspaceId::new_test(1), &Monitor::mock_1());
  let window_1 = Window::new_test(1, Rect::new(0, 0, 100, 100));
  let window_2 = Window::new_test(2, Rect::new(100, 100, 200, 200));
  let mock_api = MockWindowsApi;
  MockWindowsApi::add_or_update_window(
    window_1.handle,
    window_1.title.clone(),
    window_1.rect.into(),
    false,
    false,
    true,
  );
  MockWindowsApi::add_or_update_window(
    window_2.handle,
    window_2.title.clone(),
    window_2.rect.into(),
    false,
    false,
    true,
  );

  workspace.store_and_hide_windows(vec![window_1.clone(), window_2.clone()], 1.into(), &mock_api);

  assert_eq!(workspace.get_windows().len(), 2);
  assert!(workspace.get_windows().contains(&window_1));
  assert!(workspace.get_windows().contains(&window_2));
}

#[test]
fn stores_returns_true_if_window_is_in_workspace() {
  let monitor = Monitor::new_test(1, Rect::default());
  let mut workspace = Workspace::new_test(PersistentWorkspaceId::new(monitor.id, 1, true), &monitor);
  let window = Window::new_test(1, Rect::new(0, 0, 100, 100));
  workspace.windows.push(window.clone());

  assert!(workspace.stores(&window.handle));
  assert!(!workspace.stores(&WindowHandle::new(42)));
}

#[test]
fn stores_returns_false_if_window_is_not_in_workspace() {
  let monitor = Monitor::new_test(1, Rect::default());
  let workspace = Workspace::new_test(PersistentWorkspaceId::new(monitor.id, 1, true), &monitor);

  assert!(!workspace.stores(&WindowHandle::new(2)));
}

#[test]
fn restore_windows_restores_all_windows() {
  let mut workspace = Workspace::new_test(PersistentWorkspaceId::new_test(1), &Monitor::mock_1());
  let sizing_window_1 = Sizing::new(0, 0, 100, 100);
  let sizing_window_2 = Sizing::new(100, 100, 100, 100);
  MockWindowsApi::add_or_update_window(1.into(), "Test Window 1".to_string(), sizing_window_1, false, false, true);
  MockWindowsApi::add_or_update_window(2.into(), "Test Window 2".to_string(), sizing_window_2, false, false, true);
  let mock_api = MockWindowsApi;
  let windows = mock_api.get_all_visible_windows();
  workspace.store_and_hide_windows(windows, 1.into(), &mock_api);

  workspace.restore_windows(&mock_api);

  let windows = mock_api.get_all_visible_windows();
  assert_eq!(windows.len(), 2);
  assert!(workspace.get_windows().is_empty());
}

#[test]
fn restore_windows_handles_empty_workspace() {
  let mut workspace = Workspace::new_test(PersistentWorkspaceId::new_test(1), &Monitor::mock_1());
  let mock_api = MockWindowsApi;

  workspace.restore_windows(&mock_api);

  assert!(workspace.get_windows().is_empty());
}

#[test]
fn remove_windows_if_present_removes_specified_windows() {
  let mut workspace = Workspace::new_test(PersistentWorkspaceId::new_test(1), &Monitor::mock_1());
  let window_1 = Window::new_test(1, Rect::new(0, 0, 100, 100));
  let window_2 = Window::new_test(2, Rect::new(100, 100, 200, 200));
  workspace.windows.push(window_1.clone());
  workspace.windows.push(window_2.clone());
  workspace.minimised_windows.push((window_1.handle, false));
  workspace.minimised_windows.push((window_2.handle, true));

  workspace.remove_windows_if_present(std::slice::from_ref(&window_1));

  assert_eq!(workspace.windows.len(), 1);
  assert_eq!(workspace.windows[0].handle, window_2.handle);
  assert_eq!(workspace.minimised_windows.len(), 1);
  assert_eq!(workspace.minimised_windows[0].0, window_2.handle);
}

#[test]
fn remove_windows_if_present_does_nothing_if_windows_not_present() {
  let mut workspace = Workspace::new_test(PersistentWorkspaceId::new_test(1), &Monitor::mock_1());
  let window_1 = Window::new_test(1, Rect::new(0, 0, 100, 100));
  let window_2 = Window::new_test(2, Rect::new(100, 100, 200, 200));
  workspace.windows.push(window_1.clone());
  workspace.minimised_windows.push((window_1.handle, false));

  workspace.remove_windows_if_present(std::slice::from_ref(&window_2));

  assert_eq!(workspace.windows.len(), 1);
  assert_eq!(workspace.windows[0].handle, window_1.handle);
  assert_eq!(workspace.minimised_windows.len(), 1);
  assert_eq!(workspace.minimised_windows[0].0, window_1.handle);
}

#[test]
fn remove_windows_if_present_handles_empty_input() {
  let mut workspace = Workspace::new_test(PersistentWorkspaceId::new_test(1), &Monitor::mock_1());
  let window = Window::new_test(1, Rect::new(0, 0, 100, 100));
  workspace.windows.push(window.clone());
  workspace.minimised_windows.push((window.handle, false));

  workspace.remove_windows_if_present(&[]);

  assert_eq!(workspace.windows.len(), 1);
  assert_eq!(workspace.windows[0].handle, window.handle);
  assert_eq!(workspace.minimised_windows.len(), 1);
  assert_eq!(workspace.minimised_windows[0].0, window.handle);
}

#[test]
fn remove_windows_if_present_handles_empty_workspace() {
  let mut workspace = Workspace::new_test(PersistentWorkspaceId::new_test(1), &Monitor::mock_1());
  let window = Window::new_test(1, Rect::new(0, 0, 100, 100));

  workspace.remove_windows_if_present(&[window]);

  assert!(workspace.windows.is_empty());
  assert!(workspace.minimised_windows.is_empty());
}
