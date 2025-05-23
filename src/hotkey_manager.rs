use crate::common::{Command, Direction, PersistentWorkspaceId};
use crate::configuration_provider::ConfigurationProvider;
use crate::utils::CONFIGURATION_PROVIDER_LOCK;
use crossbeam_channel::Sender;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;
use win_hotkeys::{InterruptHandle, VKey};

const BACKSLASH: u32 = 0xDC;
const MAIN_MOD: VKey = VKey::LWin;
const SECONDARY_MOD: VKey = VKey::Shift;

pub struct HotkeyManager {
  hkm: win_hotkeys::HotkeyManager<Command>,
  configuration_provider: Arc<Mutex<ConfigurationProvider>>,
}

// TODO: Try to make MOD_NOREPEAT work again
impl HotkeyManager {
  fn new(configuration_provider: Arc<Mutex<ConfigurationProvider>>) -> Self {
    Self {
      hkm: win_hotkeys::HotkeyManager::new(),
      configuration_provider,
    }
  }

  pub fn new_with_hotkeys(
    configuration_provider: Arc<Mutex<ConfigurationProvider>>,
    workspace_ids: Vec<PersistentWorkspaceId>,
  ) -> Self {
    let mut hotkey_manager = HotkeyManager::new(configuration_provider.clone());

    // Move cursor
    hotkey_manager.register_move_cursor_hotkey(Direction::Left, VKey::Left);
    hotkey_manager.register_move_cursor_hotkey(Direction::Down, VKey::Down);
    hotkey_manager.register_move_cursor_hotkey(Direction::Up, VKey::Up);
    hotkey_manager.register_move_cursor_hotkey(Direction::Right, VKey::Right);

    // Move window
    hotkey_manager.register_move_window_hotkey(Direction::Left, VKey::Left);
    hotkey_manager.register_move_window_hotkey(Direction::Down, VKey::Down);
    hotkey_manager.register_move_window_hotkey(Direction::Up, VKey::Up);
    hotkey_manager.register_move_window_hotkey(Direction::Right, VKey::Right);
    hotkey_manager.register_move_window_hotkey(Direction::Left, VKey::H);
    hotkey_manager.register_move_window_hotkey(Direction::Down, VKey::J);
    hotkey_manager.register_move_window_hotkey(Direction::Up, VKey::K);
    hotkey_manager.register_move_window_hotkey(Direction::Right, VKey::L);

    // Other window management
    hotkey_manager.register_close_window_hotkey(VKey::Q);
    hotkey_manager.register_near_maximise_window_hotkey(VKey::CustomKeyCode(BACKSLASH as u16));
    hotkey_manager.register_minimise_window_hotkey(VKey::CustomKeyCode(BACKSLASH as u16));

    // Workspace management
    hotkey_manager.register_switch_workspace_hotkeys(&workspace_ids);
    hotkey_manager.register_move_window_to_workspace_hotkeys(&workspace_ids);

    // Launch application
    hotkey_manager.register_application_hotkeys();

    hotkey_manager
  }

  pub fn initialise(mut self, command_sender: Sender<Command>) -> InterruptHandle {
    self.hkm.register_channel(command_sender);
    let interrupt_handle = self.hkm.interrupt_handle();
    thread::spawn(move || {
      self.hkm.event_loop();
    });

    interrupt_handle
  }

  fn register_near_maximise_window_hotkey(&mut self, key: VKey) {
    self
      .hkm
      .register_hotkey(key, &[MAIN_MOD], || Command::NearMaximiseWindow)
      .unwrap_or_else(|err| panic!("Failed to register hotkey for {:?}: {err}", Command::NearMaximiseWindow));
  }

  fn register_minimise_window_hotkey(&mut self, key: VKey) {
    self
      .hkm
      .register_hotkey(key, &[MAIN_MOD, SECONDARY_MOD], || Command::MinimiseWindow)
      .unwrap_or_else(|err| panic!("Failed to register hotkey for {:?}: {err}", Command::MinimiseWindow));
  }

