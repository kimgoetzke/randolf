use crate::files::file_type::FileType;
use directories::ProjectDirs;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::error::Error;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// A struct to manage file operations for a single file, located at `file_path` and deserialised to type `T`. Allows
/// you to load, create, reload, and save this file.
pub struct FileManager<T: Default + Serialize + DeserializeOwned> {
  file_path: PathBuf,
  file_prefix: String,
  _marker: std::marker::PhantomData<T>,
}

impl<T: Default + Serialize + DeserializeOwned> FileManager<T> {
  pub fn new(file_name: &str, file_type: FileType) -> Self {
    FileManager {
      file_path: Self::get_path_to_file(file_name, file_type)
        .unwrap_or_else(|err| panic!("Failed to get path to {file_name}: {err}:")),
      file_prefix: String::new(),
      _marker: Default::default(),
    }
  }

  /// Set the prefix to be added to the file content e.g. `# This is a comment`. Make sure the prefix ends with a
  /// newline.
  pub fn set_content_prefix(&mut self, prefix: &str) {
    self.file_prefix = prefix.to_string();
  }

  /// Get the path to the file, creating the directory (but not the file) if it doesn't exist. Storage location is
  /// determined by the `FileType` enum.
  pub fn get_path_to_file(file_name: &str, file_type: FileType) -> Result<PathBuf, Box<dyn Error>> {
    if let Some(project_directories) = ProjectDirs::from("io", "kimgoetzke", "randolf") {
      let file_directory = Self::determine_file_directory(file_type, &project_directories);
      if let Err(err) = fs::create_dir_all(file_directory) {
        error!("Failed to create directory [{}] : {err}", file_directory.display());
        return Err(Box::new(err));
      }

      Ok(file_directory.join(file_name))
    } else {
      Err("Could not determine standard project directories".into())
    }
  }

  fn determine_file_directory(file_type: FileType, project_directories: &ProjectDirs) -> &Path {
    match file_type {
      FileType::Config => project_directories.config_dir(),
      FileType::Data => project_directories.data_local_dir(),
    }
  }

  pub fn load_or_create(&self) -> Result<(T, Option<String>), Box<dyn Error>> {
    match fs::read_to_string(&self.file_path) {
      Ok(file_content) => {
        let t: T = match toml::from_str(&file_content) {
          Ok(parsed) => parsed,
          Err(err) => {
            error!("Failed to parse [{}]: {}", self.file_path.display(), err);
            return Err(Box::new(err));
          }
        };

        Ok((t, Some(file_content)))
      }
      Err(err) => {
        if err.kind() == ErrorKind::NotFound {
          info!("File not found, creating default file: {}", self.file_path.display());
          let t = T::default();
          let toml_string = toml::to_string_pretty(&t)?;
          fs::write(&self.file_path, format!("{}{}", self.file_prefix, toml_string))?;

          Ok((t, None))
        } else {
          error!("Failed to load [{}] ({}): {}", self.file_path.display(), err.kind(), err);

          Err(Box::new(err))
        }
      }
    }
  }

  pub fn reload(&mut self) -> Result<(T, Option<String>), Box<dyn Error>> {
    info!("Reloading [{}]", self.file_path.display());

    self.load_or_create()
  }

  pub fn save(&self, t: &T) -> Result<(), Box<dyn Error>> {
    info!("Saving [{}]", self.file_path.display());
    let toml_string = toml::to_string_pretty(t)?;
    fs::write(&self.file_path, format!("{}{}", self.file_prefix, toml_string))?;

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::utils::create_temp_directory;
  use serde::Deserialize;
  use std::fs::File;
  use std::io::Write;

  #[derive(Default, Serialize, Deserialize)]
  struct TestConfig {
    key: String,
    value: i32,
  }

  impl<T: Default + Serialize + DeserializeOwned> FileManager<T> {
    pub fn default() -> Self {
      FileManager {
        file_path: PathBuf::new(),
        file_prefix: String::new(),
        _marker: Default::default(),
      }
    }

    pub fn new_test(path: PathBuf) -> Self {
      FileManager {
        file_path: path,
        file_prefix: String::new(),
        _marker: Default::default(),
      }
    }
  }

  #[test]
  fn get_path_to_file_returns_correct_path_for_data_file_type() {
    let file_name = "test_log.toml";

    let file_path = FileManager::<TestConfig>::get_path_to_file(file_name, FileType::Data);

    assert!(file_path.is_ok());
    let file_path = file_path.unwrap();
    assert!(file_path.to_str().unwrap().contains("AppData\\Local\\kimgoetzke"));
    assert!(file_path.ends_with("test_log.toml"));
  }

  #[test]
  fn get_path_to_file_returns_correct_path_for_config_file_type() {
    let file_name = "test_config.toml";

    let file_path = FileManager::<TestConfig>::get_path_to_file(file_name, FileType::Config);

    assert!(file_path.is_ok());
    let file_path = file_path.unwrap();
    assert!(file_path.to_str().unwrap().contains("AppData\\Roaming\\kimgoetzke"));
    assert!(file_path.ends_with("test_config.toml"));
  }

  #[test]
  fn load_or_create_loads_existing_file_if_present() {
    let file_name = "test_config.toml";
    let file_path = FileManager::<TestConfig>::get_path_to_file(file_name, FileType::Data).unwrap();
    let mut file = File::create(&file_path).unwrap();
    writeln!(file, "key = \"test\"\nvalue = 42").unwrap();

    let file_manager = FileManager::<TestConfig>::new(file_name, FileType::Data);
    let (config, _) = file_manager.load_or_create().expect("Failed to load or create config");

    assert_eq!(config.key, "test");
    assert_eq!(config.value, 42);

    // Clean up the created file because we're not using TempDir here
    fs::remove_file(&file_path).unwrap();
  }

  #[test]
  fn load_or_create_creates_default_file_if_not_found() {
    let temp_dir = create_temp_directory();
    let file_path = temp_dir.path().join("nonexistent_config.toml");
    let file_manager = FileManager::<TestConfig>::new_test(file_path.clone());

    let (config, content) = file_manager.load_or_create().expect("Failed to load or create config");

    assert_eq!(config.key, "");
    assert_eq!(config.value, 0);
    assert!(content.is_none());
    assert!(file_path.exists());
  }

  #[test]
  fn load_or_create_returns_error_for_invalid_toml() {
    let temp_dir = create_temp_directory();
    let file_path = temp_dir.path().join("invalid_config.toml");
    let mut file = File::create(&file_path).unwrap();
    writeln!(file, "invalid_toml_content").unwrap();

    let file_manager = FileManager::<TestConfig>::new_test(file_path);

    let result = file_manager.load_or_create();
    assert!(result.is_err());
  }

  #[test]
  fn load_or_create_handles_empty_file_gracefully() {
    let temp_dir = create_temp_directory();
    let file_path = temp_dir.path().join("empty_config.toml");
    File::create(&file_path).unwrap();

    let file_manager = FileManager::<TestConfig>::new_test(file_path);

    let result = file_manager.load_or_create();
    assert!(result.is_err());
    if let Err(err) = result {
      assert!(err.to_string().contains("missing field `key`"));
    }
  }
}
