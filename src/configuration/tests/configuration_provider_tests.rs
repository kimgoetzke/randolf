use super::configuration_provider::*;
use crate::files::FileManager;
use crate::utils::create_temp_directory;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

impl ConfigurationProvider {
  pub fn default() -> Self {
    Self {
      file_manager: FileManager::default(),
      config: Configuration::default(),
    }
  }

  pub fn default_with_hotkeys(hotkeys: Vec<CustomHotkey>) -> Self {
    Self {
      file_manager: FileManager::default(),
      config: Configuration {
        general: GeneralConfiguration::default(),
        hotkey: hotkeys,
        exclusion_settings: ExclusionSettings::default(),
        ..Configuration::default()
      },
    }
  }

  pub fn new_test(temp_path: PathBuf) -> Self {
    let file_manager = FileManager::new_test(temp_path);
    Self::new_with(file_manager)
  }

  fn new_test_without_validation(temp_path: PathBuf, config: Configuration) -> Self {
    let file_manager = FileManager::new_test(temp_path);
    Self { file_manager, config }
  }

  /// Adds a monitor override without saving it.
  pub fn set_monitor_layout(&mut self, id: &str, layout: Layout) {
    self.config.layout.monitor.push(MonitorLayoutConfiguration {
      id: id.to_string(),
      mode: layout,
    });
  }
}

#[test]
fn scrolling_layout_loads_animation_and_reconciliation_durations() {
  let directory = create_temp_directory();
  let path = directory.path().join(CONFIGURATION_FILE_NAME);
  fs::write(
    &path,
    r#"
        [general]
        [layout]
        [spatial_layout]
        [scrolling_layout]
        animation_duration_in_ms = 75
        reconciliation_interval_in_ms = 400
        [exclusion_settings]
      "#,
  )
  .expect("Failed to write config file");
  let configuration_provider = ConfigurationProvider::new_test(path);

  assert_eq!(configuration_provider.get_i32(SCROLLING_ANIMATION_DURATION_IN_MS), 75);
  assert_eq!(configuration_provider.get_i32(SCROLLING_RECONCILIATION_INTERVAL_IN_MS), 400);
}

#[test]
fn scrolling_layout_replaces_negative_durations_with_defaults() {
  let directory = create_temp_directory();
  let path = directory.path().join(CONFIGURATION_FILE_NAME);
  fs::write(
    &path,
    r#"
        [general]
        [layout]
        [spatial_layout]
        [scrolling_layout]
        animation_duration_in_ms = -1
        reconciliation_interval_in_ms = -1
        [exclusion_settings]
      "#,
  )
  .expect("Failed to write config file");

  let configuration_provider = ConfigurationProvider::new_test(path);

  assert_eq!(
    configuration_provider.get_i32(SCROLLING_ANIMATION_DURATION_IN_MS),
    DEFAULT_SCROLLING_ANIMATION_DURATION_IN_MS
  );
  assert_eq!(
    configuration_provider.get_i32(SCROLLING_RECONCILIATION_INTERVAL_IN_MS),
    DEFAULT_SCROLLING_RECONCILIATION_INTERVAL_IN_MS
  );
}

#[test]
fn layout_defaults_to_spatial() {
  let configuration_provider = ConfigurationProvider::default();

  assert_eq!(configuration_provider.layout_for_monitor("DISPLAY1", true), Layout::Spatial);
}

#[test]
fn get_default_layout_returns_configured_default() {
  let mut configuration_provider = ConfigurationProvider::default();
  configuration_provider.set_default_layout(Layout::Scrolling);

  assert_eq!(configuration_provider.get_default_layout(), Layout::Scrolling);
}

#[test]
fn set_default_layout_persists_without_changing_monitor_overrides() {
  let directory = create_temp_directory();
  let path = directory.path().join(CONFIGURATION_FILE_NAME);
  fs::write(
    &path,
    r#"
        [general]
        [layout]
        default = "spatial"

        [[layout.monitor]]
        id = "DISPLAY1"
        mode = "spatial"

        [spatial_layout]
        [scrolling_layout]
        [exclusion_settings]
      "#,
  )
  .expect("Failed to write config file");
  let mut configuration_provider = ConfigurationProvider::new_test(path.clone());

  configuration_provider.set_default_layout(Layout::Scrolling);

  let reloaded = ConfigurationProvider::new_test(path);
  assert_eq!(reloaded.get_default_layout(), Layout::Scrolling);
  assert_eq!(reloaded.layout_for_monitor("DISPLAY1", false), Layout::Spatial);
  assert_eq!(reloaded.layout_for_monitor("DISPLAY2", false), Layout::Scrolling);
}