  fn register_close_window_hotkey(&mut self, key: VKey) {
    self
      .hkm
      .register_hotkey(key, &[MAIN_MOD, SECONDARY_MOD], || Command::CloseWindow)
      .unwrap_or_else(|err| panic!("Failed to register hotkey for {:?}: {err}", Command::CloseWindow));
  }

  fn register_switch_workspace_hotkeys(&mut self, workspace_ids: &[PersistentWorkspaceId]) {
    for (i, workspace_id) in workspace_ids.iter().enumerate() {
      let key_number = i + 1;
      if key_number >= 9 {
        warn!(
          "Cannot bind workspace number [{}] to a hotkey because it is greater than 9",
          key_number
        );
        continue;
      }
      match VKey::from_keyname(key_number.to_string().as_str()) {
        Ok(key) => {
          self.register_switch_workspace_hotkey(key, workspace_id);
        }
        Err(err) => {
          warn!("Failed to parse workspace hotkey [{}]: {err}", i);
          continue;
        }
      }
      trace!(
        "Registered hotkey [{}] + [{}] to switch to workspace [{}]",
        MAIN_MOD, key_number, workspace_id
      );
    }
  }

  fn register_switch_workspace_hotkey(&mut self, key: VKey, workspace_id: &PersistentWorkspaceId) {
    let id = *workspace_id;
    self
      .hkm
      .register_hotkey(key, &[MAIN_MOD], move || Command::SwitchWorkspace(id))
      .unwrap_or_else(|err| panic!("Failed to register hotkey for {:?}: {err}", Command::SwitchWorkspace(id)));
  }

  fn register_move_window_to_workspace_hotkeys(&mut self, workspace_ids: &[PersistentWorkspaceId]) {
    for (i, workspace_id) in workspace_ids.iter().enumerate() {
      let key_number = i + 1;
      if key_number >= 9 {
        warn!(
          "Cannot bind workspace number [{}] to a hotkey because it is greater than 9",
          key_number
        );
        continue;
      }
      match VKey::from_keyname(key_number.to_string().as_str()) {
        Ok(key) => {
          self.register_move_window_to_workspace_hotkey(key, workspace_id);
        }
        Err(err) => {
          warn!("Failed to parse workspace hotkey [{}]: {err}", i);
          continue;
        }
      }
      trace!(
        "Registered hotkey [{}] + [{}] + [{}] to move foreground window to workspace [{}]",
        MAIN_MOD, SECONDARY_MOD, key_number, workspace_id
      );
    }
  }

  fn register_move_window_to_workspace_hotkey(&mut self, key: VKey, workspace_id: &PersistentWorkspaceId) {
    let id = *workspace_id;
    self
      .hkm
      .register_hotkey(key, &[MAIN_MOD, SECONDARY_MOD], move || Command::MoveWindowToWorkspace(id))
      .unwrap_or_else(|err| {
        panic!(
          "Failed to register hotkey for {:?}: {err}",
          Command::MoveWindowToWorkspace(id)
        )
      });
  }

  fn register_application_hotkeys(&mut self) {
    let config_provider = self.configuration_provider.clone();
    for hotkey in config_provider.lock().expect(CONFIGURATION_PROVIDER_LOCK).get_hotkeys() {
      match VKey::from_str(&hotkey.hotkey) {
        Ok(key) => {
          self.register_application_hotkey(&hotkey.name, &hotkey.path, key, hotkey.execute_as_admin);
        }
        Err(err) => {
          warn!("Failed to parse hotkey [{}] for [{}]: {err}", hotkey.hotkey, &hotkey.name);
          continue;
        }
      }
    }
  }

  fn register_application_hotkey(&mut self, name: &str, path: &str, key: VKey, open_as_admin: bool) {
    self
      .hkm
      .register_hotkey(key, &[MAIN_MOD], {
        let path_for_closure = path.to_string();
        move || Command::OpenApplication(path_for_closure.clone(), open_as_admin)
      })
      .unwrap_or_else(|err| {
        panic!(
          "Failed to register hotkey for {:?}: {err}",
          Command::OpenApplication(name.to_string(), open_as_admin)
        )
      });
    debug!(
      "Registered hotkey for [{}] to open [{}] as admin [{}]",
      name, path, open_as_admin
    );
  }

