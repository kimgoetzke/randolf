use crate::api::get_all_monitors;
use crate::common::{Command, PersistentWorkspaceId};
use crate::configuration_provider::{
  ALLOW_SELECTING_SAME_CENTER_WINDOWS, ConfigurationProvider, FORCE_USING_ADMIN_PRIVILEGES, WINDOW_MARGIN,
};
use crate::utils::{CONFIGURATION_PROVIDER_LOCK, TRAY_ICON_LOCK, TRAY_ICON_OPEN};
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::sync::atomic::{AtomicBool, AtomicU8};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use trayicon::*;

static WORKSPACE: AtomicU8 = AtomicU8::new(1);
static IS_DRAG_ICON_SHOWN: AtomicBool = AtomicBool::new(false);

pub struct TrayMenuManager {
  configuration_provider: Arc<Mutex<ConfigurationProvider>>,
  menu: Option<Arc<Mutex<TrayIcon<Event>>>>,
  workspace_tray_icons: Vec<Icon>,
  drag_icon: Icon,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Event {
  RightClickTrayIcon,
  LeftClickTrayIcon,
  DoubleClickTrayIcon,
  Exit,
  DisabledItem,
  SetMargin(i32),
  ToggleSelectingSameCenterWindows,
  ToggleForceUsingAdminPrivileges,
  LogMonitorLayout,
  RestartRandolf(bool),
  OpenRandolfExecutableFolder,
  OpenRandolfConfigFolder,
  OpenRandolfDataFolder,
}

impl TrayMenuManager {
  fn new(configuration_provider: Arc<Mutex<ConfigurationProvider>>) -> Self {
    Self {
      configuration_provider,
      menu: None,
      workspace_tray_icons: vec![],
      drag_icon: Icon::from_buffer(include_bytes!("../assets/randolf-drag.ico"), Some(32), Some(32))
        .expect("Failed to create drag icon from buffer"),
    }
  }

  pub fn new_initialised(
    configuration_provider: Arc<Mutex<ConfigurationProvider>>,
    command_sender: Sender<Command>,
  ) -> Self {
    let mut manager = Self::new(configuration_provider);
    let (tray_event_sender, tray_event_receiver) = unbounded();
    let tray = manager.create_tray_icon(tray_event_sender);
    manager.menu = Some(Arc::from(Mutex::new(tray)));
    manager.initialise(tray_event_receiver, command_sender);
    manager.workspace_tray_icons = (1..=9).map(Self::create_icon).collect();
    debug!("Created tray icon & menu");

    manager
  }

  fn create_icon(index: u8) -> Icon {
    let icon_data = match index {
      1 => include_bytes!("../assets/randolf-1.ico"),
      2 => include_bytes!("../assets/randolf-2.ico"),
      3 => include_bytes!("../assets/randolf-3.ico"),
      4 => include_bytes!("../assets/randolf-4.ico"),
      5 => include_bytes!("../assets/randolf-5.ico"),
      6 => include_bytes!("../assets/randolf-6.ico"),
      7 => include_bytes!("../assets/randolf-7.ico"),
      8 => include_bytes!("../assets/randolf-8.ico"),
      9 => include_bytes!("../assets/randolf-9.ico"),
      _ => panic!("Invalid icon index"),
    };

    Icon::from_buffer(icon_data, Some(32), Some(32)).expect("Failed to create icon from buffer")
  }

  fn create_tray_icon(&mut self, tx: Sender<Event>) -> TrayIcon<Event> {
    TrayIconBuilder::new()
      .sender(move |e| {
        let _ = tx.send(*e);
      })
      .icon_from_buffer(include_bytes!("../assets/randolf.ico"))
      .tooltip("Randolf")
      .on_right_click(Event::RightClickTrayIcon)
      .on_click(Event::LeftClickTrayIcon)
      .on_double_click(Event::DoubleClickTrayIcon)
      .menu(build_menu(&self.configuration_provider))
      .build()
      .expect("Failed to build tray icon")
  }

