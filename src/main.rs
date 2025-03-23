mod hotkey_manager;
mod native_api;
mod point;
mod rect;
mod tray_menu_manager;
mod window;
mod window_manager;
mod sizing;

#[macro_use]
extern crate log;
extern crate simplelog;

use crate::tray_menu_manager::TrayMenuManager;
use crate::window_manager::{Direction, WindowManager};
use simplelog::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::hotkey_manager::HotkeyManager;

#[derive(Debug)]
enum ControlFlow {
  CloseWindow,
  NearMaximiseWindow,
  MoveWindow(Direction),
  MoveCursorToWindowInDirection(Direction),
  Exit,
}

// TODO: Make window resizing work with arrow keys (in addition to, or instead of, h/j/k/l)
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

  loop {
    let command = receiver.recv().unwrap();
    info!("Hotkey pressed: {:?}", command);
    match command {
      ControlFlow::NearMaximiseWindow => wm.borrow_mut().near_maximise_or_restore(),
      ControlFlow::MoveWindow(direction) => wm.borrow_mut().move_window(direction),
      ControlFlow::MoveCursorToWindowInDirection(direction) => wm.borrow_mut().move_cursor_to_window(direction),
      ControlFlow::CloseWindow => wm.borrow_mut().close(),
      ControlFlow::Exit => interrupt_handle.interrupt(),
    }
  }
}
