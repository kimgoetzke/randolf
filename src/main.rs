#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod application_launcher;
mod common;
mod configuration_provider;
mod files;
mod hotkey_manager;
mod log_manager;
mod tray_menu_manager;
mod utils;
mod window_drag_manager;
mod window_manager;
mod workspace_guard;
mod workspace_manager;

#[macro_use]
extern crate log;
extern crate simplelog;

use crate::api::{RealWindowsApi, WindowsApi};
use crate::application_launcher::ApplicationLauncher;
use crate::configuration_provider::{ConfigurationProvider, FORCE_USING_ADMIN_PRIVILEGES};
use crate::files::FileType;
use crate::hotkey_manager::HotkeyManager;
use crate::log_manager::LogManager;
use crate::tray_menu_manager::TrayMenuManager;
use crate::utils::CONFIGURATION_PROVIDER_LOCK;
use crate::window_drag_manager::WindowDragManager;
use crate::window_manager::WindowManager;
use common::Command;
use crossbeam_channel::unbounded;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const EVENT_LOOP_SLEEP_DURATION: Duration = Duration::from_millis(20);
const HEART_BEAT_DURATION: Duration = Duration::from_secs(5);

fn main() {
  LogManager::new_initialised();

  // Create configuration manager and tray menu
  let configuration_manager = Arc::new(Mutex::new(ConfigurationProvider::new()));
  let (command_sender, command_receiver) = unbounded();
  let tray_menu_manager = Rc::new(RefCell::new(TrayMenuManager::new_initialised(
    configuration_manager.clone(),
    command_sender.clone(),
  )));

  // Create Windows API, application launcher, and log current configuration
  let windows_api = RealWindowsApi::new(
    configuration_manager
      .lock()
      .expect(CONFIGURATION_PROVIDER_LOCK)
      .get_exclusion_settings(),
  );
  let launcher = Rc::new(RefCell::new(ApplicationLauncher::new_initialised(
    configuration_manager.clone(),
    windows_api.clone(),
  )));
  configuration_manager
    .lock()
    .expect(CONFIGURATION_PROVIDER_LOCK)
    .log_current_config();

  // Restart the application with admin privileges, if required
  if !windows_api.is_running_as_admin()
    && configuration_manager
      .lock()
      .expect(CONFIGURATION_PROVIDER_LOCK)
      .get_bool(FORCE_USING_ADMIN_PRIVILEGES)
  {
    let executable = launcher.borrow_mut().get_executable_path();
    launcher.borrow_mut().launch(executable, None, true);
    return;
  }

  // Create window manager and register hotkeys
  let wm = Rc::new(RefCell::new(WindowManager::new(
    configuration_manager.clone(),
    windows_api.clone(),
  )));
  let workspace_ids = wm.borrow_mut().get_ordered_permanent_workspace_ids();
  let hkm = HotkeyManager::new_with_hotkeys(configuration_manager.clone(), workspace_ids);
  let interrupt_handle = hkm.initialise(command_sender.clone());

  // Create window drag manager
  let mut window_drag_manager = WindowDragManager::new(windows_api.clone());
  if let Err(e) = window_drag_manager.initialise() {
    error!("Failed to initialise window drag manager: {}", e);
    panic!("Exiting now because application failed to initialise window drag manager");
  }

  // Run event loop
  let mut last_heartbeat = Instant::now();
  loop {
    api::do_process_windows_messages();
    if let Ok(command) = command_receiver.try_recv() {
      info!("Command received: {}", command);
      match command {
        Command::NearMaximiseWindow => wm.borrow_mut().near_maximise_or_restore(),
        Command::MinimiseWindow => wm.borrow_mut().minimise_window(),
        Command::MoveWindow(direction) => wm.borrow_mut().move_window(direction),
        Command::MoveCursor(direction) => wm.borrow_mut().move_cursor(direction),
        Command::CloseWindow => wm.borrow_mut().close_window(),
        Command::SwitchWorkspace(id) => {
          wm.borrow_mut().switch_workspace(id);
          tray_menu_manager.borrow_mut().update_tray_icon(id);
        }
        Command::MoveWindowToWorkspace(id) => wm.borrow_mut().move_window_to_workspace(id),
        Command::OpenApplication(path, as_admin) => launcher.borrow_mut().launch(path, None, as_admin),
        Command::OpenRandolfExecutableFolder => {
          let args = launcher.borrow_mut().get_executable_folder();
          launcher.borrow_mut().launch("explorer.exe".to_string(), Some(&args), false);
        }
        Command::OpenRandolfConfigFolder => {
          let args = launcher.borrow_mut().get_project_folder(FileType::Config);
          launcher.borrow_mut().launch("explorer.exe".to_string(), Some(&args), false);
        }
        Command::OpenRandolfDataFolder => {
          let args = launcher.borrow_mut().get_project_folder(FileType::Data);
          launcher.borrow_mut().launch("explorer.exe".to_string(), Some(&args), false);
        }
        Command::RestartRandolf(as_admin) => {
          wm.borrow_mut().restore_all_managed_windows();
          interrupt_handle.interrupt();
          let as_admin = configuration_manager
            .lock()
            .expect(CONFIGURATION_PROVIDER_LOCK)
            .get_bool(FORCE_USING_ADMIN_PRIVILEGES)
            || as_admin;
          let args = launcher.borrow_mut().get_executable_path();
          launcher.borrow_mut().launch(args, None, as_admin);
          std::process::exit(0);
        }
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