#[test]
fn repairs_generated_empty_monitor_list_before_loading_monitor_override() {
  let directory = create_temp_directory();
  let path = directory.path().join(CONFIGURATION_FILE_NAME);
  fs::write(
    &path,
    r#"
        [general]
        [layout]
        default = "scrolling"
        monitor = []

        [[layout.monitor]]
        id = "primary"
        mode = "spatial"
      "#,
  )
  .expect("Failed to write config file");

  let configuration_provider = ConfigurationProvider::new_test(path.clone());

  assert_eq!(configuration_provider.layout_for_monitor("DISPLAY1", true), Layout::Spatial);
  assert!(!fs::read_to_string(path).unwrap().contains("monitor = []"));
}

#[test]
fn exact_monitor_override_precedes_primary_then_falls_back_to_default() {
  let directory = create_temp_directory();
  let path = directory.path().join(CONFIGURATION_FILE_NAME);
  fs::write(
    &path,
    r#"
        [general]

        [layout]
        default = "scrolling"

        [[layout.monitor]]
        id = "primary"
        mode = "spatial"

        [[layout.monitor]]
        id = "DISPLAY1"
        mode = "scrolling"

        [spatial_layout]
        allow_selecting_same_center_windows = false

        [scrolling_layout]

        [exclusion_settings]
      "#,
  )
  .expect("Failed to write config file");
  let configuration_provider = ConfigurationProvider::new_test(path);

  assert_eq!(configuration_provider.layout_for_monitor("DISPLAY1", true), Layout::Scrolling);
  assert_eq!(configuration_provider.layout_for_monitor("DISPLAY2", true), Layout::Spatial);
  assert_eq!(
    configuration_provider.layout_for_monitor("DISPLAY3", false),
    Layout::Scrolling
  );
  assert!(!configuration_provider.get_bool(ALLOW_SELECTING_SAME_CENTER_WINDOWS));
}

#[test]
fn new_with_file_manager_creates_default_when_file_does_not_exist() {
  let directory = create_temp_directory();
  let path = directory.path().join(CONFIGURATION_FILE_NAME);
  let configuration_provider = ConfigurationProvider::new_test(path.clone());

  let config = configuration_provider.config;
  assert_eq!(config.general.window_margin, DEFAULT_WINDOW_MARGIN_VALUE);
  assert!(config.spatial_layout.allow_selecting_same_center_windows);
  assert_eq!(config.layout.default, Layout::Spatial);
  assert_eq!(config.general.additional_workspace_count, 2);
  assert!(config.hotkey.is_empty());
  assert!(path.exists(), "Config file should have been created");
  let raw_contents = fs::read_to_string(path).expect("Should read the config file");
  assert!(!raw_contents.contains("monitor = []"));
  assert!(raw_contents.contains("animation_duration_in_ms = 120"));
  assert!(raw_contents.contains("reconciliation_interval_in_ms = 250"));
  let parsed_contents: Configuration = toml::from_str(&raw_contents).expect("Should parse valid TOML");
  assert_eq!(parsed_contents.general.window_margin, DEFAULT_WINDOW_MARGIN_VALUE);
}

