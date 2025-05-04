use serde::Serialize;
use serde::de::DeserializeOwned;
use std::error::Error;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

pub trait FileType: Default + Serialize + DeserializeOwned {}

impl<T> FileType for T where T: Default + Serialize + DeserializeOwned {}

pub struct FileManager<T: FileType> {
  file_path: PathBuf,
  file_prefix: String,
  _marker: std::marker::PhantomData<T>,
}

impl<T: FileType> FileManager<T> {
  pub fn new(file_name: &str) -> Self {
    let path = Self::get_path_to_file(file_name).unwrap_or_else(|err| panic!("Failed to get path to {file_name}: {err}:"));
    FileManager {
      file_path: path,
      file_prefix: String::new(),
      _marker: Default::default(),
    }
  }

  pub fn set_prefix(&mut self, prefix: &str) {
    self.file_prefix = prefix.to_string();
  }

  pub fn get_path_to_file(file_name: &str) -> Result<PathBuf, Box<dyn Error>> {
    if let Ok(executable_path) = std::env::current_exe() {
      if let Some(executable_directory) = executable_path.parent() {
        Ok(executable_directory.join(file_name))
      } else {
        Err("Failed to get the parent directory of the executable path".into())
      }
    } else {
      Err("Failed to get the current executable path".into())
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
  use serde::Deserialize;
  use std::fs::File;
  use std::io::Write;
  use tempfile::TempDir;

  #[derive(Default, Serialize, Deserialize)]
  struct TestConfig {
    key: String,
    value: i32,
  }

  impl<T: FileType> FileManager<T> {
    pub fn default() -> Self {
      FileManager {
        file_path: PathBuf::new(),
        file_prefix: String::new(),
        _marker: Default::default(),
      }
    }

    pub fn new_test(file_name: &str) -> Self {
      let directory = create_temp_directory();
      let path = directory.path().join(file_name);
      FileManager {
        file_path: path,
        file_prefix: String::new(),
        _marker: Default::default(),
      }
    }

    pub fn new_test_with_custom_path(path: PathBuf) -> Self {
      FileManager {
        file_path: path,
        file_prefix: String::new(),
        _marker: Default::default(),
      }
    }
  }

  fn create_temp_directory() -> TempDir {
    TempDir::new().expect("Failed to create temporary directory")
  }

  #[test]
  fn load_or_create_loads_existing_file_if_present() {
    let file_name = "test_config.toml";
    let file_path = FileManager::<TestConfig>::get_path_to_file(file_name).unwrap();
    let mut file = File::create(&file_path).unwrap();
    writeln!(file, "key = \"test\"\nvalue = 42").unwrap();

    let file_manager = FileManager::<TestConfig>::new(file_name);
    let (config, _) = file_manager.load_or_create().expect("Failed to load or create config");

    assert_eq!(config.key, "test");
    assert_eq!(config.value, 42);

    fs::remove_file(&file_path).unwrap();
  }

  #[test]
  fn load_or_create_creates_default_file_if_not_found() {
    let temp_dir = create_temp_directory();
    let file_path = temp_dir.path().join("nonexistent_config.toml");
    let file_manager = FileManager::<TestConfig>::new_test_with_custom_path(file_path.clone());

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

    let file_manager = FileManager::<TestConfig>::new_test_with_custom_path(file_path);

    let result = file_manager.load_or_create();

    assert!(result.is_err());
  }

  #[test]
  fn load_or_create_handles_empty_file_gracefully() {
    let temp_dir = create_temp_directory();
    let file_path = temp_dir.path().join("empty_config.toml");
    File::create(&file_path).unwrap();

    let file_manager = FileManager::<TestConfig>::new_test_with_custom_path(file_path);

    let result = file_manager.load_or_create();

    assert!(result.is_err());
  }
}
