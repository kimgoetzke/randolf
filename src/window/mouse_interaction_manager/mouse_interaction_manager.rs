use super::native_mouse_interactions::NativeMouseInteractions;
use crate::common::Command;
use crate::configuration::{ConfigurationProvider, DELAY_IN_MS_BEFORE_DRAGGING_IS_ALLOWED, ENABLE_FEATURES_USING_MOUSE};
use crate::utils::CONFIGURATION_PROVIDER_LOCK;
use crossbeam_channel::Sender;
use std::sync::{Arc, Mutex};

pub struct MouseInteractionManager {
  api: Option<NativeMouseInteractions>,
}

impl MouseInteractionManager {
  pub fn new(configuration_provider: Arc<Mutex<ConfigurationProvider>>, sender: Sender<Command>) -> Self {
    let guard = match configuration_provider.try_lock() {
      Ok(guard) => guard,
      Err(err) => {
        error!(
          "Mouse operations are disabled because: {} with error: {}",
          CONFIGURATION_PROVIDER_LOCK, err
        );

        return Self { api: None };
      }
    };
    let is_enabled = guard.get_bool(ENABLE_FEATURES_USING_MOUSE);
    let delay_in_ms = guard.get_i32(DELAY_IN_MS_BEFORE_DRAGGING_IS_ALLOWED) as u32;
    match is_enabled {
      true => Self {
        api: Some(NativeMouseInteractions::new(sender, delay_in_ms)),
      },
      false => Self { api: None },
    }
  }

  pub fn initialise(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(api) = &mut self.api {
      api.initialise()
    } else {
      Ok(())
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::configuration::{ConfigurationProvider, ENABLE_FEATURES_USING_MOUSE};
  use crossbeam_channel::unbounded;
  use std::sync::{Arc, Mutex};

  #[test]
  fn mouse_interaction_manager_initialises_with_enabled_feature() {
    let (sender, _receiver) = unbounded();
    let configuration_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
    let mut manager = MouseInteractionManager::new(configuration_provider, sender);

    assert!(manager.initialise().is_ok());
    assert!(manager.api.is_some());
  }

  #[test]
  fn mouse_interaction_manager_initialises_with_disabled_feature() {
    let (sender, _receiver) = unbounded();
    let configuration_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
    configuration_provider
      .lock()
      .expect("Failed to lock configuration provider")
      .set_bool(ENABLE_FEATURES_USING_MOUSE, false);
    let mut manager = MouseInteractionManager::new(configuration_provider, sender);

    assert!(manager.initialise().is_ok());
    assert!(manager.api.is_none());
  }

  #[test]
  fn mouse_interaction_manager_initialises_when_configuration_provider_lock_fails() {
    let (sender, _receiver) = unbounded();
    let configuration_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
    let configuration_provider_clone = Arc::clone(&configuration_provider);
    let _guard = configuration_provider.lock().expect("Failed to lock configuration provider");
    std::thread::spawn({
      let configuration_provider = Arc::clone(&configuration_provider);
      move || {
        let _ignored = configuration_provider.lock();
      }
    });

    let mut manager = MouseInteractionManager::new(configuration_provider_clone, sender);

    assert!(manager.initialise().is_ok());
    assert!(manager.api.is_none());
  }
}
