use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub const WINDOW_MARGIN: &str = "window_margin";
pub const FILE_LOGGING_ENABLED: &str = "file_logging_enabled";
pub const ALLOW_SELECTING_SAME_CENTER_WINDOWS: &str = "allow_selecting_same_center_windows";
pub const DEFAULT_TERMINAL: &str = "default_terminal";
pub const DEFAULT_BROWSER: &str = "default_browser";
pub const DEFAULT_FILE_MANAGER: &str = "default_file_manager";

const CONFIGURATION_FILE_NAME: &str = "randolf.toml";
const DEFAULT_WINDOW_MARGIN_VALUE: i32 = 20;

#[derive(Debug, Serialize, Deserialize)]
struct Configuration {
  window_margin: i32,
  file_logging_enabled: bool,
  allow_selecting_same_center_windows: bool,
  default_terminal: Option<String>,
  default_browser: Option<String>,
  default_file_manager: Option<String>,
}

impl Default for Configuration {
  fn default() -> Self {
    Self {
      window_margin: DEFAULT_WINDOW_MARGIN_VALUE,
      file_logging_enabled: true,
      allow_selecting_same_center_windows: true,
      default_terminal: None,
      default_browser: None,
      default_file_manager: None,
    }
  }
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
      FILE_LOGGING_ENABLED => self.config.file_logging_enabled,
      ALLOW_SELECTING_SAME_CENTER_WINDOWS => self.config.allow_selecting_same_center_windows,
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
        if self.config.file_logging_enabled != value {
          self.config.file_logging_enabled = value;
          if let Err(err) = self.save_config() {
            error!("Failed to save configuration: {}", err);
          }
        }
      }
      ALLOW_SELECTING_SAME_CENTER_WINDOWS => {
        if self.config.allow_selecting_same_center_windows != value {
          self.config.allow_selecting_same_center_windows = value;
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
      WINDOW_MARGIN => self.config.window_margin,
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
        if self.config.window_margin != value {
          self.config.window_margin = value;
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

  pub fn get_str(&self, name: &str) -> Option<String> {
    match name {
      DEFAULT_BROWSER => self.config.default_browser.clone(),
      DEFAULT_TERMINAL => self.config.default_terminal.clone(),
      DEFAULT_FILE_MANAGER => self.config.default_file_manager.clone(),
      &_ => {
        warn!("Failed to get configuration because [{name}] is unknown");

        None
      }
    }
  }

  /// Saves the current configuration to file.
  fn save_config(&self) -> Result<(), Box<dyn std::error::Error>> {
    let toml_string = toml::to_string_pretty(&self.config)?;
    fs::write(&self.config_path, toml_string)?;
    Ok(())
  }
}
