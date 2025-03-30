use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub const WINDOW_MARGIN: &str = "window_margin";
pub const FILE_LOGGING_ENABLED: &str = "file_logging_enabled";

const CONFIGURATION_FILE_NAME: &str = "randolf.toml";
const DEFAULT_WINDOW_MARGIN_VALUE: i32 = 20;

#[derive(Debug, Serialize, Deserialize)]
struct Configuration {
  file_logging_enabled: bool,
  default_margin: i32,
}

impl Default for Configuration {
  fn default() -> Self {
    Self {
      file_logging_enabled: true,
      default_margin: DEFAULT_WINDOW_MARGIN_VALUE,
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
    info!("{:?}", self.config);
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
      &_ => false,
    }
  }

  #[allow(clippy::single_match)]
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
      &_ => {}
    }
  }

  pub fn get_i32(&self, name: &str) -> i32 {
    match name {
      WINDOW_MARGIN => self.config.default_margin,
      &_ => 0,
    }
  }

  #[allow(clippy::single_match)]
  pub fn set_i32(&mut self, name: &str, value: i32) {
    match name {
      WINDOW_MARGIN => {
        if self.config.default_margin != value {
          self.config.default_margin = value;
          if let Err(err) = self.save_config() {
            error!("Failed to save configuration: {}", err);
          }
        }
      }
      &_ => {}
    }
  }

  /// Saves the current configuration to file.
  fn save_config(&self) -> Result<(), Box<dyn std::error::Error>> {
    let toml_string = toml::to_string_pretty(&self.config)?;
    fs::write(&self.config_path, toml_string)?;
    Ok(())
  }
}
