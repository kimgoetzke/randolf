use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, TrayIconBuilder, TrayIconEvent};

pub(crate) struct TrayMenuManager;

impl TrayMenuManager {
  pub fn new() -> Self {
    tray_icon();
    Self
  }
}

// TODO: Allow closing the application from the tray menu
fn tray_icon() {
  std::thread::spawn(move || {
    debug!("Creating tray icon...");
    let icon = Icon::from_path("assets/icon.ico", Some((32, 32))).expect("Failed to load icon");
    let menu = create_tray_menu();
    let _tray = TrayIconBuilder::new()
      .with_menu(Box::new(menu))
      .with_tooltip("Randolf")
      .with_icon(icon)
      .with_menu_on_left_click(true)
      .build()
      .expect("Failed to build tray icon");

    loop {
      if let Ok(event) = MenuEvent::receiver().try_recv() {
        debug!("Received tray menu event: {:?}", event);
      }
      std::thread::sleep(std::time::Duration::from_millis(100));
      if let Ok(event) = TrayIconEvent::receiver().try_recv() {
        println!("Received tray icon event: {:?}", event);
      }
      std::thread::sleep(std::time::Duration::from_millis(100));
    }
  });
}

fn create_tray_menu() -> Menu {
  let menu = Menu::new();
  let exit = MenuItem::new("Exit", true, None);
  if let Err(err) = menu.append(&exit) {
    error!("Error during menu creation: {err:?}");
  }

  menu
}
