use crate::Command;
use crate::configuration_provider::ConfigurationProvider;
use crate::utils::{CONFIGURATION_PROVIDER_LOCK, Direction};
use crossbeam_channel::{Receiver, unbounded};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;
use win_hotkeys::{InterruptHandle, VKey};

const BACKSLASH: u32 = 0xDC;

pub struct HotkeyManager {
  hkm: win_hotkeys::HotkeyManager<Command>,
  _configuration_provider: Arc<Mutex<ConfigurationProvider>>,
}

// TODO: Try to make MOD_NOREPEAT work again
impl HotkeyManager {
  fn new(configuration_provider: Arc<Mutex<ConfigurationProvider>>) -> Self {
    Self {
      hkm: win_hotkeys::HotkeyManager::new(),
      _configuration_provider: configuration_provider,
    }
  }

  pub fn new_initialised(configuration_provider: Arc<Mutex<ConfigurationProvider>>, desktop_ids: Vec<isize>) -> Self {
    let mut hotkey_manager = HotkeyManager::new(configuration_provider.clone());
    let hkm = &mut hotkey_manager.hkm;

    // Close window
    hkm
      .register_hotkey(VKey::Q, &[VKey::LWin, VKey::Shift], || Command::CloseWindow)
      .unwrap_or_else(|_| panic!("Failed to register hotkey for {:?}", Command::CloseWindow));

    // Near maximise window
    hkm
      .register_hotkey(VKey::CustomKeyCode(BACKSLASH as u16), &[VKey::LWin], || {
        Command::NearMaximiseWindow
      })
      .unwrap_or_else(|_| panic!("Failed to register hotkey for {:?}", Command::NearMaximiseWindow));

    // Move cursor
    register_move_cursor_hotkey(hkm, Direction::Left, VKey::Left);
    register_move_cursor_hotkey(hkm, Direction::Down, VKey::Down);
    register_move_cursor_hotkey(hkm, Direction::Up, VKey::Up);
    register_move_cursor_hotkey(hkm, Direction::Right, VKey::Right);

    // Move window
    register_move_window_hotkey(hkm, Direction::Left, VKey::Left);
    register_move_window_hotkey(hkm, Direction::Down, VKey::Down);
    register_move_window_hotkey(hkm, Direction::Up, VKey::Up);
    register_move_window_hotkey(hkm, Direction::Right, VKey::Right);
    register_move_window_hotkey(hkm, Direction::Left, VKey::H);
    register_move_window_hotkey(hkm, Direction::Down, VKey::J);
    register_move_window_hotkey(hkm, Direction::Up, VKey::K);
    register_move_window_hotkey(hkm, Direction::Right, VKey::L);

    // Switch desktop
    for (i, desktop_id) in desktop_ids.iter().enumerate() {
      let key_number = i + 1;
      if key_number >= 9 {
        warn!(
          "Cannot bind desktop number [{}] to a hotkey because it is greater than 9",
          key_number
        );
        continue;
      }
      match VKey::from_keyname(key_number.to_string().as_str()) {
        Ok(key) => {
          register_switch_desktop_hotkey(hkm, key, *desktop_id);
        }
        Err(err) => {
          warn!("Failed to parse desktop hotkey [{}]: {err}", i);
          continue;
        }
      }
      debug!(
        "Registered hotkey [Win + {}] to switch to desktop [{}]",
        key_number, desktop_id
      );
    }

    // Launch application
    for hotkey in configuration_provider
      .lock()
      .expect(CONFIGURATION_PROVIDER_LOCK)
      .get_hotkeys()
    {
      match VKey::from_str(&hotkey.hotkey) {
        Ok(key) => {
          hotkey_manager.register_application_hotkey(&hotkey.name, &hotkey.path, key, hotkey.execute_as_admin);
        }
        Err(err) => {
          warn!("Failed to parse hotkey [{}] for [{}]: {err}", hotkey.hotkey, &hotkey.name);
          continue;
        }
      }
    }

    hotkey_manager
  }

  pub fn initialise(mut self) -> (Receiver<Command>, InterruptHandle) {
    let (tx, rx) = unbounded();
    self.hkm.register_channel(tx);
    let handle = self.hkm.interrupt_handle();
    thread::spawn(move || {
      self.hkm.event_loop();
    });

    (rx, handle)
  }

  fn register_application_hotkey(&mut self, name: &str, path: &str, key: VKey, open_as_admin: bool) {
    self
      .hkm
      .register_hotkey(key, &[VKey::LWin], {
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
}

fn register_move_cursor_hotkey(hkm: &mut win_hotkeys::HotkeyManager<Command>, direction: Direction, key: VKey) {
  hkm
    .register_hotkey(key, &[VKey::LWin], move || Command::MoveCursor(direction))
    .unwrap_or_else(|err| panic!("Failed to register hotkey for {:?}: {err}", Command::MoveCursor(direction)));
}

fn register_move_window_hotkey(hkm: &mut win_hotkeys::HotkeyManager<Command>, direction: Direction, key: VKey) {
  hkm
    .register_hotkey(key, &[VKey::LWin, VKey::Shift], move || Command::MoveWindow(direction))
    .unwrap_or_else(|err| panic!("Failed to register hotkey for {:?}: {err}", Command::MoveWindow(direction)));
}

fn register_switch_desktop_hotkey(hkm: &mut win_hotkeys::HotkeyManager<Command>, key: VKey, desktop: isize) {
  hkm
    .register_hotkey(key, &[VKey::LWin], move || Command::SwitchDesktop(desktop))
    .unwrap_or_else(|err| panic!("Failed to register hotkey for {:?}: {err}", Command::SwitchDesktop(desktop)));
}
