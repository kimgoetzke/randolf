mod native_api;
mod window_manager;

#[macro_use]
extern crate log;
extern crate simplelog;

use crate::window_manager::WindowManager;
use hotkey::Listener;
use simplelog::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem};
use tray_icon::{Icon, TrayIconBuilder, TrayIconEvent};
use winapi::um::winuser::{MOD_NOREPEAT, MOD_WIN};

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

  // Initialise tray icon
  let icon = Icon::from_path("assets/icon.ico", Some((32, 32))).expect("Failed to load icon");
  let (menu, menu_items) = new_tray_menu();
  let _tray = TrayIconBuilder::new()
    .with_menu(Box::new(menu))
    .with_tooltip("Randolf")
    .with_icon(icon)
    .with_menu_on_left_click(true)
    .build()
    .expect("Failed to build tray icon");

  std::thread::spawn({
    move || {
      if let Ok(event) = MenuEvent::receiver().try_recv() {
        debug!("Received tray icon menu event: {:?}", event);
        if event.id == menu_items.get_key_value("Exit").expect("Exit menu item not found").1.0 {
          std::process::exit(0);
        }
      }
    }
  });

  // Create window manager and register hotkeys
  let wm = Rc::new(RefCell::new(WindowManager::new()));
  let mut listener = Listener::new();
  register_hotkey(&mut listener, &wm, F11, |wm| wm.borrow_mut().near_maximise_active_window());
  // register_hotkey(&mut listener, &wm, F13, |wm| wm.borrow_mut().near_maximise_active_window());
  register_hotkey(&mut listener, &wm, F12, |wm| wm.borrow_mut().something_else());
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

fn new_tray_menu() -> (Menu, HashMap<String, MenuId>) {
  let mut menu_item_ids = HashMap::new();
  let menu = Menu::new();
  let exit = MenuItem::new("Exit", true, None);
  menu_item_ids.insert("Exit".to_owned(), exit.id().clone());
  if let Err(err) = menu.append(&exit) {
    error!("{err:?}");
  }

  (menu, menu_item_ids)
}
