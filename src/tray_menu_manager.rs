use crate::configuration_provider::{
  ALLOW_SELECTING_SAME_CENTER_WINDOWS, ConfigurationProvider, FILE_LOGGING_ENABLED, WINDOW_MARGIN,
};
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use trayicon::*;

pub struct TrayMenuManager {
  configuration_provider: Arc<Mutex<ConfigurationProvider>>,
  menu: Option<Arc<Mutex<TrayIcon<Event>>>>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Event {
  RightClickTrayIcon,
  LeftClickTrayIcon,
  DoubleClickTrayIcon,
  Exit,
  DisabledItem,
  ToggleLogging,
  SetMargin(i32),
  ToggleSelectingSameCenterWindows,
}

impl TrayMenuManager {
  fn new(configuration_provider: Arc<Mutex<ConfigurationProvider>>) -> Self {
    Self {
      configuration_provider,
      menu: None,
    }
  }

  pub fn new_initialised(configuration_provider: Arc<Mutex<ConfigurationProvider>>) -> Self {
    let mut manager = Self::new(configuration_provider);
    let (tx, rx) = unbounded();
    let tray = manager.create_tray_icon(tx);
    manager.menu = Some(Arc::from(Mutex::new(tray)));
    manager.initialise(rx);
    debug!("Created tray icon & menu");

    manager
  }

  fn create_tray_icon(&mut self, tx: Sender<Event>) -> TrayIcon<Event> {
    let icon_bytes = include_bytes!("../assets/icon.ico");
    let version = env!("CARGO_PKG_VERSION");
    let configuration = self.configuration_provider.lock().expect("Configuration provider is locked");

    TrayIconBuilder::new()
      .sender(move |e| {
        let _ = tx.send(*e);
      })
      .icon_from_buffer(icon_bytes)
      .tooltip("Randolf")
      .on_right_click(Event::RightClickTrayIcon)
      .on_click(Event::LeftClickTrayIcon)
      .on_double_click(Event::DoubleClickTrayIcon)
      .menu(self.menu_builder(version, configuration))
      .build()
      .expect("Failed to build tray icon")
  }

  fn menu_builder(&self, version: &str, config: MutexGuard<ConfigurationProvider>) -> MenuBuilder<Event> {
    let current_margin: i32 = config.get_i32(WINDOW_MARGIN);

    MenuBuilder::new()
      .with(MenuItem::Item {
        name: format!("Randolf v{version}"),
        disabled: true,
        id: Event::DisabledItem,
        icon: None,
      })
      .separator()
      .submenu(
        "Set window margin to...",
        MenuBuilder::new()
          .checkable("5", 5 == current_margin, Event::SetMargin(5))
          .checkable("10", 10 == current_margin, Event::SetMargin(10))
          .checkable("15", 15 == current_margin, Event::SetMargin(15))
          .checkable("20 (default)", 20 == current_margin, Event::SetMargin(20))
          .checkable("30", 30 == current_margin, Event::SetMargin(30))
          .checkable("40", 40 == current_margin, Event::SetMargin(40))
          .checkable("50", 50 == current_margin, Event::SetMargin(50)),
      )
      .checkable(
        "Allow selecting same center windows",
        config.get_bool(ALLOW_SELECTING_SAME_CENTER_WINDOWS),
        Event::ToggleSelectingSameCenterWindows,
      )
      .checkable(
        "Enable file logging",
        config.get_bool(FILE_LOGGING_ENABLED),
        Event::ToggleLogging,
      )
      .item("Exit  👋", Event::Exit)
  }

  // TODO: Update margins of "known" windows when the margin is changed
  fn initialise(&self, rx: Receiver<Event>) {
    let tray_icon = Arc::clone(self.menu.as_ref().unwrap());
    let config_provider = self.configuration_provider.clone();
    thread::spawn(move || {
      rx.iter().for_each(|m| match m {
        Event::RightClickTrayIcon => {
          tray_icon
            .lock()
            .expect("Failed to lock tray icon")
            .show_menu()
            .expect("Failed to open tray menu");
        }
        Event::DoubleClickTrayIcon => {
          debug!("Double clicking tray icon is not implemented");
        }
        Event::LeftClickTrayIcon => {
          tray_icon
            .lock()
            .expect("Failed to lock tray icon")
            .show_menu()
            .expect("Failed to open tray menu");
        }
        Event::SetMargin(margin) => {
          let mut config = unlocked_config_provider(&config_provider);
          if config.get_i32(WINDOW_MARGIN) != margin {
            let mut tray_icon = tray_icon.lock().expect("Failed to lock tray icon");
            tray_icon
              .set_menu_item_checkable(Event::SetMargin(margin), true)
              .expect("Failed to toggle menu item");
            tray_icon
              .set_menu_item_checkable(Event::SetMargin(config.get_i32(WINDOW_MARGIN)), false)
              .expect("Failed to toggle menu item");
            config.set_i32(WINDOW_MARGIN, margin);
            debug!("Set window margin to [{}]", margin);
          }
        }
        Event::ToggleSelectingSameCenterWindows => {
          let mut config = unlocked_config_provider(&config_provider);
          let is_enabled = config.get_bool(ALLOW_SELECTING_SAME_CENTER_WINDOWS);
          if let Err(result) = tray_icon
            .lock()
            .expect("Failed to lock tray icon")
            .set_menu_item_checkable(Event::ToggleSelectingSameCenterWindows, !is_enabled)
          {
            error!("Failed to toggle menu item: {result}");
          }
          config.set_bool(ALLOW_SELECTING_SAME_CENTER_WINDOWS, !is_enabled);
          debug!("Set [{:?}] to [{}]", Event::ToggleSelectingSameCenterWindows, !is_enabled);
        }
        Event::ToggleLogging => {
          let mut config = unlocked_config_provider(&config_provider);
          let is_enabled = config.get_bool(FILE_LOGGING_ENABLED);
          if let Err(result) = tray_icon
            .lock()
            .expect("Failed to lock tray icon")
            .set_menu_item_checkable(Event::ToggleLogging, !is_enabled)
          {
            error!("Failed to toggle menu item: {result}");
          }
          config.set_bool(FILE_LOGGING_ENABLED, !is_enabled);
          debug!("Set [{:?}] to [{}] - REQUIRES RESTART", Event::ToggleLogging, !is_enabled);
        }
        Event::Exit => {
          info!("Exit application...");
          std::process::exit(0);
        }
        e => {
          error!("Received unhandled tray menu event: {:?}", e);
        }
      })
    });
  }
}

fn unlocked_config_provider(config_provider: &Arc<Mutex<ConfigurationProvider>>) -> MutexGuard<ConfigurationProvider> {
  config_provider.lock().expect("Failed to lock configuration provider")
}
