mod hotkey_manager;
mod native_api;
mod tray_menu_manager;
mod utils;
mod window_manager;

#[macro_use]
extern crate log;
extern crate simplelog;

use crate::hotkey_manager::HotkeyManager;
use crate::tray_menu_manager::TrayMenuManager;
use crate::window_manager::WindowManager;
use simplelog::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};
use utils::Command;

const EVENT_LOOP_SLEEP_DURATION: Duration = Duration::from_millis(20);
const HEART_BEAT_DURATION: Duration = Duration::from_secs(5);

fn main() {
  // Initialise logger
  CombinedLogger::init(vec![TermLogger::new(
    LevelFilter::Debug,
    Config::default(),
    TerminalMode::Mixed,
    ColorChoice::Auto,
  )])
  .expect("Failed to initialize logger");

  // Create tray menu
  TrayMenuManager::new();

  // Create window manager and register hotkeys
  let wm = Rc::new(RefCell::new(WindowManager::new()));
  let hkm = HotkeyManager::default();
  let (hotkey_receiver, hkm_interrupt_handle) = hkm.initialise();

  // Run event loop
  let mut last_heartbeat = Instant::now();
  loop {
    native_api::process_windows_messages();
    if let Ok(command) = hotkey_receiver.try_recv() {
      info!("Hotkey pressed: {}", command);
      match command {
        Command::NearMaximiseWindow => wm.borrow_mut().near_maximise_or_restore(),
        Command::MoveWindow(direction) => wm.borrow_mut().move_window(direction),
        Command::MoveCursorToWindowInDirection(direction) => wm.borrow_mut().move_cursor_to_window(direction),
        Command::CloseWindow => wm.borrow_mut().close(),
        Command::Exit => hkm_interrupt_handle.interrupt(),
      }
    }
    last_heartbeat = update_heart_beat(last_heartbeat);
    std::thread::sleep(EVENT_LOOP_SLEEP_DURATION);
  }
}

fn update_heart_beat(last_heartbeat: Instant) -> Instant {
  let now = Instant::now();
  if now.duration_since(last_heartbeat) >= HEART_BEAT_DURATION {
    debug!("Still listening for events...");
    return now;
  }

  last_heartbeat
}
