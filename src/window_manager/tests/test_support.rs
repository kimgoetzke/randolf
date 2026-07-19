use super::window_manager::WindowManager;
use crate::api::MockWindowsApi;
use crate::configuration_provider::{ConfigurationProvider, Layout};
use crate::utils::create_temp_directory;
use crate::workspace_manager::WorkspaceManager;
use std::sync::{Arc, Mutex};

impl WindowManager<MockWindowsApi> {
  /// Builds a manager with default configuration and isolated test state.
  pub(crate) fn default(api: MockWindowsApi) -> Self {
    Self {
      configuration_provider: Arc::new(Mutex::new(ConfigurationProvider::default())),
      placement: Default::default(),
      allow_moving_cursor_after_close_or_minimise: true,
      scrolling: Default::default(),
      spatial: Default::default(),
      workspace_manager: WorkspaceManager::default(),
      virtual_desktop_manager: None,
      windows_api: api,
    }
  }
}

/// Builds a test [`WindowManager`] whose default layout is scrolling.
pub(super) fn scrolling_manager() -> (WindowManager<MockWindowsApi>, tempfile::TempDir) {
  MockWindowsApi::reset();
  let directory = create_temp_directory();
  let workspace_manager = WorkspaceManager::new_test(true, directory.path().join("workspaces.toml"));
  let configuration_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
  configuration_provider.lock().unwrap().set_default_layout(Layout::Scrolling);
  let manager = WindowManager {
    configuration_provider,
    placement: Default::default(),
    allow_moving_cursor_after_close_or_minimise: true,
    scrolling: Default::default(),
    spatial: Default::default(),
    workspace_manager,
    virtual_desktop_manager: None,
    windows_api: MockWindowsApi,
  };
  (manager, directory)
}