  // TODO: Update margins of "known" windows when the margin is changed
  fn initialise(&self, rx: Receiver<Event>, command_sender: Sender<Command>) {
    let tray_icon = Arc::clone(self.menu.as_ref().unwrap());
    let config_provider = self.configuration_provider.clone();
    thread::spawn(move || {
      rx.iter().for_each(|m| match m {
        Event::RightClickTrayIcon => {
          tray_icon.lock().expect(TRAY_ICON_LOCK).show_menu().expect(TRAY_ICON_OPEN);
        }
        Event::DoubleClickTrayIcon => {
          trace!("Tray icon double clicked: Not implemented");
        }
        Event::LeftClickTrayIcon => {
          tray_icon.lock().expect(TRAY_ICON_LOCK).show_menu().expect(TRAY_ICON_OPEN);
        }
        Event::LogMonitorLayout => {
          get_all_monitors().print_layout();
          info!("Logged monitor layout");
        }
        Event::SetMargin(margin) => {
          let current_margin = { unlocked_config_provider(&config_provider).get_i32(WINDOW_MARGIN) };
          if current_margin != margin {
            unlocked_config_provider(&config_provider).set_i32(WINDOW_MARGIN, margin);
            let menu = build_menu(&config_provider);
            if let Err(err) = tray_icon.lock().expect(TRAY_ICON_LOCK).set_menu(&menu) {
              error!("Failed to set menu: {err}");
            }
            debug!("Set window margin to [{}]", margin);
          }
        }
        Event::ToggleSelectingSameCenterWindows => {
          let mut config = unlocked_config_provider(&config_provider);
          let is_enabled = config.get_bool(ALLOW_SELECTING_SAME_CENTER_WINDOWS);
          if let Err(result) = tray_icon
            .lock()
            .expect(TRAY_ICON_LOCK)
            .set_menu_item_checkable(Event::ToggleSelectingSameCenterWindows, !is_enabled)
          {
            error!("Failed to toggle menu item: {result}");
          }
          config.set_bool(ALLOW_SELECTING_SAME_CENTER_WINDOWS, !is_enabled);
          debug!("Set [{:?}] to [{}]", Event::ToggleSelectingSameCenterWindows, !is_enabled);
        }
        Event::ToggleForceUsingAdminPrivileges => {
          let mut config = unlocked_config_provider(&config_provider);
          let is_enabled = config.get_bool(FORCE_USING_ADMIN_PRIVILEGES);
          if let Err(result) = tray_icon
            .lock()
            .expect(TRAY_ICON_LOCK)
            .set_menu_item_checkable(Event::ToggleForceUsingAdminPrivileges, !is_enabled)
          {
            error!("Failed to toggle menu item: {result}");
          }
          config.set_bool(FORCE_USING_ADMIN_PRIVILEGES, !is_enabled);
          debug!("Set [{:?}] to [{}]", Event::ToggleForceUsingAdminPrivileges, !is_enabled);
        }
        Event::OpenRandolfExecutableFolder => {
          command_sender
            .send(Command::OpenRandolfExecutableFolder)
            .expect("Failed to send open randolf executable folder command");
        }
        Event::OpenRandolfConfigFolder => {
          command_sender
            .send(Command::OpenRandolfConfigFolder)
            .expect("Failed to send open randolf config folder command");
        }
        Event::OpenRandolfDataFolder => {
          command_sender
            .send(Command::OpenRandolfDataFolder)
            .expect("Failed to send open randolf data folder command");
        }
        Event::RestartRandolf(as_admin) => {
          let mut config = unlocked_config_provider(&config_provider);
          config.reload_configuration();
          command_sender
            .send(Command::RestartRandolf(as_admin))
            .expect("Failed to send restart command");
        }
        Event::Exit => {
          command_sender.send(Command::Exit).expect("Failed to send exit command");
        }
        e => {
          error!("Received unhandled tray menu event: {:?}", e);
        }
      })
    });
  }

