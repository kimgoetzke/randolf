use crate::api::MockWindowsApi;
use crate::common::{MonitorHandle, Point, Rect};
use crate::utils::create_temp_directory;
use crate::window::workspace::WorkspaceManager;
use crate::window::workspace::workspace_guard::WorkspaceGuard;

const TEST_WORKSPACE_FILE: &str = "test.toml";

#[test]
fn get_active_workspace_for_cursor_position_returns_workspace_if_one_active_workspace_found() {
  let directory = create_temp_directory();
  let path = directory.path().join(TEST_WORKSPACE_FILE);
  let mut workspace_manager = WorkspaceManager::new_test(true, path.clone());
  let mut guard = WorkspaceGuard::new(&mut workspace_manager);
  // Cursor on primary monitor
  MockWindowsApi::set_cursor_position(Point::new(50, 50));

  let result = guard.get_active_workspace_for_cursor_position();

  assert_eq!(
    result,
    Some((*crate::window::workspace::tests::primary_active_ws_id()).into())
  );
}

#[test]
fn get_active_workspace_for_cursor_position_returns_none_when_no_matches() {
  let mut workspace_manager = WorkspaceManager::default();
  let mut guard = WorkspaceGuard::new(&mut workspace_manager);
  MockWindowsApi::set_cursor_position(Point::new(100, 100));
  MockWindowsApi::add_monitor(MonitorHandle::from(5), Rect::new(0, 0, 200, 200), true);

  let result = guard.get_active_workspace_for_cursor_position();

  assert!(result.is_none());
}
