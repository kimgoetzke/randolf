use crate::api::WindowsApi;
use crate::configuration_provider::ConfigurationProvider;
use std::process::Command;
use std::sync::{Arc, Mutex};

pub struct ApplicationLauncher<T: WindowsApi> {
  _configuration_provider: Arc<Mutex<ConfigurationProvider>>,
  windows_api: T,
}

impl<T: WindowsApi> ApplicationLauncher<T> {
  pub fn new_initialised(configuration_provider: Arc<Mutex<ConfigurationProvider>>, windows_api: T) -> Self {
    Self {
      _configuration_provider: configuration_provider.clone(),
      windows_api,
    }
  }

  pub fn launch(&self, path_to_executable: String, as_admin: bool) {
    if path_to_executable.is_empty() {
      warn!("Path to executable is empty");
      return;
    }
    if !path_to_executable.ends_with(".exe") {
      warn!("Path to executable is not a valid executable");
      return;
    }
    if self.execute_command(&path_to_executable, as_admin) {
      std::thread::sleep(std::time::Duration::from_millis(750));
      self.set_cursor_position();
    }
  }

  fn execute_command(&self, path_to_executable: &String, as_admin: bool) -> bool {
    if as_admin {
      match Command::new("powershell")
        .args(["-Command", "Start-Process", path_to_executable, "-Verb", "RunAs"])
        .spawn()
      {
        Ok(_) => true,
        Err(err) => {
          warn!("Failed to launch application as admin: {}", err);

          false
        }
      }
    } else {
      match Command::new(path_to_executable).spawn() {
        Ok(_) => true,
        Err(_) => {
          warn!("Failed to launch application: {}", path_to_executable);

          false
        }
      }
    }
  }

  fn set_cursor_position(&self) {
    let Some(foreground_window) = self.windows_api.get_foreground_window() else {
      debug!("Failed to get foreground window, no window to set cursor position");
      return;
    };
    let Some(placement) = self.windows_api.get_window_placement(foreground_window) else {
      debug!("Failed to get window placement, no window to set cursor position");
      return;
    };
    let center_point = placement.normal_position.center();
    self.windows_api.set_cursor_position(&center_point);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api::MockWindowsApi;
  use crate::configuration_provider::ConfigurationProvider;
  use crate::utils::{Point, Sizing, WindowHandle};
  use log::Level::Warn;

  #[test]
  fn launch_fails_silently() {
    testing_logger::setup();
    let cursor_position = Point::new(-1, -1);
    MockWindowsApi::set_cursor_position(cursor_position);
    let mock_api = MockWindowsApi;
    let configuration_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
    let launcher = ApplicationLauncher::new_initialised(configuration_provider.clone(), mock_api);

    launcher.launch("C:\\does\\not\\exist.exe".to_string(), false);
    launcher.launch("not an executable".to_string(), false);
    launcher.launch("".to_string(), false);

    testing_logger::validate(|captured_logs| {
      assert_eq!(captured_logs.len(), 4);
      assert_eq!(
        captured_logs[1].body,
        "Failed to launch application: C:\\does\\not\\exist.exe".to_string()
      );
      assert_eq!(captured_logs[1].level, Warn);
      assert_eq!(
        captured_logs[2].body,
        "Path to executable is not a valid executable".to_string()
      );
      assert_eq!(captured_logs[2].level, Warn);
      assert_eq!(captured_logs[3].body, "Path to executable is empty".to_string());
      assert_eq!(captured_logs[3].level, Warn);
    });
    assert_eq!(mock_api.get_cursor_position(), cursor_position);
  }

  #[test]
  fn set_cursor_position_does_nothing_when_foreground_window_is_none() {
    let cursor_position = Point::new(-1, -1);
    MockWindowsApi::set_cursor_position(cursor_position);
    let mock_api = MockWindowsApi;
    let config_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
    let launcher = ApplicationLauncher::new_initialised(config_provider, mock_api);

    launcher.set_cursor_position();

    assert_eq!(mock_api.get_cursor_position(), cursor_position);
  }

  #[test]
  fn set_cursor_position_does_nothing_when_window_placement_is_none() {
    let cursor_position = Point::new(-1, -1);
    let foreground_window_handle = WindowHandle::new(1);
    MockWindowsApi::set_cursor_position(cursor_position);
    MockWindowsApi::set_foreground_window(foreground_window_handle);
    let mock_api = MockWindowsApi;
    let config_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
    let launcher = ApplicationLauncher::new_initialised(config_provider, mock_api);

    launcher.set_cursor_position();

    assert_eq!(mock_api.get_cursor_position(), cursor_position);
  }

  #[test]
  fn set_cursor_position_sets_correct_position_when_valid() {
    let handle = WindowHandle::new(1);
    let sizing = Sizing::new(50, 50, 50, 50);
    MockWindowsApi::add_or_update_window(handle, "Test Window".to_string(), sizing, false, false, true);
    let mock_api = MockWindowsApi;
    let config_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
    let launcher = ApplicationLauncher::new_initialised(config_provider, mock_api);

    launcher.set_cursor_position();

    assert_eq!(mock_api.get_cursor_position(), Point::new(75, 75));
  }
}
