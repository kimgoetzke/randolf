mod command;
mod direction;
mod hotkey_manager;
mod native_api;
mod point;
mod rect;
mod sizing;
mod tray_menu_manager;
mod utils;
mod window;
mod window_manager;

#[macro_use]
extern crate log;
extern crate simplelog;

use crate::command::Command;
use crate::hotkey_manager::HotkeyManager;
use crate::tray_menu_manager::TrayMenuManager;
use crate::window_manager::WindowManager;
use simplelog::*;
use std::cell::RefCell;
use std::rc::Rc;

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
  let (receiver, interrupt_handle) = hkm.initialise();

  // Run event loop
  loop {
    let command = receiver.recv().unwrap();
    info!("Hotkey pressed: {}", command);
    match command {
      Command::NearMaximiseWindow => wm.borrow_mut().near_maximise_or_restore(),
      Command::MoveWindow(direction) => wm.borrow_mut().move_window(direction),
      Command::MoveCursorToWindowInDirection(direction) => wm.borrow_mut().move_cursor_to_window(direction),
      Command::CloseWindow => wm.borrow_mut().close(),
      Command::Exit => interrupt_handle.interrupt(),
    }
  }
}
