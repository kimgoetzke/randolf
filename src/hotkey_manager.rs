use crate::Command;
use crate::configuration_provider::{ConfigurationProvider, DEFAULT_BROWSER, DEFAULT_FILE_MANAGER, DEFAULT_TERMINAL};
use crate::utils::direction::Direction;
use crossbeam_channel::{Receiver, unbounded};
use std::sync::{Arc, Mutex};
use std::thread;
use win_hotkeys::{InterruptHandle, VKey};

const BACKSLASH: u32 = 0xDC;

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

  pub fn default(configuration_provider: Arc<Mutex<ConfigurationProvider>>) -> Self {
    let mut hotkey_manager = HotkeyManager::new(configuration_provider.clone());
    let hkm = &mut hotkey_manager.hkm;

    hkm
      .register_hotkey(VKey::Q, &[VKey::LWin, VKey::Shift], || Command::CloseWindow)
      .unwrap_or_else(|_| panic!("Failed to register hotkey for {:?}", Command::CloseWindow));
    hkm
      .register_hotkey(VKey::CustomKeyCode(BACKSLASH as u16), &[VKey::LWin], || {
        Command::NearMaximiseWindow
      })
      .unwrap_or_else(|_| panic!("Failed to register hotkey for {:?}", Command::NearMaximiseWindow));

    register_move_cursor_hotkey(hkm, Direction::Left, VKey::Left);
    register_move_cursor_hotkey(hkm, Direction::Down, VKey::Down);
    register_move_cursor_hotkey(hkm, Direction::Up, VKey::Up);
    register_move_cursor_hotkey(hkm, Direction::Right, VKey::Right);
    register_move_window_hotkey(hkm, Direction::Left, VKey::Left);
    register_move_window_hotkey(hkm, Direction::Down, VKey::Down);
    register_move_window_hotkey(hkm, Direction::Up, VKey::Up);
    register_move_window_hotkey(hkm, Direction::Right, VKey::Right);
    register_move_window_hotkey(hkm, Direction::Left, VKey::H);
    register_move_window_hotkey(hkm, Direction::Down, VKey::J);
    register_move_window_hotkey(hkm, Direction::Up, VKey::K);
    register_move_window_hotkey(hkm, Direction::Right, VKey::L);
    register_switch_desktop_hotkey(hkm, VKey::Vk1, 1);
    register_switch_desktop_hotkey(hkm, VKey::Vk2, 2);
    register_switch_desktop_hotkey(hkm, VKey::Vk3, 3);
    register_switch_desktop_hotkey(hkm, VKey::Vk4, 4);
    register_switch_desktop_hotkey(hkm, VKey::Vk5, 5);

    hotkey_manager.register_application_hotkey(DEFAULT_TERMINAL, VKey::T, true);
    hotkey_manager.register_application_hotkey(DEFAULT_BROWSER, VKey::F, false);
    hotkey_manager.register_application_hotkey(DEFAULT_FILE_MANAGER, VKey::Home, false);

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

  fn register_application_hotkey(&mut self, application: &str, key: VKey, open_as_admin: bool) {
    if let Some(path_to_executable) = self
      .configuration_provider
      .lock()
      .expect("Failed to read configuration provider")
      .get_str(application)
    {
      let path_to_executable_clone = path_to_executable.clone();
      self
        .hkm
        .register_hotkey(key, &[VKey::LWin], move || {
          Command::OpenApplication(path_to_executable.clone(), open_as_admin)
        })
        .unwrap_or_else({
          |_| {
            panic!(
              "Failed to register hotkey for {:?}",
              Command::OpenApplication(path_to_executable_clone, open_as_admin)
            )
          }
        });
    } else {
      info!(
        "Did not to register hotkey for [{}] because it is not set in configuration file",
        application
      );
    }
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

fn register_switch_desktop_hotkey(hkm: &mut win_hotkeys::HotkeyManager<Command>, key: VKey, desktop: i8) {
  hkm
    .register_hotkey(key, &[VKey::LWin], move || Command::SwitchDesktop(desktop))
    .unwrap_or_else(|err| panic!("Failed to register hotkey for {:?}: {err}", Command::SwitchDesktop(desktop)));
}
