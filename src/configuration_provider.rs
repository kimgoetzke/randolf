use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub const WINDOW_MARGIN: &str = "window_margin";
pub const FILE_LOGGING_ENABLED: &str = "file_logging_enabled";
pub const ALLOW_SELECTING_SAME_CENTER_WINDOWS: &str = "allow_selecting_same_center_windows";
pub const ADDITIONAL_WORKSPACE_COUNT: &str = "additional_workspace_count";

const CONFIGURATION_FILE_NAME: &str = "randolf.toml";
const DEFAULT_WINDOW_MARGIN_VALUE: i32 = 20;

#[derive(Debug, Serialize, Deserialize, Default)]
struct Configuration {
  pub general: GeneralConfiguration,
  #[serde(default)]
  pub hotkey: Vec<CustomHotkey>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeneralConfiguration {
  window_margin: i32,
  file_logging_enabled: bool,
  allow_selecting_same_center_windows: bool,
  additional_workspace_count: i32,
}

impl Default for GeneralConfiguration {
  fn default() -> Self {
    Self {
      window_margin: DEFAULT_WINDOW_MARGIN_VALUE,
      file_logging_enabled: true,
      allow_selecting_same_center_windows: true,
      additional_workspace_count: 3,
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CustomHotkey {
  pub name: String,
  pub path: String,
  pub hotkey: String,
  pub execute_as_admin: bool,
}

pub struct ConfigurationProvider {
  config: Configuration,
  config_path: PathBuf,
}

impl ConfigurationProvider {
  pub fn new() -> Self {
    let config_path = Self::get_path_to_config().expect("Failed to determine configuration path");
    let config = Self::load_or_create_config(&config_path).expect("Failed to load configuration");

    Self { config, config_path }
  }

  pub fn log_current_config(&self) {
    debug!("{:?}", self.config);
  }

  /// Determines the appropriate path for the configuration file. First tries the executable directory, then falls back
  /// to the current directory.
  fn get_path_to_config() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Ok(exe_path) = std::env::current_exe() {
      info!("Using current executable path: {}", exe_path.display());
      if let Some(exe_dir) = exe_path.parent() {
        let config_path = exe_dir.join(CONFIGURATION_FILE_NAME);
        return Ok(config_path);
      }
    }

    info!("Using current directory path");
    let current_dir = std::env::current_dir()?;
    let config_path = current_dir.join(CONFIGURATION_FILE_NAME);

    Ok(config_path)
  }

  // TODO: Add missing configurations with default values when loading the configuration
  // TODO: Add validation e.g. desktop_container_count < x, paths for hotkeys, etc.
  /// Loads configuration from file or creates a default one if the file doesn't exist.
  fn load_or_create_config(config_path: &Path) -> Result<Configuration, Box<dyn std::error::Error>> {
    match fs::read_to_string(config_path) {
      Ok(contents) => {
        let config: Configuration = toml::from_str(&contents)?;

        Ok(config)
      }
      Err(error) => {
        if error.kind() == ErrorKind::NotFound {
          let default_config = Configuration::default();
          let toml_string = toml::to_string_pretty(&default_config)?;
          fs::write(config_path, toml_string)?;
          Ok(default_config)
        } else {
          error!("Failed to load configuration: {}", error);

          Err(Box::new(error))
        }
      }
    }
  }

  pub fn get_bool(&self, name: &str) -> bool {
    match name {
      FILE_LOGGING_ENABLED => self.config.general.file_logging_enabled,
      ALLOW_SELECTING_SAME_CENTER_WINDOWS => self.config.general.allow_selecting_same_center_windows,
      &_ => {
        warn!("Failed to get configuration because [{name}] is unknown");

        false
      }
    }
  }

  /// Sets bool value and saves the configuration to file.
  pub fn set_bool(&mut self, name: &str, value: bool) {
    match name {
      FILE_LOGGING_ENABLED => {
        if self.config.general.file_logging_enabled != value {
          self.config.general.file_logging_enabled = value;
          if let Err(err) = self.save_config() {
            error!("Failed to save configuration: {}", err);
          }
        }
      }
      ALLOW_SELECTING_SAME_CENTER_WINDOWS => {
        if self.config.general.allow_selecting_same_center_windows != value {
          self.config.general.allow_selecting_same_center_windows = value;
          if let Err(err) = self.save_config() {
            error!("Failed to save configuration: {}", err);
          }
        }
      }
      &_ => {
        warn!("Failed to save configuration because [{name}] is unknown");
      }
    }
  }

  pub fn get_i32(&self, name: &str) -> i32 {
    match name {
      WINDOW_MARGIN => self.config.general.window_margin,
      ADDITIONAL_WORKSPACE_COUNT => self.config.general.additional_workspace_count,
      &_ => {
        warn!("Failed to get configuration because [{name}] is unknown");

        0
      }
    }
  }

  #[allow(clippy::single_match)]
  pub fn set_i32(&mut self, name: &str, value: i32) {
    match name {
      WINDOW_MARGIN => {
        if self.config.general.window_margin != value {
          self.config.general.window_margin = value;
          if let Err(err) = self.save_config() {
            error!("Failed to save configuration: {}", err);
          }
        }
      }
      &_ => {
        warn!("Failed to save configuration because [{name}] is unknown");
      }
    }
  }

  pub fn get_hotkeys(&self) -> &Vec<CustomHotkey> {
    &self.config.hotkey
  }

  /// Saves the current configuration to file.
  fn save_config(&self) -> Result<(), Box<dyn std::error::Error>> {
    let toml_string = toml::to_string_pretty(&self.config)?;
    fs::write(&self.config_path, toml_string)?;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs::{self, File};
  use std::io::Write;
  use tempfile::TempDir;

  fn create_temp_directory() -> TempDir {
    TempDir::new().expect("Failed to create temporary directory")
  }

  #[test]
  fn load_or_create_config_creates_default_when_file_does_not_exist() {
    let directory = create_temp_directory();
    let path = directory.path().join(CONFIGURATION_FILE_NAME);

    let result = ConfigurationProvider::load_or_create_config(&path);

    assert!(result.is_ok(), "Should successfully create default config");
    let config = result.unwrap();
    assert_eq!(config.general.window_margin, DEFAULT_WINDOW_MARGIN_VALUE);
    assert_eq!(config.general.file_logging_enabled, true);
    assert_eq!(config.general.allow_selecting_same_center_windows, true);
    assert_eq!(config.general.additional_workspace_count, 3);
    assert!(config.hotkey.is_empty());

    assert!(path.exists(), "Config file should have been created");

    let raw_contents = fs::read_to_string(path).expect("Should read the config file");
    let parsed_contents: Configuration = toml::from_str(&raw_contents).expect("Should parse valid TOML");
    assert_eq!(parsed_contents.general.window_margin, DEFAULT_WINDOW_MARGIN_VALUE);
  }

  #[test]
  fn load_or_create_config_loads_existing_file() {
    let directory = create_temp_directory();
    let path = directory.path().join(CONFIGURATION_FILE_NAME);
    let custom_config = Configuration {
      general: GeneralConfiguration {
        window_margin: 50,
        file_logging_enabled: false,
        allow_selecting_same_center_windows: false,
        additional_workspace_count: 5,
      },
      hotkey: vec![CustomHotkey {
        name: "Test App".to_string(),
        path: "C:\\test.exe".to_string(),
        hotkey: "y".to_string(),
        execute_as_admin: true,
      }],
    };
    let toml_string = toml::to_string_pretty(&custom_config).expect("Failed to serialize config");
    fs::write(&path, toml_string).expect("Failed to write config file");

    let result = ConfigurationProvider::load_or_create_config(&path);

    assert!(result.is_ok(), "Should successfully load config");
    let loaded_config = result.unwrap();
    assert_eq!(loaded_config.general.window_margin, 50);
    assert_eq!(loaded_config.general.file_logging_enabled, false);
    assert_eq!(loaded_config.general.allow_selecting_same_center_windows, false);
    assert_eq!(loaded_config.general.additional_workspace_count, 5);
    assert_eq!(loaded_config.hotkey.len(), 1);
    assert_eq!(loaded_config.hotkey[0].name, "Test App");
    assert_eq!(loaded_config.hotkey[0].execute_as_admin, true);
  }

  #[test]
  fn load_or_create_config_handles_invalid_toml() {
    let directory = create_temp_directory();
    let path = directory.path().join(CONFIGURATION_FILE_NAME);
    let mut file = File::create(&path).expect("Failed to create test file");
    file.write_all(b"this is not valid TOML]").expect("Failed to write test data");

    let result = ConfigurationProvider::load_or_create_config(&path);

    assert!(result.is_err(), "Should fail with invalid TOML");
  }
}
