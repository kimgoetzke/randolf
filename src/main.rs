#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod application_launcher;
mod configuration_provider;
mod hotkey_manager;
mod log_manager;
mod tray_menu_manager;
mod utils;
mod window_manager;
mod workspace_manager;

#[macro_use]
extern crate log;
extern crate simplelog;

use crate::api::RealWindowsApi;
use crate::application_launcher::ApplicationLauncher;
use crate::configuration_provider::ConfigurationProvider;
use crate::hotkey_manager::HotkeyManager;
use crate::log_manager::LogManager;
use crate::tray_menu_manager::TrayMenuManager;
use crate::utils::CONFIGURATION_PROVIDER_LOCK;
use crate::window_manager::WindowManager;
use crossbeam_channel::unbounded;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use utils::Command;

const EVENT_LOOP_SLEEP_DURATION: Duration = Duration::from_millis(20);
const HEART_BEAT_DURATION: Duration = Duration::from_secs(5);

fn main() {
  LogManager::new_initialised();
  let windows_api = RealWindowsApi::new();

  // Create configuration manager and tray menu
  let configuration_manager = Arc::new(Mutex::new(ConfigurationProvider::new()));
  let (command_sender, command_receiver) = unbounded();
  TrayMenuManager::new_initialised(configuration_manager.clone(), command_sender.clone());
  let launcher = Rc::new(RefCell::new(ApplicationLauncher::new_initialised(
    configuration_manager.clone(),
    windows_api,
  )));
  configuration_manager
    .lock()
    .expect(CONFIGURATION_PROVIDER_LOCK)
    .log_current_config();

  // Create window manager and register hotkeys
  let wm = Rc::new(RefCell::new(WindowManager::new(configuration_manager.clone(), windows_api)));
  let workspace_ids = wm.borrow_mut().get_ordered_permanent_workspace_ids();
  let hkm = HotkeyManager::new_with_hotkeys(configuration_manager.clone(), workspace_ids);
  let interrupt_handle = hkm.initialise(command_sender);

  // Run event loop
  let mut last_heartbeat = Instant::now();
  loop {
    api::do_process_windows_messages();
    if let Ok(command) = command_receiver.try_recv() {
      info!("Command received: {}", command);
      match command {
        Command::NearMaximiseWindow => wm.borrow_mut().near_maximise_or_restore(),
        Command::MoveWindow(direction) => wm.borrow_mut().move_window(direction),
        Command::MoveCursor(direction) => wm.borrow_mut().move_cursor(direction),
        Command::CloseWindow => wm.borrow_mut().close_window(),
        Command::SwitchWorkspace(id) => wm.borrow_mut().switch_workspace(id),
        Command::MoveWindowToWorkspace(id) => wm.borrow_mut().move_window_to_workspace(id),
        Command::OpenApplication(path, as_admin) => launcher.borrow_mut().launch(path, as_admin),
        Command::Exit => {
          wm.borrow_mut().restore_all_managed_windows();
          interrupt_handle.interrupt();
          info!("Application exited cleanly");
          std::process::exit(0);
        }
      }
    }
    last_heartbeat = update_heart_beat(last_heartbeat);
    std::thread::sleep(EVENT_LOOP_SLEEP_DURATION);
  }
}

fn update_heart_beat(last_heartbeat: Instant) -> Instant {
  let now = Instant::now();
  if now.duration_since(last_heartbeat) >= HEART_BEAT_DURATION {
    #[cfg(debug_assertions)]
    trace!("Still listening for events...");
    return now;
  }

  last_heartbeat
}
