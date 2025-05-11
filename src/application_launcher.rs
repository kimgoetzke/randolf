use crate::api::WindowsApi;
use crate::configuration_provider::ConfigurationProvider;
use crate::files::{FileManager, FileType};
use std::process::Command;
use std::sync::{Arc, Mutex};

const FIXED_DELAY: u64 = 750;

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

  pub fn launch(&self, path_to_executable: String, args: Option<&str>, as_admin: bool) {
    if path_to_executable.is_empty() {
      warn!("Path to executable is empty");
      return;
    }
    if !path_to_executable.ends_with(".exe") {
      warn!("Path to executable is not a valid executable");
      return;
    }
    if self.execute_command(&path_to_executable, args, as_admin) {
      std::thread::sleep(std::time::Duration::from_millis(FIXED_DELAY));
      self.set_cursor_position();
    }
  }

  pub fn get_executable_path(&self) -> String {
    if let Ok(executable_path) = std::env::current_exe() {
      executable_path
        .to_str()
        .expect("Failed to convert Randolf's executable path to string")
        .to_string()
    } else {
      warn!("Failed to get Randolf's executable path");

      "".to_string()
    }
  }

  pub fn get_executable_folder(&self) -> String {
    if let Ok(executable_path) = std::env::current_exe() {
      let executable_directory = executable_path
        .parent()
        .expect("Failed to get parent directory")
        .to_str()
        .expect("Failed to convert directory path to string");
      if executable_directory.is_empty() {
        warn!("Path to Randolf folder is empty");
      }

      executable_directory.to_string()
    } else {
      warn!("Failed to get current executable path");

      "".to_string()
    }
  }

  pub fn get_project_folder(&self, file_type: FileType) -> String {
    FileManager::<String>::get_path_to_directory(file_type)
      .expect("Failed to get path to directory")
      .to_str()
      .expect("Failed to convert directory path to string")
      .to_string()
  }

  fn execute_command(&self, path_to_executable: &str, args: Option<&str>, as_admin: bool) -> bool {
    if as_admin {
      let mut powershell_args = vec!["-Command", "Start-Process", path_to_executable];
      if let Some(arg) = args {
        powershell_args.push("-ArgumentList");
        powershell_args.push(arg);
      }
      powershell_args.push("-Verb");
      powershell_args.push("RunAs");
      match Command::new("powershell").args(powershell_args).spawn() {
        Ok(_) => true,
        Err(err) => {
          warn!(
            "Failed to launch application [{path_to_executable}] with arg(s) [{:?}] as admin: {}",
            args, err
          );
          false
        }
      }
    } else {
      let mut command = Command::new(path_to_executable);
      if let Some(arg) = args {
        command.arg(arg);
      }
      match command.spawn() {
        Ok(_) => true,
        Err(err) => {
          warn!(
            "Failed to launch application [{path_to_executable}] with arg(s) [{:?}] because: {}",
            args, err
          );
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
  use crate::common::{Point, Sizing, WindowHandle};
  use crate::configuration_provider::ConfigurationProvider;
  use log::Level::Warn;

  #[test]
  fn launch_fails_silently() {
    testing_logger::setup();
    let cursor_position = Point::new(-1, -1);
    MockWindowsApi::set_cursor_position(cursor_position);
    let mock_api = MockWindowsApi;
    let configuration_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
    let launcher = ApplicationLauncher::new_initialised(configuration_provider.clone(), mock_api);

    launcher.launch("C:\\does\\not\\exist.exe".to_string(), Some("C:\\does\\not\\exist"), false);
    launcher.launch("not an executable".to_string(), None, false);
    launcher.launch("".to_string(), None, false);

    testing_logger::validate(|captured_logs| {
      assert_eq!(captured_logs.len(), 4);
      assert_eq!(
        captured_logs[1].body,
        "Failed to launch application [C:\\does\\not\\exist.exe] with arg(s) [Some(\"C:\\\\does\\\\not\\\\exist\")] because: The system cannot find the path specified. (os error 3)".to_string()
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

  #[test]
  fn get_executable_folder_returns_correct_path() {
    let config_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
    let launcher = ApplicationLauncher::new_initialised(config_provider, MockWindowsApi);

    let folder = launcher.get_executable_folder();

    assert!(folder.ends_with("randolf\\target\\debug\\deps"));
  }

  #[test]
  fn get_executable_path_returns_path_to_an_executable() {
    let config_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
    let launcher = ApplicationLauncher::new_initialised(config_provider, MockWindowsApi);

    let path = launcher.get_executable_path();

    assert!(path.ends_with(".exe"));
  }

  #[test]
  fn get_project_folder_returns_a_path() {
    let config_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
    let launcher = ApplicationLauncher::new_initialised(config_provider, MockWindowsApi);

    let folder = launcher.get_project_folder(FileType::Data);

    assert!(!folder.is_empty());
    assert!(folder.len() > 30);

    let folder = launcher.get_project_folder(FileType::Config);

    assert!(!folder.is_empty());
    assert!(folder.len() > 30);
  }
}
