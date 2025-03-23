mod native_api;
mod point;
mod tray_menu_manager;
mod window_manager;
mod window;
mod rect;

#[macro_use]
extern crate log;
extern crate simplelog;

use crate::tray_menu_manager::TrayMenuManager;
use crate::window_manager::{Direction, WindowManager};
use hotkey::Listener;
use simplelog::*;
use std::cell::RefCell;
use std::rc::Rc;
use windows::Win32::UI::Input::KeyboardAndMouse::{MOD_NOREPEAT, MOD_SHIFT, MOD_WIN};

#[allow(dead_code)]
const ARROW_UP: u32 = 0x26;
const ARROW_DOWN: u32 = 0x28;
const ARROW_LEFT: u32 = 0x25;
const ARROW_RIGHT: u32 = 0x27;
const H: u32 = 0x48;
const J: u32 = 0x4A;
const K: u32 = 0x4B;
const L: u32 = 0x4C;
const Q: u32 = 0x51;
const F8: u32 = 0x77;
const F9: u32 = 0x78;
const F10: u32 = 0x79;
const F11: u32 = 0x7A;
const F12: u32 = 0x7B;
const F13: u32 = 0x7C;
const BACKSLASH: u32 = 0xDC;
const MAIN_MOD: u32 = MOD_WIN.0;
const SHIFT: u32 = MOD_SHIFT.0;

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
  let mut listener = Listener::new();
  register_hotkey(&mut listener, &wm, MAIN_MOD | SHIFT, Q, |wm| wm.borrow_mut().close());
  register_hotkey(&mut listener, &wm, MAIN_MOD, F9, |wm| {
    wm.borrow_mut().move_cursor_to_window_in_direction(Direction::Left)
  });
  register_hotkey(&mut listener, &wm, MAIN_MOD, F10, |wm| {
    wm.borrow_mut().move_cursor_to_window_in_direction(Direction::Down)
  });
  register_hotkey(&mut listener, &wm, MAIN_MOD, F11, |wm| {
    wm.borrow_mut().move_cursor_to_window_in_direction(Direction::Left)
  });
  register_hotkey(&mut listener, &wm, MAIN_MOD, F12, |wm| {
    wm.borrow_mut().move_cursor_to_window_in_direction(Direction::Right)
  });
  register_hotkey(&mut listener, &wm, MAIN_MOD | SHIFT, H, |wm| {
    wm.borrow_mut().move_to_left_half_of_screen()
  });
  register_hotkey(&mut listener, &wm, MAIN_MOD | SHIFT, J, |wm| {
    wm.borrow_mut().move_to_bottom_half_of_screen()
  });
  register_hotkey(&mut listener, &wm, MAIN_MOD | SHIFT, K, |wm| {
    wm.borrow_mut().move_to_top_half_of_screen()
  });
  register_hotkey(&mut listener, &wm, MAIN_MOD | SHIFT, L, |wm| {
    wm.borrow_mut().move_to_right_half_of_screen()
  });
  register_hotkey(&mut listener, &wm, MAIN_MOD, BACKSLASH, |wm| {
    wm.borrow_mut().near_maximise_or_restore()
  });
  listener.listen();
}

fn register_hotkey<F>(listener: &mut Listener, wm: &Rc<RefCell<WindowManager>>, mods: u32, hotkey: u32, action: F)
where
  F: Fn(&Rc<RefCell<WindowManager>>) + 'static,
{
  let listener_id = listener
    .register_hotkey(mods | MOD_NOREPEAT.0, hotkey, {
      let wm = Rc::clone(wm);
      move || {
        action(&wm);
      }
    })
    .unwrap_or_else(|err| {
      error!("Failed to register hotkey mods={mods} hotkey={hotkey} because [{err}]");
      panic!("Failed to register hotkey");
    });
  info!("Listener #{listener_id} has been registered...");
}
