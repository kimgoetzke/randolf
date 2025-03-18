mod native_api;

#[macro_use]
extern crate log;
extern crate simplelog;

use hotkey::Listener;
use simplelog::*;
use tray_icon::menu::{Menu, MenuItem};
use tray_icon::{Icon, TrayIconBuilder};
use winapi::shared::windef::{HWND, POINT, RECT};
use winapi::um::winuser::{
  GetActiveWindow, GetForegroundWindow, GetMonitorInfoW, MOD_NOREPEAT, MOD_WIN, MONITOR_DEFAULTTONEAREST, MONITORINFO,
  MonitorFromWindow, SW_MAXIMIZE, SW_SHOWNORMAL, SendMessageW, SetWindowPlacement, ShowWindow, WINDOWPLACEMENT, WM_PAINT,
};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT_FLAGS;

#[allow(dead_code)]
const SLASH: u32 = 0xDC;
const HASH_TAG: u32 = 0xDE;
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
    .with_menu(Box::new(new_tray_menu()))
    .with_tooltip("Randolf")
    .with_icon(icon)
    .with_menu_on_left_click(true)
    .build()
    .expect("Failed to build tray icon");

  // Register hotkey
  let mut listener = Listener::new();
  let listener_id = listener
    .register_hotkey(MOD_WIN as u32 | MOD_NOREPEAT as u32, F13, || resize_active_window())
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

fn resize_active_window() {
  info!("Hotkey has been pressed...");
  let window = native_api::get_foreground_window();
  if window.is_null() {
    debug!("There is no active window...");
    return;
  }

  let margin = 30;
  near_maximize_window(window, margin);
}

pub fn near_maximize_window(window: HWND, margin: i32) {
  // Get the monitor working area for the window
  let monitor_info = match native_api::get_monitor_info(window) {
    Some(value) => value,
    None => return,
  };

  // Get the working area of the screen (excluding taskbar, etc.)
  let work_area = monitor_info.rcWork;

  // Maximize first to get animation effect
  native_api::maximise_window(window);

  // Calculate new window size with padding
  let new_x = work_area.left + margin;
  let new_y = work_area.top + margin + EXTRA_Y_PADDING;
  let new_width = work_area.right - work_area.left - margin * 2;
  let new_height = work_area.bottom - work_area.top - margin * 2 - EXTRA_Y_PADDING;

  // Define the new window placement
  let placement = WINDOWPLACEMENT {
    length: size_of::<WINDOWPLACEMENT>() as u32,
    flags: WINDOWPLACEMENT_FLAGS(0).0,
    showCmd: SW_SHOWNORMAL as u32,
    ptMaxPosition: POINT { x: 0, y: 0 },
    ptMinPosition: POINT { x: -1, y: -1 },
    rcNormalPosition: RECT {
      left: new_x,
      top: new_y,
      right: new_x + new_width,
      bottom: new_y + new_height,
    },
  };

  // Update window placement
  info!("Near-maximizing window: {:?}", window);
  unsafe {
    if SetWindowPlacement(window, &placement) == 0 {
      warn!("Failed to set window placement for window: {:?}", window);
    }

    // Force a repaint
    SendMessageW(window, WM_PAINT, WPARAM(0).0, LPARAM(0).0);
  }
}
