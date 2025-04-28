use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub const WINDOW_MARGIN: &str = "window_margin";
pub const ALLOW_SELECTING_SAME_CENTER_WINDOWS: &str = "allow_selecting_same_center_windows";
pub const FORCE_USING_ADMIN_PRIVILEGES: &str = "force_using_admin_privileges";
pub const ADDITIONAL_WORKSPACE_COUNT: &str = "additional_workspace_count";

const CONFIGURATION_FILE_NAME: &str = "randolf.toml";
const DEFAULT_WINDOW_MARGIN_VALUE: i32 = 20;

#[derive(Debug, Serialize, Deserialize, Default)]
struct Configuration {
  pub general: GeneralConfiguration,
  #[serde(default)]
  pub hotkey: Vec<CustomHotkey>,
  #[serde(default)]
  pub exclusion_settings: ExclusionSettings,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeneralConfiguration {
  #[serde(default = "default_window_margin")]
  window_margin: i32,
  #[serde(default = "default_allow_selecting_same_center_windows")]
  allow_selecting_same_center_windows: bool,
  #[serde(default = "default_force_using_admin_privileges")]
  force_using_admin_privileges: bool,
  #[serde(default = "default_additional_workspace_count")]
  additional_workspace_count: i32,
}

fn default_window_margin() -> i32 {
  DEFAULT_WINDOW_MARGIN_VALUE
}

fn validate_window_margin(config_str: &str, configuration_provider: &mut ConfigurationProvider) {
  if !config_str.contains(WINDOW_MARGIN) {
    warn!(
      "[{}] was missing; adding it now with default value: {}",
      WINDOW_MARGIN, DEFAULT_WINDOW_MARGIN_VALUE
    );
    configuration_provider.set_i32(WINDOW_MARGIN, DEFAULT_WINDOW_MARGIN_VALUE);
  } else if configuration_provider.config.general.window_margin <= 0 {
    warn!(
      "[{}] is 0 or negative, setting to default value: {}",
      WINDOW_MARGIN, DEFAULT_WINDOW_MARGIN_VALUE
    );
    configuration_provider.set_i32(WINDOW_MARGIN, DEFAULT_WINDOW_MARGIN_VALUE);
  }
}

fn default_allow_selecting_same_center_windows() -> bool {
  true
}

fn validate_allow_selecting_same_center_windows(config_str: &str, configuration_provider: &mut ConfigurationProvider) {
  if !config_str.contains(ALLOW_SELECTING_SAME_CENTER_WINDOWS) {
    warn!(
      "[{}] was missing; adding it now with default value: {}",
      ALLOW_SELECTING_SAME_CENTER_WINDOWS,
      default_allow_selecting_same_center_windows()
    );
    configuration_provider.set_bool(
      ALLOW_SELECTING_SAME_CENTER_WINDOWS,
      default_allow_selecting_same_center_windows(),
    );
  }
}

fn default_force_using_admin_privileges() -> bool {
  false
}

fn validate_force_using_admin_privileges(config_str: &str, configuration_provider: &mut ConfigurationProvider) {
  if !config_str.contains(FORCE_USING_ADMIN_PRIVILEGES) {
    warn!(
      "[{}] was missing; adding it now with default value: {}",
      FORCE_USING_ADMIN_PRIVILEGES,
      default_force_using_admin_privileges()
    );
    configuration_provider.set_bool(FORCE_USING_ADMIN_PRIVILEGES, default_force_using_admin_privileges());
  }
}

fn default_additional_workspace_count() -> i32 {
  2
}

fn validate_workspace_count(config_str: &str, configuration_provider: &mut ConfigurationProvider) {
  if !config_str.contains(ADDITIONAL_WORKSPACE_COUNT) {
    warn!(
      "[{}] was missing; adding it now with default value: [{}]",
      ADDITIONAL_WORKSPACE_COUNT,
      default_additional_workspace_count()
    );
    configuration_provider.set_i32(ADDITIONAL_WORKSPACE_COUNT, default_additional_workspace_count());
  } else if configuration_provider.config.general.additional_workspace_count < 0 {
    warn!(
      "[{}] is negative, setting to default value: [{}]",
      ADDITIONAL_WORKSPACE_COUNT,
      default_additional_workspace_count()
    );
    configuration_provider.set_i32(ADDITIONAL_WORKSPACE_COUNT, default_additional_workspace_count());
  } else if configuration_provider.config.general.additional_workspace_count > 8 {
    warn!(
      "[{}] is larger than 8 which is not permitted, setting to 8",
      ADDITIONAL_WORKSPACE_COUNT,
    );
    configuration_provider.set_i32(ADDITIONAL_WORKSPACE_COUNT, 8);
  }
}

impl Default for GeneralConfiguration {
  fn default() -> Self {
    Self {
      window_margin: default_window_margin(),
      allow_selecting_same_center_windows: default_allow_selecting_same_center_windows(),
      force_using_admin_privileges: default_force_using_admin_privileges(),
      additional_workspace_count: default_additional_workspace_count(),
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

/// Settings for excluding certain windows from being managed by the application. This is useful for ignoring
/// system windows or other applications that should not be affected by this application at all i.e. they should not
/// be moved, selected, etc.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExclusionSettings {
  #[serde(default = "default_excluded_window_titles")]
  pub window_titles: Vec<String>,
  #[serde(default = "default_excluded_window_classes")]
  pub window_class_names: Vec<String>,
}

impl Default for ExclusionSettings {
  fn default() -> Self {
    Self {
      window_titles: default_excluded_window_titles(),
      window_class_names: default_excluded_window_classes(),
    }
  }
}

fn default_excluded_window_titles() -> Vec<String> {
  vec![
    "Program Manager".to_string(),
    "Windows Input Experience".to_string(),
    "".to_string(),
    "Windows Shell Experience Host".to_string(),
    "ZPToolBarParentWnd".to_string(),
  ]
}

fn validate_excluded_window_titles(config_str: &str, configuration_provider: &mut ConfigurationProvider) {
  if !config_str.contains("window_titles") {
    warn!(
      "[{}] was missing; saving it now with default value: {:#?}",
      "window_titles",
      default_excluded_window_titles()
    );
    if let Err(err) = configuration_provider.save_config() {
      error!("Failed to save configuration: {}", err);
    }
  }
}

fn default_excluded_window_classes() -> Vec<String> {
  vec![
    "Progman".to_string(),
    "WorkerW".to_string(),
    "Shell_TrayWnd".to_string(),
    "Shell_SecondaryTrayWnd".to_string(),
    "DV2ControlHost".to_string(),
  ]
}

fn validate_excluded_window_classes(config_str: &str, configuration_provider: &mut ConfigurationProvider) {
  if !config_str.contains("window_class_names") {
    warn!(
      "[{}] was missing; saving it now with default value: {:#?}",
      "window_class_names",
      default_excluded_window_classes()
    );
    if let Err(err) = configuration_provider.save_config() {
      error!("Failed to save configuration: {}", err);
    }
  }
}

pub struct ConfigurationProvider {
  config: Configuration,
  config_path: PathBuf,
  config_string: Option<String>,
}

impl ConfigurationProvider {
  pub fn new() -> Self {
    let config_path = Self::get_path_to_config().expect("Failed to determine configuration path");
    let (config, config_string) = Self::load_or_create_config(&config_path).expect("Failed to load configuration");
    let mut configuration_provider = ConfigurationProvider {
      config,
      config_path,
      config_string,
    };
    configuration_provider.validate_config();

    configuration_provider
  }

  pub fn log_current_config(&self) {
    info!("{:?}", self.config);
  }

  /// Determines the appropriate path for the configuration file. Tries the directory of the executable first, then
  /// falls back to the current directory.
  fn get_path_to_config() -> Result<PathBuf, Box<dyn Error>> {
    if let Ok(executable_path) = std::env::current_exe() {
      trace!(
        "Using current executable path to load configuration file: {}",
        executable_path.display()
      );
      if let Some(executable_directory) = executable_path.parent() {
        let config_path = executable_directory.join(CONFIGURATION_FILE_NAME);
        return Ok(config_path);
      }
    }

    let current_directory = std::env::current_dir()?;
    let config_path = current_directory.join(CONFIGURATION_FILE_NAME);
    trace!(
      "Using current directory path to load configuration file: {}",
      current_directory.display()
    );

    Ok(config_path)
  }

  /// Loads the configuration from the specified path. If the file does not exist, it creates a new one with default
  /// values.
  fn load_or_create_config(config_path: &Path) -> Result<(Configuration, Option<String>), Box<dyn Error>> {
    match fs::read_to_string(config_path) {
      Ok(config_string) => {
        let config: Configuration = match toml::from_str(&config_string) {
          Ok(config) => config,
          Err(error) => {
            error!("Failed to parse configuration: {}", error);
            return Err(Box::new(error));
          }
        };

        Ok((config, Some(config_string)))
      }
      Err(error) => {
        if error.kind() == ErrorKind::NotFound {
          info!(
            "Configuration file not found, writing default configuration to file: {}",
            config_path.display()
          );
          let default_config = Configuration::default();
          let toml_string = toml::to_string_pretty(&default_config)?;
          fs::write(config_path, toml_string)?;

          Ok((default_config, None))
        } else {
          error!("Failed to load configuration ({}): {}", error.kind(), error);

          Err(Box::new(error))
        }
      }
    }
  }

  // TODO: Consider validating hotkeys
  fn validate_config(&mut self) {
    if let Some(config_as_string) = self.config_string.clone() {
      validate_window_margin(&config_as_string, self);
      validate_allow_selecting_same_center_windows(&config_as_string, self);
      validate_force_using_admin_privileges(&config_as_string, self);
      validate_workspace_count(&config_as_string, self);
      validate_excluded_window_titles(&config_as_string, self);
      validate_excluded_window_classes(&config_as_string, self);
    } else {
      warn!("Failed to validate configuration: configuration string not available");
    }
  }

  pub fn get_bool(&self, name: &str) -> bool {
    match name {
      ALLOW_SELECTING_SAME_CENTER_WINDOWS => self.config.general.allow_selecting_same_center_windows,
      FORCE_USING_ADMIN_PRIVILEGES => self.config.general.force_using_admin_privileges,
      &_ => {
        warn!("Failed to get configuration because [{name}] is unknown");

        false
      }
    }
  }

  /// Sets bool value and saves the configuration to file.
  pub fn set_bool(&mut self, name: &str, value: bool) {
    match name {
      ALLOW_SELECTING_SAME_CENTER_WINDOWS => {
        self.config.general.allow_selecting_same_center_windows = value;
        if let Err(err) = self.save_config() {
          error!("Failed to save configuration: {}", err);
        }
      }
      FORCE_USING_ADMIN_PRIVILEGES => {
        self.config.general.force_using_admin_privileges = value;
        if let Err(err) = self.save_config() {
          error!("Failed to save configuration: {}", err);
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

  /// Sets i32 value and saves the configuration to file.
  pub fn set_i32(&mut self, name: &str, value: i32) {
    match name {
      WINDOW_MARGIN => {
        self.config.general.window_margin = value;
        if let Err(err) = self.save_config() {
          error!("Failed to save configuration: {}", err);
        }
      }
      ADDITIONAL_WORKSPACE_COUNT => {
        self.config.general.additional_workspace_count = value;
        if let Err(err) = self.save_config() {
          error!("Failed to save configuration: {}", err);
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

  pub fn get_exclusion_settings(&self) -> &ExclusionSettings {
    &self.config.exclusion_settings
  }

  pub fn reload_configuration(&mut self) {
    info!("Reloading configuration from file: {}", self.config_path.display());
    let (config, config_string) = Self::load_or_create_config(&self.config_path).expect("Failed to load configuration");
    self.config = config;
    self.config_string = config_string;
    self.validate_config();
  }

  fn save_config(&self) -> Result<(), Box<dyn Error>> {
    info!("Saving configuration to file: {}", self.config_path.display());
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

  impl ConfigurationProvider {
    pub fn default() -> Self {
      Self {
        config: Configuration::default(),
        config_path: PathBuf::new(),
        config_string: None,
      }
    }

    pub fn default_with_hotkeys(hotkeys: Vec<CustomHotkey>) -> Self {
      Self {
        config: Configuration {
          general: GeneralConfiguration::default(),
          hotkey: hotkeys,
          exclusion_settings: ExclusionSettings::default(),
        },
        config_path: PathBuf::new(),
        config_string: None,
      }
    }
  }

  fn create_temp_directory() -> TempDir {
    TempDir::new().expect("Failed to create temporary directory")
  }

  #[test]
  fn load_or_create_config_creates_default_when_file_does_not_exist() {
    let directory = create_temp_directory();
    let path = directory.path().join(CONFIGURATION_FILE_NAME);

    let result = ConfigurationProvider::load_or_create_config(&path);

    assert!(result.is_ok(), "Should successfully create default config");
    let config = result.unwrap().0;
    assert_eq!(config.general.window_margin, DEFAULT_WINDOW_MARGIN_VALUE);
    assert!(config.general.allow_selecting_same_center_windows);
    assert_eq!(config.general.additional_workspace_count, 2);
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
        allow_selecting_same_center_windows: false,
        force_using_admin_privileges: true,
        additional_workspace_count: 5,
      },
      hotkey: vec![CustomHotkey {
        name: "Test App".to_string(),
        path: "C:\\test.exe".to_string(),
        hotkey: "y".to_string(),
        execute_as_admin: true,
      }],
      exclusion_settings: ExclusionSettings::default(),
    };
    let toml_string = toml::to_string_pretty(&custom_config).expect("Failed to serialize config");
    fs::write(&path, toml_string).expect("Failed to write config file");

    let result = ConfigurationProvider::load_or_create_config(&path);

    assert!(result.is_ok(), "Should successfully load config");
    let loaded_config = result.unwrap().0;
    assert_eq!(loaded_config.general.window_margin, 50);
    assert!(!loaded_config.general.allow_selecting_same_center_windows);
    assert!(loaded_config.general.force_using_admin_privileges);
    assert_eq!(loaded_config.general.additional_workspace_count, 5);
    assert_eq!(loaded_config.hotkey.len(), 1);
    assert_eq!(loaded_config.hotkey[0].name, "Test App");
    assert!(loaded_config.hotkey[0].execute_as_admin);
    assert_eq!(
      loaded_config.exclusion_settings.window_titles,
      default_excluded_window_titles()
    );
    assert_eq!(
      loaded_config.exclusion_settings.window_class_names,
      default_excluded_window_classes()
    );
  }

  #[test]
  fn load_or_create_config_handles_invalid_toml() {
    let directory = create_temp_directory();
    let path = directory.path().join(CONFIGURATION_FILE_NAME);
    let mut file = File::create(&path).expect("Failed to create test file");
    file.write_all(b"this is not valid TOML]").expect("Failed to write test data");

    let result = ConfigurationProvider::load_or_create_config(&path);

    assert!(result.is_err(), "Should fail with invalid TOML");
    assert!(result.unwrap_err().to_string().contains("TOML parse error at line 1"));
  }

  #[test]
  fn load_or_create_config_loads_file_with_missing_fields() {
    let directory = create_temp_directory();
    let path = directory.path().join(CONFIGURATION_FILE_NAME);
    let toml_string = r#"
      [general]
      
      [[hotkey]]
      name = "Test App"
      path = "C:\\test.exe"
      hotkey = "y"
      execute_as_admin = true
      "#;
    fs::write(&path, toml_string).expect("Failed to write config file");

    let result = ConfigurationProvider::load_or_create_config(&path);

    assert!(result.is_ok(), "Should successfully load config");
    let loaded_config = result.unwrap().0;
    assert_eq!(loaded_config.general.window_margin, default_window_margin());
    assert_eq!(
      loaded_config.general.allow_selecting_same_center_windows,
      default_allow_selecting_same_center_windows(),
      "Should use default value for [default_allow_selecting_same_center_windows]"
    );
    assert_eq!(
      loaded_config.general.additional_workspace_count,
      default_additional_workspace_count(),
      "Should use default value for [default_additional_workspace_count]"
    );
    assert_eq!(loaded_config.hotkey.len(), 1);
    assert_eq!(loaded_config.hotkey[0].name, "Test App");
    assert_eq!(
      loaded_config.exclusion_settings.window_titles,
      default_excluded_window_titles(),
      "Should use default value for [default_excluded_window_titles]"
    );
    assert_eq!(
      loaded_config.exclusion_settings.window_class_names,
      default_excluded_window_classes(),
      "Should use default value for [default_excluded_window_classes]"
    );
  }

  #[test]
  fn validate_config_writes_missing_fields_to_file() {
    let directory = create_temp_directory();
    let path = directory.path().join(CONFIGURATION_FILE_NAME);
    let toml_string = r#"
      [general]
      allow_selecting_same_center_windows = true

      "#;
    fs::write(&path, toml_string).expect("Failed to write config file");
    let (config, config_string) = ConfigurationProvider::load_or_create_config(&path).expect("Failed to load config");
    let mut configuration_provider = ConfigurationProvider {
      config,
      config_path: path.clone(),
      config_string: config_string.clone(),
    };
    let window_margin = format!("{} = {}", WINDOW_MARGIN, DEFAULT_WINDOW_MARGIN_VALUE);
    let allow_selecting_same_center_windows = format!(
      "{} = {}",
      ALLOW_SELECTING_SAME_CENTER_WINDOWS,
      default_allow_selecting_same_center_windows()
    );
    let additional_workspace_count = format!("{} = {}", ADDITIONAL_WORKSPACE_COUNT, default_additional_workspace_count());

    // Prior to validation, the config string does not contain the missing fields
    let config_string = config_string.unwrap();
    assert!(!config_string.contains(window_margin.as_str()));
    assert!(config_string.contains(allow_selecting_same_center_windows.as_str()));
    assert!(!config_string.contains(additional_workspace_count.as_str()));

    // Validate the config
    configuration_provider.validate_config();
    let (_, config_string) = ConfigurationProvider::load_or_create_config(&path).expect("Failed to load config");

    // After validation, the missing fields were added to the config string
    let config_string = config_string.unwrap();
    assert!(config_string.contains(window_margin.as_str()));
    assert!(config_string.contains(allow_selecting_same_center_windows.as_str()));
    assert!(config_string.contains(additional_workspace_count.as_str()));
  }

  #[test]
  fn validate_config_updates_window_margin_if_negative_value_loaded() {
    let directory = create_temp_directory();
    let path = directory.path().join(CONFIGURATION_FILE_NAME);
    let toml_string = r#"
      [general]
      window_margin = -10
      "#;
    fs::write(&path, toml_string).expect("Failed to write config file");
    let (config, config_string) = ConfigurationProvider::load_or_create_config(&path).expect("Failed to load config");
    let mut configuration_provider = ConfigurationProvider {
      config,
      config_path: path,
      config_string: config_string.clone(),
    };

    configuration_provider.validate_config();

    assert!(config_string.unwrap().contains("window_margin = -10"));
    assert_eq!(
      configuration_provider.config.general.window_margin,
      DEFAULT_WINDOW_MARGIN_VALUE
    );
  }

  #[test]
  fn validate_config_updates_additional_workspace_count_if_loaded_value_exceeds_max() {
    let directory = create_temp_directory();
    let path = directory.path().join(CONFIGURATION_FILE_NAME);
    let toml_string = r#"
      [general]
      additional_workspace_count = 15
      "#;
    fs::write(&path, toml_string).expect("Failed to write config file");
    let (config, config_string) = ConfigurationProvider::load_or_create_config(&path).expect("Failed to load config");
    let mut configuration_provider = ConfigurationProvider {
      config,
      config_path: path,
      config_string: config_string.clone(),
    };

    configuration_provider.validate_config();

    assert!(config_string.unwrap().contains("additional_workspace_count = 15"));
    assert_eq!(configuration_provider.config.general.additional_workspace_count, 8);
  }

  #[test]
  fn reload_configuration_replaces_prior_settings() {
    let directory = create_temp_directory();
    let path = directory.path().join(CONFIGURATION_FILE_NAME);
    let custom_config = Configuration {
      general: GeneralConfiguration {
        window_margin: 50,
        allow_selecting_same_center_windows: false,
        force_using_admin_privileges: false,
        additional_workspace_count: 2,
      },
      hotkey: vec![],
      exclusion_settings: ExclusionSettings::default(),
    };
    let toml_string = toml::to_string_pretty(&custom_config).expect("Failed to serialize config");
    fs::write(&path, toml_string).expect("Failed to write config file");
    let (config, config_string) = ConfigurationProvider::load_or_create_config(&path).expect("Failed to load config");
    let mut configuration_provider = ConfigurationProvider {
      config,
      config_path: path,
      config_string: config_string.clone(),
    };

    let new_config = Configuration {
      general: GeneralConfiguration {
        window_margin: 100,
        allow_selecting_same_center_windows: true,
        force_using_admin_privileges: true,
        additional_workspace_count: 8,
      },
      hotkey: vec![CustomHotkey {
        name: "Test App".to_string(),
        path: "C:\\test.exe".to_string(),
        hotkey: "y".to_string(),
        execute_as_admin: true,
      }],
      exclusion_settings: ExclusionSettings::default(),
    };
    let new_toml_string = toml::to_string_pretty(&new_config).expect("Failed to serialize new config");
    fs::write(&configuration_provider.config_path, new_toml_string).expect("Failed to write new config file");

    configuration_provider.reload_configuration();

    assert_eq!(configuration_provider.config.general.window_margin, 100);
    assert!(configuration_provider.config.general.allow_selecting_same_center_windows);
    assert!(configuration_provider.config.general.force_using_admin_privileges);
    assert_eq!(configuration_provider.config.general.additional_workspace_count, 8);
    assert_eq!(configuration_provider.config.hotkey.len(), 1);
    assert_eq!(configuration_provider.config.hotkey[0].name, "Test App");
    assert!(configuration_provider.config.hotkey[0].execute_as_admin);
  }
}
