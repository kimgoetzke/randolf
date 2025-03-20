mod native_api;
mod tray_menu_manager;
mod window_manager;

#[macro_use]
extern crate log;
extern crate simplelog;

use crate::tray_menu_manager::TrayMenuManager;
use crate::window_manager::WindowManager;
use hotkey::Listener;
use simplelog::*;
use std::cell::RefCell;
use std::rc::Rc;
use winapi::um::winuser::{MOD_NOREPEAT, MOD_WIN};

const F9: u32 = 0x78;
const F10: u32 = 0x79;
const F11: u32 = 0x7A;
const F12: u32 = 0x7B;
const F13: u32 = 0x7C;

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
  register_hotkey(&mut listener, &wm, F9, |wm| wm.borrow_mut().move_to_top_half_of_screen());
  register_hotkey(&mut listener, &wm, F10, |wm| wm.borrow_mut().move_to_bottom_half_of_screen() );
  register_hotkey(&mut listener, &wm, F11, |wm| wm.borrow_mut().move_to_left_half_of_screen());
  register_hotkey(&mut listener, &wm, F12, |wm| wm.borrow_mut().move_to_right_half_of_screen());
  register_hotkey(&mut listener, &wm, F13, |wm| wm.borrow_mut().near_maximise_or_restore());
  listener.listen();
}

fn register_hotkey<F>(listener: &mut Listener, wm: &Rc<RefCell<WindowManager>>, hotkey: u32, action: F)
where
  F: Fn(&Rc<RefCell<WindowManager>>) + 'static,
{
  let listener_id = listener
    .register_hotkey(MOD_WIN as u32 | MOD_NOREPEAT as u32, hotkey, {
      let wm = Rc::clone(&wm);
      move || {
        action(&wm);
      }
    })
    .expect("Failed to register hotkey");
  info!("Listener #{listener_id} has been registered...");
}