#[test]
fn new_with_file_manager_loads_existing_file() {
  let directory = create_temp_directory();
  let path = directory.path().join(CONFIGURATION_FILE_NAME);
  let custom_config = Configuration {
    general: GeneralConfiguration {
      window_margin: 50,
      force_using_admin_privileges: true,
      additional_workspace_count: 5,
      enable_features_using_mouse: true,
      delay_in_ms_before_dragging_is_allowed: 1000,
      allow_moving_cursor_after_open_close_or_minimise: false,
    },
    layout: LayoutConfiguration {
      default: Layout::Scrolling,
      monitor: vec![],
    },
    spatial_layout: SpatialLayoutConfiguration {
      allow_selecting_same_center_windows: false,
    },
    scrolling_layout: ScrollingLayoutConfiguration::default(),
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

  let configuration_provider = ConfigurationProvider::new_test(path);

  let loaded_config = configuration_provider.config;
  assert_eq!(loaded_config.general.window_margin, 50);
  assert!(!loaded_config.spatial_layout.allow_selecting_same_center_windows);
  assert!(loaded_config.general.force_using_admin_privileges);
  assert_eq!(loaded_config.general.additional_workspace_count, 5);
  assert!(loaded_config.general.enable_features_using_mouse);
  assert_eq!(loaded_config.general.delay_in_ms_before_dragging_is_allowed, 1000);
  assert_eq!(loaded_config.layout.default, Layout::Scrolling);
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
#[should_panic(expected = "Failed to load configuration")]
fn new_with_file_manager_rejects_invalid_layout_mode() {
  let directory = create_temp_directory();
  let path = directory.path().join(CONFIGURATION_FILE_NAME);
  fs::write(
    &path,
    r#"
        [general]
        [layout]
        default = "columns"
      "#,
  )
  .expect("Failed to write config file");

  ConfigurationProvider::new_test(path);
}

#[test]
#[should_panic(expected = "Failed to load configuration")]
fn new_with_file_manager_prevents_startup_when_invalid_toml_configuration() {
  let directory = create_temp_directory();
  let path = directory.path().join(CONFIGURATION_FILE_NAME);
  let mut file = File::create(&path).expect("Failed to create test file");
  file.write_all(b"this is not valid TOML]").expect("Failed to write test data");

  ConfigurationProvider::new_test(path);
}

#[test]
fn new_with_file_manager_loads_file_with_missing_fields() {
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
  let configuration_provider = ConfigurationProvider::new_test(path);

  let loaded_config = configuration_provider.config;
  assert_eq!(loaded_config.general.window_margin, default_window_margin());
  assert_eq!(
    loaded_config.spatial_layout.allow_selecting_same_center_windows,
    default_allow_selecting_same_center_windows(),
    "Should use default value for [default_allow_selecting_same_center_windows]"
  );
  assert_eq!(
    loaded_config.general.additional_workspace_count,
    default_additional_workspace_count(),
    "Should use default value for [default_additional_workspace_count]"
  );
  assert_eq!(loaded_config.layout.default, Layout::Spatial);
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
  let config_string = r#"
      [general]
  
      "#;
  fs::write(&path, config_string).expect("Failed to write config file");
  let mut configuration_provider =
    ConfigurationProvider::new_test_without_validation(path.clone(), Configuration::default());

  // Prepare expected values
  let window_margin = format!("{} = {}", WINDOW_MARGIN, DEFAULT_WINDOW_MARGIN_VALUE);
  let allow_selecting_same_center_windows = format!(
    "{} = {}",
    ALLOW_SELECTING_SAME_CENTER_WINDOWS,
    default_allow_selecting_same_center_windows()
  );
  let additional_workspace_count = format!("{} = {}", ADDITIONAL_WORKSPACE_COUNT, default_additional_workspace_count());

  // Validate the config
  configuration_provider.validate_config(Some(config_string.into()));

  // After validation, the missing fields were added to the config string
  let config_string = fs::read_to_string(path).expect("Failed to read config file");
  assert!(config_string.contains(window_margin.as_str()));
  assert!(config_string.contains(allow_selecting_same_center_windows.as_str()));
  assert!(config_string.contains(additional_workspace_count.as_str()));
  assert!(config_string.contains("[layout]"));
  assert!(config_string.contains("default = \"spatial\""));
  assert!(config_string.contains("[spatial_layout]"));
  assert!(config_string.contains("[scrolling_layout]"));
  assert!(config_string.contains("animation_duration_in_ms = 120"));
  assert!(config_string.contains("reconciliation_interval_in_ms = 250"));
}

#[test]
fn validate_config_updates_window_margin_if_negative_value_loaded() {
  let directory = create_temp_directory();
  let path = directory.path().join(CONFIGURATION_FILE_NAME);
  let config_string = r#"
      [general]
      window_margin = -10
      "#;
  fs::write(&path, config_string).expect("Failed to write config file");
  let mut configuration_provider =
    ConfigurationProvider::new_test_without_validation(path.clone(), Configuration::default());

  configuration_provider.validate_config(Some(config_string.into()));

  let config_string = fs::read_to_string(path).expect("Failed to read config file");
  assert!(config_string.contains("window_margin = 20"));
}

#[test]
fn validate_config_preserves_window_margin_if_zero_value_loaded() {
  let directory = create_temp_directory();
  let path = directory.path().join(CONFIGURATION_FILE_NAME);
  let config_string = r#"
      [general]
      window_margin = 0
      "#;
  fs::write(&path, config_string).expect("Failed to write config file");
  let mut config = Configuration::default();
  config.general.window_margin = 0;
  let mut configuration_provider = ConfigurationProvider::new_test_without_validation(path.clone(), config);

  configuration_provider.validate_config(Some(config_string.into()));

  let config_string = fs::read_to_string(path).expect("Failed to read config file");
  assert!(config_string.contains("window_margin = 0"));
  assert_eq!(configuration_provider.config.general.window_margin, 0);
}

#[test]
fn validate_config_updates_additional_workspace_count_if_loaded_value_exceeds_max() {
  let directory = create_temp_directory();
  let path = directory.path().join(CONFIGURATION_FILE_NAME);
  let config_string = r#"
      [general]
      additional_workspace_count = 15
      "#;
  fs::write(&path, config_string).expect("Failed to write config file");
  let mut config = Configuration::default();
  config.general.additional_workspace_count = 15;
  let mut configuration_provider = ConfigurationProvider::new_test_without_validation(path.clone(), config);

  configuration_provider.validate_config(Some(config_string.into()));

  let config_string = fs::read_to_string(path).expect("Failed to read config file");
  info!("[{}]", config_string);
  assert!(config_string.contains("additional_workspace_count = 8"));
  assert_eq!(configuration_provider.config.general.additional_workspace_count, 8);
}

#[test]
fn reload_configuration_replaces_prior_configuration() {
  let directory = create_temp_directory();
  let path = directory.path().join(CONFIGURATION_FILE_NAME);
  let mut configuration_provider = ConfigurationProvider::new_test(path);

  let new_config = Configuration {
    general: GeneralConfiguration {
      window_margin: 100,
      force_using_admin_privileges: true,
      additional_workspace_count: 8,
      enable_features_using_mouse: false,
      delay_in_ms_before_dragging_is_allowed: 500,
      allow_moving_cursor_after_open_close_or_minimise: false,
    },
    layout: LayoutConfiguration {
      default: Layout::Scrolling,
      monitor: vec![],
    },
    spatial_layout: SpatialLayoutConfiguration {
      allow_selecting_same_center_windows: true,
    },
    scrolling_layout: ScrollingLayoutConfiguration::default(),
    hotkey: vec![CustomHotkey {
      name: "Test App".to_string(),
      path: "C:\\test.exe".to_string(),
      hotkey: "y".to_string(),
      execute_as_admin: true,
    }],
    exclusion_settings: ExclusionSettings::default(),
  };
  configuration_provider
    .file_manager
    .save(&new_config)
    .expect("Failed to write new config file");

  configuration_provider.reload_configuration();

  assert_eq!(configuration_provider.config.general.window_margin, 100);
  assert!(
    configuration_provider
      .config
      .spatial_layout
      .allow_selecting_same_center_windows
  );
  assert!(configuration_provider.config.general.force_using_admin_privileges);
  assert_eq!(configuration_provider.config.general.additional_workspace_count, 8);
  assert!(!configuration_provider.config.general.enable_features_using_mouse);
  assert_eq!(configuration_provider.config.layout.default, Layout::Scrolling);
  assert_eq!(
    configuration_provider.config.general.delay_in_ms_before_dragging_is_allowed,
    500
  );
  assert_eq!(configuration_provider.config.hotkey.len(), 1);
  assert_eq!(configuration_provider.config.hotkey[0].name, "Test App");
  assert!(configuration_provider.config.hotkey[0].execute_as_admin);
}