  pub fn update_tray_icon(&self, workspace_id: PersistentWorkspaceId) {
    if !workspace_id.is_on_primary_monitor() {
      return;
    }
    if workspace_id.workspace > self.workspace_tray_icons.len() {
      error!(
        "Workspace ID [{}] is out of bounds for tray icons (max: [{}]) - ignoring request",
        workspace_id.workspace,
        self.workspace_tray_icons.len()
      );
      return;
    }
    WORKSPACE.store(workspace_id.workspace as u8, std::sync::atomic::Ordering::Relaxed);
    if IS_DRAG_ICON_SHOWN.load(std::sync::atomic::Ordering::Relaxed) {
      debug!("Not updating tray icon because window drag icon is shown");
      return;
    }
    let icon = &self.workspace_tray_icons[workspace_id.workspace - 1];
    let tray_icon = Arc::clone(self.menu.as_ref().unwrap());
    if let Err(err) = tray_icon.lock().expect(TRAY_ICON_LOCK).set_icon(icon) {
      error!("Failed to set workspace tray icon to [{}]: {err}", workspace_id.workspace);
    } else {
      debug!(
        "Set tray icon [{}/{}] to reflect active workspace on primary monitor",
        workspace_id.workspace,
        self.workspace_tray_icons.len()
      );
    }
  }

