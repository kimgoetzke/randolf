use crate::api::NativeApi;
use crate::configuration_provider::ConfigurationProvider;
use std::process::Command;
use std::sync::{Arc, Mutex};

pub struct ApplicationLauncher<T: NativeApi> {
  _configuration_provider: Arc<Mutex<ConfigurationProvider>>,
  windows_api: T,
}

impl<T: NativeApi> ApplicationLauncher<T> {
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
  use crate::utils::{Point, Rect, Window, WindowHandle, WindowPlacement};
  use log::Level::Warn;

  #[test]
  fn launch_fails_silently() {
    testing_logger::setup();
    let cursor_position = Point::new(-1, -1);
    MockWindowsApi::set_cursor_position(cursor_position);
    let api = MockWindowsApi;
    let configuration_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
    let launcher = ApplicationLauncher::new_initialised(configuration_provider.clone(), api);

    launcher.launch("C:\\does\\not\\exist.exe".to_string(), false);
    launcher.launch("not an executable".to_string(), false);
    launcher.launch("".to_string(), false);

    testing_logger::validate(|captured_logs| {
      assert_eq!(captured_logs.len(), 3);
      assert_eq!(
        captured_logs[0].body,
        "Failed to launch application: C:\\does\\not\\exist.exe".to_string()
      );
      assert_eq!(captured_logs[0].level, Warn);
      assert_eq!(
        captured_logs[1].body,
        "Path to executable is not a valid executable".to_string()
      );
      assert_eq!(captured_logs[1].level, Warn);
      assert_eq!(captured_logs[2].body, "Path to executable is empty".to_string());
      assert_eq!(captured_logs[2].level, Warn);
    });
    assert_eq!(api.get_cursor_position(), cursor_position);
  }

  #[test]
  fn set_cursor_position_does_nothing_when_foreground_window_is_none() {
    let cursor_position = Point::new(-1, -1);
    MockWindowsApi::set_cursor_position(cursor_position);
    let api = MockWindowsApi;
    let config_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
    let launcher = ApplicationLauncher::new_initialised(config_provider, api);

    launcher.set_cursor_position();

    assert_eq!(api.get_cursor_position(), cursor_position);
  }

  #[test]
  fn set_cursor_position_does_nothing_when_window_placement_is_none() {
    let cursor_position = Point::new(-1, -1);
    let foreground_window_handle = WindowHandle::new(1);
    MockWindowsApi::set_cursor_position(cursor_position);
    MockWindowsApi::set_foreground_window(foreground_window_handle);
    let api = MockWindowsApi;
    let config_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
    let launcher = ApplicationLauncher::new_initialised(config_provider, api);

    launcher.set_cursor_position();

    assert_eq!(api.get_cursor_position(), cursor_position);
  }

  #[test]
  fn set_cursor_position_sets_correct_position_when_valid() {
    let foreground_window_handle = WindowHandle::new(1);
    let foreground_window_placement = WindowPlacement::new_from_rect(Rect::new(50, 50, 100, 100));
    let foreground_window = Window::new(
      foreground_window_handle.as_hwnd(),
      "Test Window".to_string(),
      foreground_window_placement.normal_position,
    );
    MockWindowsApi::set_foreground_window(foreground_window_handle);
    MockWindowsApi::set_window_placement(foreground_window_handle, foreground_window_placement);
    MockWindowsApi::set_window_title(foreground_window.title.to_string());
    MockWindowsApi::set_visible_windows(vec![foreground_window]);
    let api = MockWindowsApi;
    let config_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
    let launcher = ApplicationLauncher::new_initialised(config_provider, api);

    launcher.set_cursor_position();

    assert_eq!(api.get_cursor_position(), Point::new(75, 75));
  }
}
