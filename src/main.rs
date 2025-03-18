mod native_api;
mod window_manager;

#[macro_use]
extern crate log;
extern crate simplelog;

use hotkey::Listener;
use simplelog::*;
use std::cell::RefCell;
use std::rc::Rc;
use tray_icon::menu::{Menu, MenuItem};
use tray_icon::{Icon, TrayIconBuilder};
use winapi::um::winuser::{MOD_NOREPEAT, MOD_WIN};

const F13: u32 = 0x7C;
const EXTRA_Y_PADDING: i32 = 10;

fn main() {
  // Initialise logger
  CombinedLogger::init(vec![TermLogger::new(
    LevelFilter::Debug,
    Config::default(),
    TerminalMode::Mixed,
    ColorChoice::Auto,
  )])
  .expect("Failed to initialize logger");

  // Initialise tray icon
  let icon = Icon::from_path("assets/icon.ico", Some((32, 32))).expect("Failed to load icon");
  let _tray = TrayIconBuilder::new()
    // .with_menu(Box::new(new_tray_menu()))
    .with_tooltip("Randolf")
    .with_icon(icon)
    .with_menu_on_left_click(true)
    .build()
    .expect("Failed to build tray icon");

  let wm = Rc::new(RefCell::new(window_manager::WindowManager::new()));

  // Register hotkey
  let mut listener = Listener::new();
  let listener_id = listener
    .register_hotkey(MOD_WIN as u32 | MOD_NOREPEAT as u32, F13, move || {
      wm.borrow_mut().near_maximise_active_window()
    })
    .expect("Failed to register hotkey");
  info!("Listener #{listener_id} has been registered and is ready...");
  listener.listen();
}

fn new_tray_menu() -> Menu {
  let menu = Menu::new();
  let exit = MenuItem::new("Exit", true, None);
  if let Err(err) = menu.append(&exit) {
    error!("{err:?}");
  }

  menu
}
