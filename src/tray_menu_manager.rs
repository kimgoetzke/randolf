use crossbeam_channel::{Receiver, Sender, unbounded};
use std::thread;
use trayicon::*;

pub(crate) struct TrayMenuManager;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Event {
  RightClickTrayIcon,
  LeftClickTrayIcon,
  DoubleClickTrayIcon,
  Exit,
  DisabledItem,
}

impl TrayMenuManager {
  pub fn new() -> Self {
    debug!("Creating tray icon...");
    let (tx, rx) = unbounded();
    let tray_icon = Self::create_tray_icon(tx);
    Self::initialise(rx, tray_icon);

    Self {}
  }

  fn create_tray_icon(tx: Sender<Event>) -> TrayIcon<Event> {
    let icon_bytes = include_bytes!("../assets/icon.ico");
    let version = env!("CARGO_PKG_VERSION");

    TrayIconBuilder::new()
      .sender(move |e| {
        let _ = tx.send(*e);
      })
      .icon_from_buffer(icon_bytes)
      .tooltip("Randolf")
      .on_right_click(Event::RightClickTrayIcon)
      .on_click(Event::LeftClickTrayIcon)
      .on_double_click(Event::DoubleClickTrayIcon)
      .menu(
        MenuBuilder::new()
          .with(MenuItem::Item {
            name: format!("Randolf v{version}"),
            disabled: true,
            id: Event::DisabledItem,
            icon: None,
          })
          .separator()
          .item("Exit  👋", Event::Exit),
      )
      .build()
      .expect("Failed to build tray icon")
  }

  fn initialise(rx: Receiver<Event>, mut tray_icon: TrayIcon<Event>) {
    thread::spawn(move || {
      rx.iter().for_each(|m| match m {
        Event::RightClickTrayIcon => {
          tray_icon.show_menu().expect("Failed to open tray menu");
        }
        Event::DoubleClickTrayIcon => {
          debug!("Double clicking tray icon is not implemented");
        }
        Event::LeftClickTrayIcon => {
          tray_icon.show_menu().expect("Failed to open tray menu");
        }
        Event::Exit => {
          info!("Exit application...");
          std::process::exit(0);
        }
        e => {
          debug!("Received unhandled tray menu event: {:?}", e);
        }
      })
    });
  }
}