  pub fn set_window_drag_icon(&self, is_enabled: bool) {
    let tray_icon = Arc::clone(self.menu.as_ref().unwrap());
    let icon = if is_enabled {
      IS_DRAG_ICON_SHOWN.store(true, std::sync::atomic::Ordering::Relaxed);

      &self.drag_icon
    } else {
      IS_DRAG_ICON_SHOWN.store(false, std::sync::atomic::Ordering::Relaxed);

      &self.workspace_tray_icons[WORKSPACE.load(std::sync::atomic::Ordering::Relaxed) as usize - 1]
    };
    if let Err(err) = tray_icon.lock().expect(TRAY_ICON_LOCK).set_icon(icon) {
      error!("Failed to set window drag tray icon to [{is_enabled}]: {err}");
    } else {
      debug!("Set window drag icon to [{}]", is_enabled);
    }
  }
}

fn unlocked_config_provider(config_provider: &Arc<Mutex<ConfigurationProvider>>) -> MutexGuard<ConfigurationProvider> {
  config_provider.lock().expect(CONFIGURATION_PROVIDER_LOCK)
}

fn build_menu(config_provider: &Arc<Mutex<ConfigurationProvider>>) -> MenuBuilder<Event> {
  let config = unlocked_config_provider(config_provider);
  let current_margin: i32 = config.get_i32(WINDOW_MARGIN);
  let icon_bytes = include_bytes!("../assets/randolf.ico");

  MenuBuilder::new()
    .with(MenuItem::Item {
      name: format!("Randolf v{}", env!("CARGO_PKG_VERSION")),
      disabled: true,
      id: Event::DisabledItem,
      icon: Some(Icon::from_buffer(icon_bytes, Some(32), Some(32)).unwrap()),
    })
    .separator()
    .submenu(
      "Explore debug settings",
      MenuBuilder::new().item("Print monitor layout to log file", Event::LogMonitorLayout),
    )
    .separator()
    .submenu(
      "Set window margin to...",
      MenuBuilder::new()
        .checkable("10 px", 10 == current_margin, Event::SetMargin(10))
        .checkable("15 px", 15 == current_margin, Event::SetMargin(15))
        .checkable("20 px (default)", 20 == current_margin, Event::SetMargin(20))
        .checkable("30 px", 30 == current_margin, Event::SetMargin(30))
        .checkable("40 px", 40 == current_margin, Event::SetMargin(40))
        .checkable("50 px", 50 == current_margin, Event::SetMargin(50))
        .checkable("75 px", 75 == current_margin, Event::SetMargin(75))
        .checkable("100 px", 100 == current_margin, Event::SetMargin(100))
        .checkable("150 px", 150 == current_margin, Event::SetMargin(150)),
    )
    .separator()
    .checkable(
      "Allow selecting same center windows",
      config.get_bool(ALLOW_SELECTING_SAME_CENTER_WINDOWS),
      Event::ToggleSelectingSameCenterWindows,
    )
    .checkable(
      "Force using admin privileges",
      config.get_bool(FORCE_USING_ADMIN_PRIVILEGES),
      Event::ToggleForceUsingAdminPrivileges,
    )
    .separator()
    .item("Open executable folder", Event::OpenRandolfExecutableFolder)
    .item("Open config folder", Event::OpenRandolfConfigFolder)
    .item("Open data folder", Event::OpenRandolfDataFolder)
    .item("Restart with admin privileges", Event::RestartRandolf(true))
    .item("Restart", Event::RestartRandolf(false))
    .item("Exit (restores any hidden windows)", Event::Exit)
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::configuration_provider::ConfigurationProvider;
  use std::sync::{Arc, Mutex};

  #[test]
  fn new_initialised_returns_initialised_tray_menu_manager() {
    let configuration_provider = Arc::new(Mutex::new(ConfigurationProvider::new()));
    let command_sender = unbounded().0;
    let tray_menu_manager = TrayMenuManager::new_initialised(configuration_provider.clone(), command_sender);

    assert!(tray_menu_manager.menu.is_some());
    assert_eq!(tray_menu_manager.workspace_tray_icons.len(), 9);
  }

  #[test]
  fn update_tray_icon_sets_icon_for_primary_monitor_workspace() {
    testing_logger::setup();
    let configuration_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
    let manager = TrayMenuManager::new_initialised(configuration_provider, unbounded().0);

    let workspace_id = PersistentWorkspaceId::new_test(1);
    manager.update_tray_icon(workspace_id);

    testing_logger::validate(|captured_logs| {
      assert_eq!(captured_logs.len(), 2);
      assert_eq!(
        captured_logs[1].body,
        format!(
          "Set tray icon [{}/{}] to reflect active workspace on primary monitor",
          workspace_id.workspace,
          manager.workspace_tray_icons.len()
        )
      );
    });
  }

  #[test]
  fn update_tray_icon_does_not_set_icon_for_non_primary_monitor_workspaces() {
    testing_logger::setup();
    let configuration_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
    let manager = TrayMenuManager::new_initialised(configuration_provider, unbounded().0);

    manager.update_tray_icon(PersistentWorkspaceId::new([1; 32], 1, false));
    manager.update_tray_icon(PersistentWorkspaceId::new([2; 32], 2, false));
    manager.update_tray_icon(PersistentWorkspaceId::new([3; 32], 3, false));

    testing_logger::validate(|captured_logs| {
      assert_eq!(captured_logs.len(), 1);
      assert!(!captured_logs[0].body.contains("Set tray icon"));
    });
  }

  #[test]
  fn update_tray_icon_ignores_request_when_index_out_of_bounds() {
    testing_logger::setup();
    let configuration_provider = Arc::new(Mutex::new(ConfigurationProvider::default()));
    let manager = TrayMenuManager::new_initialised(configuration_provider, unbounded().0);

    manager.update_tray_icon(PersistentWorkspaceId::new_test(123));

    testing_logger::validate(|captured_logs| {
      assert_eq!(captured_logs.len(), 2);
      assert_eq!(
        captured_logs[1].body,
        "Workspace ID [123] is out of bounds for tray icons (max: [9]) - ignoring request"
      );
    });
  }

  #[test]
  fn create_icon_creates_icon() {
    let icon = TrayMenuManager::create_icon(4);
    assert_eq!(
      icon,
      Icon::from_buffer(include_bytes!("../assets/randolf-4.ico"), Some(32), Some(32),).unwrap()
    );
  }

  #[test]
  #[should_panic(expected = "Invalid icon index")]
  fn create_icon_panics_for_invalid_index() {
    TrayMenuManager::create_icon(10);
  }

  #[test]
  fn create_icon_creates_different_icons_for_different_indices() {
    let icon1 = TrayMenuManager::create_icon(1);
    let icon2 = TrayMenuManager::create_icon(2);
    assert_ne!(icon1, icon2);
  }
}
