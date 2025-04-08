use crate::configuration_provider::ConfigurationProvider;
use std::process::Command;
use std::sync::{Arc, Mutex};

pub struct ApplicationLauncher {
  #[allow(unused)]
  configuration_provider: Arc<Mutex<ConfigurationProvider>>,
}

impl ApplicationLauncher {
  pub fn new_initialised(configuration_provider: Arc<Mutex<ConfigurationProvider>>) -> Self {
    Self {
      configuration_provider: configuration_provider.clone(),
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
    if as_admin {
      if let Err(e) = Command::new("powershell")
        .args(["-Command", "Start-Process", &path_to_executable, "-Verb", "RunAs"])
        .spawn()
      {
        warn!("Failed to launch application as admin: {}", e);
      }
    } else if let Err(e) = Command::new(path_to_executable).spawn() {
      warn!("Failed to launch application: {}", e);
    }
  }
}