  fn register_move_cursor_hotkey(&mut self, direction: Direction, key: VKey) {
    self
      .hkm
      .register_hotkey(key, &[MAIN_MOD], move || Command::MoveCursor(direction))
      .unwrap_or_else(|err| panic!("Failed to register hotkey for {:?}: {err}", Command::MoveCursor(direction)));
  }

  fn register_move_window_hotkey(&mut self, direction: Direction, key: VKey) {
    self
      .hkm
      .register_hotkey(key, &[MAIN_MOD, VKey::Shift], move || Command::MoveWindow(direction))
      .unwrap_or_else(|err| panic!("Failed to register hotkey for {:?}: {err}", Command::MoveWindow(direction)));
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::configuration_provider::CustomHotkey;
  use log::Level::{Debug, Warn};

  #[test]
  fn registers_switch_workspace_hotkeys_for_valid_workspace_ids() {
    testing_logger::setup();
    let mut hotkey_manager = HotkeyManager::new(Arc::new(Mutex::new(ConfigurationProvider::default())));
    let workspace_ids = vec![
      PersistentWorkspaceId::new_test(1),
      PersistentWorkspaceId::new_test(2),
      PersistentWorkspaceId::new_test(3),
    ];

    hotkey_manager.register_switch_workspace_hotkeys(&workspace_ids);

    testing_logger::validate(|captured_logs| {
      assert_eq!(captured_logs.len(), 3);
      for (i, _) in captured_logs.iter().enumerate() {
        assert_eq!(
          captured_logs[i].body,
          format!(
            "Registered hotkey [{}] + [{}] to switch to workspace [wsp#P_DISPLAY-{}]",
            MAIN_MOD,
            i + 1,
            i + 1
          )
        );
      }
    });
  }

  #[test]
  fn register_switch_workspace_hotkeys_skips_workspace_ids_greater_than_9() {
    testing_logger::setup();
    let mut hotkey_manager = HotkeyManager::new(Arc::new(Mutex::new(ConfigurationProvider::default())));
    let mut workspace_ids = vec![];
    for i in 1..=9 {
      workspace_ids.push(PersistentWorkspaceId::new_test(i));
    }

    hotkey_manager.register_switch_workspace_hotkeys(&workspace_ids);

    testing_logger::validate(|captured_logs| {
      assert_eq!(captured_logs.len(), 9);
      assert_eq!(
        captured_logs[8].body,
        "Cannot bind workspace number [9] to a hotkey because it is greater than 9"
      );
      assert_eq!(captured_logs[8].level, Warn);
    });
  }

  #[test]
  fn register_application_hotkeys_test() {
    testing_logger::setup();
    let hotkeys = vec![
      CustomHotkey {
        name: "Test App 1".to_string(),
        path: "C:\\test1.exe".to_string(),
        hotkey: "y".to_string(),
        execute_as_admin: true,
      },
      CustomHotkey {
        name: "Test App 2".to_string(),
        path: "C:\\test2.exe".to_string(),
        hotkey: "invalid".to_string(),
        execute_as_admin: true,
      },
    ];
    let custom_config = ConfigurationProvider::default_with_hotkeys(hotkeys);
    let mut hotkey_manager = HotkeyManager::new(Arc::new(Mutex::new(custom_config)));

    hotkey_manager.register_application_hotkeys();

    testing_logger::validate(|captured_logs| {
      assert_eq!(captured_logs.len(), 2);
      assert_eq!(
        captured_logs[0].body,
        "Registered hotkey for [Test App 1] to open [C:\\test1.exe] as admin [true]"
      );
      assert_eq!(captured_logs[0].level, Debug);
      assert_eq!(
        captured_logs[1].body,
        "Failed to parse hotkey [invalid] for [Test App 2]: Invalid key name `INVALID`"
      );
      assert_eq!(captured_logs[1].level, Warn);
    });
  }
}
