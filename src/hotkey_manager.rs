use crate::Command;
use crate::utils::direction::Direction;
use crossbeam_channel::{Receiver, unbounded};
use std::thread;
use win_hotkeys::{InterruptHandle, VKey};

const BACKSLASH: u32 = 0xDC;

pub struct HotkeyManager {
  _hkm: win_hotkeys::HotkeyManager<Command>,
}

// TODO: Try to make MOD_NOREPEAT work again
impl HotkeyManager {
  pub fn default() -> Self {
    let mut hkm = win_hotkeys::HotkeyManager::new();

    hkm
      .register_hotkey(VKey::Q, &[VKey::LWin, VKey::Shift], || Command::CloseWindow)
      .unwrap_or_else(|_| panic!("Failed to register hotkey for {:?}", Command::CloseWindow));
    hkm
      .register_hotkey(VKey::CustomKeyCode(BACKSLASH as u16), &[VKey::LWin], || {
        Command::NearMaximiseWindow
      })
      .unwrap_or_else(|_| panic!("Failed to register hotkey for {:?}", Command::NearMaximiseWindow));

    register_move_cursor_hotkey(&mut hkm, Direction::Left, VKey::Left);
    register_move_cursor_hotkey(&mut hkm, Direction::Down, VKey::Down);
    register_move_cursor_hotkey(&mut hkm, Direction::Up, VKey::Up);
    register_move_cursor_hotkey(&mut hkm, Direction::Right, VKey::Right);
    register_move_window_hotkey(&mut hkm, Direction::Left, VKey::Left);
    register_move_window_hotkey(&mut hkm, Direction::Down, VKey::Down);
    register_move_window_hotkey(&mut hkm, Direction::Up, VKey::Up);
    register_move_window_hotkey(&mut hkm, Direction::Right, VKey::Right);
    register_move_window_hotkey(&mut hkm, Direction::Left, VKey::H);
    register_move_window_hotkey(&mut hkm, Direction::Down, VKey::J);
    register_move_window_hotkey(&mut hkm, Direction::Up, VKey::K);
    register_move_window_hotkey(&mut hkm, Direction::Right, VKey::L);

    Self { _hkm: hkm }
  }

  pub fn initialise<'a>(mut self) -> (Receiver<Command>, InterruptHandle) {
    let (tx, rx) = unbounded();
    self._hkm.register_channel(tx);
    let handle = self._hkm.interrupt_handle();
    thread::spawn(move || {
      self._hkm.event_loop();
    });

    (rx, handle)
  }
}

fn register_move_cursor_hotkey(hkm: &mut win_hotkeys::HotkeyManager<Command>, direction: Direction, key: VKey) {
  hkm
    .register_hotkey(key, &[VKey::LWin], move || {
      Command::MoveCursorToWindowInDirection(direction)
    })
    .unwrap_or_else(|err| {
      panic!(
        "Failed to register hotkey for {:?}: {err}",
        Command::MoveCursorToWindowInDirection(direction)
      )
    });
}

fn register_move_window_hotkey(hkm: &mut win_hotkeys::HotkeyManager<Command>, direction: Direction, key: VKey) {
  hkm
    .register_hotkey(key, &[VKey::LWin, VKey::Shift], move || Command::MoveWindow(direction))
    .unwrap_or_else(|err| {
      panic!(
        "Failed to register hotkey for {:?}: {err}",
        Command::MoveWindow(direction)
      )
    });
}
