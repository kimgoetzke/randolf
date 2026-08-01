use super::hotkey_outcome::HotkeyOutcome;
use super::key_release_hook::KeyReleaseHook;
use super::press_latch::PressLatch;
use crate::common::{Command, Direction, PersistentWorkspaceId};
use crate::configuration::ConfigurationProvider;
use crate::utils::CONFIGURATION_PROVIDER_LOCK;
use crossbeam_channel::{Receiver, Sender};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;
use win_hotkeys::{InterruptHandle, VKey};

const BACKSLASH: u32 = 0xDC;
const MAIN_MOD: VKey = VKey::LWin;
const SECONDARY_MOD: VKey = VKey::Shift;
const TERTIARY_MOD: VKey = VKey::Control;

/// Registers Randolf's global shortcuts and runs their keyboard event loop.
///
/// `win_hotkeys` matches low-level Windows keyboard events, invokes each registered callback, then sends its
/// [`HotkeyOutcome`] through a channel for dispatch.
pub struct HotkeyManager {
  hkm: win_hotkeys::HotkeyManager<HotkeyOutcome>,
  configuration_provider: Arc<Mutex<ConfigurationProvider>>,
  application_latches: HashMap<u32, PressLatch>,
  key_release_hook: Option<KeyReleaseHook>,
}

impl HotkeyManager {
  /// Creates an empty manager whose custom shortcuts come from `configuration_provider`.
  fn new(configuration_provider: Arc<Mutex<ConfigurationProvider>>) -> Self {
    Self {
      hkm: win_hotkeys::HotkeyManager::new(),
      configuration_provider,
      application_latches: HashMap::new(),
      key_release_hook: None,
    }
  }

  /// Creates a manager with all built-in and configured application shortcuts.
  ///
  /// `workspace_ids` are assigned number keys in order. Registration failures panic because the application cannot
  /// safely run with only some shortcuts active.
  pub fn new_with_hotkeys(
    configuration_provider: Arc<Mutex<ConfigurationProvider>>,
    workspace_ids: Vec<PersistentWorkspaceId>,
  ) -> Self {
    let mut hotkey_manager = HotkeyManager::new(configuration_provider.clone());

    // Move cursor
    hotkey_manager.register_move_cursor_hotkey(Direction::Left, VKey::Left);
    hotkey_manager.register_move_cursor_hotkey(Direction::Down, VKey::Down);
    hotkey_manager.register_move_cursor_hotkey(Direction::Up, VKey::Up);
    hotkey_manager.register_move_cursor_hotkey(Direction::Right, VKey::Right);

    // Move window
    hotkey_manager.register_move_window_hotkey(Direction::Left, VKey::Left);
    hotkey_manager.register_move_window_hotkey(Direction::Down, VKey::Down);
    hotkey_manager.register_move_window_hotkey(Direction::Up, VKey::Up);
    hotkey_manager.register_move_window_hotkey(Direction::Right, VKey::Right);
    hotkey_manager.register_move_window_hotkey(Direction::Left, VKey::H);
    hotkey_manager.register_move_window_hotkey(Direction::Down, VKey::J);
    hotkey_manager.register_move_window_hotkey(Direction::Up, VKey::K);
    hotkey_manager.register_move_window_hotkey(Direction::Right, VKey::L);

    // Resize window
    hotkey_manager.register_resize_spatial_window_hotkey(Direction::Left, VKey::Left);
    hotkey_manager.register_resize_spatial_window_hotkey(Direction::Down, VKey::Down);
    hotkey_manager.register_resize_spatial_window_hotkey(Direction::Up, VKey::Up);
    hotkey_manager.register_resize_spatial_window_hotkey(Direction::Right, VKey::Right);
    hotkey_manager.register_resize_spatial_window_hotkey(Direction::Left, VKey::H);
    hotkey_manager.register_resize_spatial_window_hotkey(Direction::Down, VKey::J);
    hotkey_manager.register_resize_spatial_window_hotkey(Direction::Up, VKey::K);
    hotkey_manager.register_resize_spatial_window_hotkey(Direction::Right, VKey::L);

    // Resize scrolling layout window, globally overriding Windows virtual-desktop switching
    hotkey_manager.register_resize_scrolling_window_hotkey(Direction::Left, VKey::Left);
    hotkey_manager.register_resize_scrolling_window_hotkey(Direction::Right, VKey::Right);

    // Other window management
    hotkey_manager.register_close_window_hotkey(VKey::Q);
    hotkey_manager.register_near_maximise_window_hotkey(VKey::CustomKeyCode(BACKSLASH as u16));
    hotkey_manager.register_minimise_window_hotkey(VKey::CustomKeyCode(BACKSLASH as u16));

    // Workspace management
    hotkey_manager.register_switch_workspace_hotkeys(&workspace_ids);
    hotkey_manager.register_move_window_to_workspace_hotkeys(&workspace_ids);

    // Launch applications
    hotkey_manager.register_application_hotkeys();

    hotkey_manager
  }

  /// Starts hotkey processing on background threads and returns its stop handle.
  ///
  /// The returned [`InterruptHandle`] stops the blocking `win_hotkeys` event loop. Configured application shortcuts
  /// also start a Windows key-release hook so holding a key launches the application only once.
  pub fn initialise(mut self, command_sender: Sender<Command>) -> InterruptHandle {
    if !self.application_latches.is_empty() {
      self.key_release_hook = Some(
        KeyReleaseHook::start(std::mem::take(&mut self.application_latches))
          .unwrap_or_else(|error| panic!("Failed to initialise application hotkey release hook: {error}")),
      );
    }

    let (hotkey_outcome_sender, hotkey_outcome_receiver) = crossbeam_channel::unbounded();
    self.hkm.register_channel(hotkey_outcome_sender);
    let interrupt_handle = self.hkm.interrupt_handle();
    thread::spawn(move || {
      forward_hotkey_outcomes(hotkey_outcome_receiver, command_sender);
    });
    thread::spawn(move || {
      self.hkm.event_loop();
    });

    interrupt_handle
  }

  /// Creates the binding to near-maximise the focused window.
  fn register_near_maximise_window_hotkey(&mut self, key: VKey) {
    self
      .hkm
      .register_hotkey(key, &[MAIN_MOD], || HotkeyOutcome::Accepted(Command::NearMaximiseWindow))
      .unwrap_or_else(|err| panic!("Failed to register hotkey for {:?}: {err}", Command::NearMaximiseWindow));
  }

  /// Creates the binding to minimise the focused window.
  fn register_minimise_window_hotkey(&mut self, key: VKey) {
    self
      .hkm
      .register_hotkey(key, &[MAIN_MOD, SECONDARY_MOD], || {
        HotkeyOutcome::Accepted(Command::MinimiseWindow)
      })
      .unwrap_or_else(|err| panic!("Failed to register hotkey for {:?}: {err}", Command::MinimiseWindow));
  }

  /// Creates the binding to close the focused window.
  fn register_close_window_hotkey(&mut self, key: VKey) {
    self
      .hkm
      .register_hotkey(key, &[MAIN_MOD, SECONDARY_MOD], || {
        HotkeyOutcome::Accepted(Command::CloseWindow)
      })
      .unwrap_or_else(|err| panic!("Failed to register hotkey for {:?}: {err}", Command::CloseWindow));
  }

  /// Creates the bindings to switch to a configured workspace, in display order, e.g. `MAIN_MOD + 1..8`.
  ///
  /// Workspaces from the ninth onwards remain unbound because only eight number-key shortcuts are supported.
  fn register_switch_workspace_hotkeys(&mut self, workspace_ids: &[PersistentWorkspaceId]) {
    for (i, workspace_id) in workspace_ids.iter().enumerate() {
      let key_number = i + 1;
      if key_number >= 9 {
        warn!(
          "Cannot bind workspace number [{}] to a hotkey because it is greater than 8",
          key_number
        );
        continue;
      }
      match VKey::from_keyname(key_number.to_string().as_str()) {
        Ok(key) => {
          self.register_switch_workspace_hotkey(key, workspace_id);
        }
        Err(err) => {
          warn!("Failed to parse workspace hotkey [{}]: {err}", i);
          continue;
        }
      }
      trace!(
        "Registered hotkey [{}] + [{}] to switch to workspace [{}]",
        MAIN_MOD, key_number, workspace_id
      );
    }
  }

  /// Creates the binding to switch to `workspace_id`.
  fn register_switch_workspace_hotkey(&mut self, key: VKey, workspace_id: &PersistentWorkspaceId) {
    let id = *workspace_id;
    self
      .hkm
      .register_hotkey(key, &[MAIN_MOD], move || {
        HotkeyOutcome::Accepted(Command::SwitchWorkspace(id))
      })
      .unwrap_or_else(|err| panic!("Failed to register hotkey for {:?}: {err}", Command::SwitchWorkspace(id)));
  }

  /// Creates the bindings to move a window to a configured workspace, in display order e.g.
  /// `MAIN_MOD + SECONDARY_MOD + 1..8`.
  fn register_move_window_to_workspace_hotkeys(&mut self, workspace_ids: &[PersistentWorkspaceId]) {
    for (i, workspace_id) in workspace_ids.iter().enumerate() {
      let key_number = i + 1;
      if key_number >= 9 {
        warn!(
          "Cannot bind workspace number [{}] to a hotkey because it is greater than 8",
          key_number
        );
        continue;
      }
      match VKey::from_keyname(key_number.to_string().as_str()) {
        Ok(key) => {
          self.register_move_window_to_workspace_hotkey(key, workspace_id);
        }
        Err(err) => {
          warn!("Failed to parse workspace hotkey [{}]: {err}", i);
          continue;
        }
      }
      trace!(
        "Registered hotkey [{}] + [{}] + [{}] to move foreground window to workspace [{}]",
        MAIN_MOD, SECONDARY_MOD, key_number, workspace_id
      );
    }
  }

  /// Creates the binding to move the focused window to `workspace_id`.
  fn register_move_window_to_workspace_hotkey(&mut self, key: VKey, workspace_id: &PersistentWorkspaceId) {
    let id = *workspace_id;
    self
      .hkm
      .register_hotkey(key, &[MAIN_MOD, SECONDARY_MOD], move || {
        HotkeyOutcome::Accepted(Command::MoveWindowToWorkspace(id))
      })
      .unwrap_or_else(|err| {
        panic!(
          "Failed to register hotkey for {:?}: {err}",
          Command::MoveWindowToWorkspace(id)
        )
      });
  }

  /// Loads application shortcuts from configuration, skips invalid `VKey` names, then creates the bindings.
  fn register_application_hotkeys(&mut self) {
    let config_provider = self.configuration_provider.clone();
    for hotkey in config_provider.lock().expect(CONFIGURATION_PROVIDER_LOCK).get_hotkeys() {
      match VKey::from_str(&hotkey.hotkey) {
        Ok(key) => {
          self.register_application_hotkey(&hotkey.name, &hotkey.path, key, hotkey.execute_as_admin);
        }
        Err(err) => {
          warn!("Failed to parse hotkey [{}] for [{}]: {err}", hotkey.hotkey, hotkey.name);
          continue;
        }
      }
    }
  }

  /// Creates the binding to launch an application once per physical key press.
  ///
  /// The key's Windows virtual-key code associates the callback's latch with release events received by
  /// [`KeyReleaseHook`].
  fn register_application_hotkey(&mut self, name: &str, path: &str, key: VKey, open_as_admin: bool) {
    let latch = PressLatch::default();
    self
      .hkm
      .register_hotkey(
        key,
        &[MAIN_MOD],
        application_hotkey_callback(path.to_string(), open_as_admin, latch.clone()),
      )
      .unwrap_or_else(|err| {
        panic!(
          "Failed to register hotkey for {:?}: {err}",
          Command::OpenApplication(name.to_string(), open_as_admin)
        )
      });
    self.application_latches.insert(key.to_vk_code().into(), latch);
    debug!(
      "Registered hotkey for [{}] to open [{}] as admin [{}]",
      name, path, open_as_admin
    );
  }

  /// Creates the binding to focus the neighbouring window in a given [`Direction`].
  fn register_move_cursor_hotkey(&mut self, direction: Direction, key: VKey) {
    self
      .hkm
      .register_hotkey(key, &[MAIN_MOD], move || {
        HotkeyOutcome::Accepted(Command::MoveCursor(direction))
      })
      .unwrap_or_else(|err| panic!("Failed to register hotkey for {:?}: {err}", Command::MoveCursor(direction)));
  }

  /// Creates the binding to move the focused window.
  fn register_move_window_hotkey(&mut self, direction: Direction, key: VKey) {
    self
      .hkm
      .register_hotkey(key, &[MAIN_MOD, VKey::Shift], move || {
        HotkeyOutcome::Accepted(Command::MoveWindow(direction))
      })
      .unwrap_or_else(|err| panic!("Failed to register hotkey for {:?}: {err}", Command::MoveWindow(direction)));
  }

  /// Creates the binding to resize a spatial-layout window.
  fn register_resize_spatial_window_hotkey(&mut self, direction: Direction, key: VKey) {
    self
      .hkm
      .register_hotkey(key, &[MAIN_MOD, SECONDARY_MOD, TERTIARY_MOD], move || {
        HotkeyOutcome::Accepted(Command::ResizeSpatialWindow(direction))
      })
      .unwrap_or_else(|err| {
        panic!(
          "Failed to register hotkey for {:?}: {err}",
          Command::ResizeSpatialWindow(direction)
        )
      });
  }

  /// Creates the binding to resize a scrolling-layout window.
  ///
  /// Windows may reserve these combinations for virtual-desktop switching, and in that case `win_hotkeys` intercepts
  /// them before Windows handles them.
  fn register_resize_scrolling_window_hotkey(&mut self, direction: Direction, key: VKey) {
    self
      .hkm
      .register_hotkey(key, &[MAIN_MOD, TERTIARY_MOD], move || {
        HotkeyOutcome::Accepted(Command::ResizeScrollingWindow(direction))
      })
      .unwrap_or_else(|err| {
        panic!(
          "Failed to register hotkey for {:?}: {err}",
          Command::ResizeScrollingWindow(direction)
        )
      });
  }
}

/// Forwards dispatchable callback results to the main command loop.
///
/// Suppressed key-repeat results are discarded. Forwarding stops when either input closes or the main command receiver
/// has been dropped.
fn forward_hotkey_outcomes(outcome_receiver: Receiver<HotkeyOutcome>, command_sender: Sender<Command>) {
  for outcome in outcome_receiver {
    if let HotkeyOutcome::Accepted(command) = outcome
      && command_sender.send(command).is_err()
    {
      return;
    }
  }
}

/// Builds a callback that launches once until its key is released.
fn application_hotkey_callback(
  path: String,
  open_as_admin: bool,
  latch: PressLatch,
) -> impl Fn() -> HotkeyOutcome + Send + 'static {
  move || {
    if !latch.try_press() {
      return HotkeyOutcome::Suppressed;
    }

    HotkeyOutcome::Accepted(Command::OpenApplication(path.clone(), open_as_admin))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::configuration::CustomHotkey;
  use log::Level::{Debug, Warn};

  #[test]
  fn application_hotkey_bindings_have_independent_press_latches() {
    let first = application_hotkey_callback("first.exe".to_string(), false, PressLatch::default());
    let second = application_hotkey_callback("second.exe".to_string(), false, PressLatch::default());

    assert!(matches!(first(), HotkeyOutcome::Accepted(Command::OpenApplication(path, false)) if path == "first.exe"));
    assert!(matches!(first(), HotkeyOutcome::Suppressed));
    assert!(matches!(second(), HotkeyOutcome::Accepted(Command::OpenApplication(path, false)) if path == "second.exe"));
  }

  #[test]
  fn physical_key_release_allows_the_next_application_activation() {
    let latch = PressLatch::default();
    let callback = application_hotkey_callback("app.exe".to_string(), false, latch.clone());
    let latches = HashMap::from([(u32::from(VKey::F.to_vk_code()), latch)]);

    assert!(matches!(callback(), HotkeyOutcome::Accepted(_)));
    assert!(matches!(callback(), HotkeyOutcome::Suppressed));

    super::super::key_release_hook::rearm_released_key(&latches, VKey::F.to_vk_code().into());

    assert!(matches!(callback(), HotkeyOutcome::Accepted(_)));
  }

  #[test]
  fn suppressed_hotkey_outcomes_are_not_forwarded() {
    let (outcome_sender, outcome_receiver) = crossbeam_channel::unbounded();
    let (command_sender, command_receiver) = crossbeam_channel::unbounded();
    outcome_sender.send(HotkeyOutcome::Suppressed).unwrap();
    drop(outcome_sender);

    forward_hotkey_outcomes(outcome_receiver, command_sender);

    assert!(command_receiver.is_empty());
  }

  #[test]
  fn dispatched_hotkey_outcomes_are_forwarded_in_order() {
    let (outcome_sender, outcome_receiver) = crossbeam_channel::unbounded();
    let (command_sender, command_receiver) = crossbeam_channel::unbounded();
    outcome_sender.send(HotkeyOutcome::Accepted(Command::CloseWindow)).unwrap();
    outcome_sender.send(HotkeyOutcome::Suppressed).unwrap();
    outcome_sender.send(HotkeyOutcome::Accepted(Command::MinimiseWindow)).unwrap();
    drop(outcome_sender);

    forward_hotkey_outcomes(outcome_receiver, command_sender);

    assert!(matches!(command_receiver.recv().unwrap(), Command::CloseWindow));
    assert!(matches!(command_receiver.recv().unwrap(), Command::MinimiseWindow));
    assert!(command_receiver.is_empty());
  }

  #[test]
  fn registers_switch_workspace_hotkeys_for_valid_workspace_ids() {
    testing_logger::setup();
    let mut hotkey_manager = HotkeyManager::new(Arc::new(Mutex::new(ConfigurationProvider::default())));
    let workspace_ids = vec![
      PersistentWorkspaceId::new_test(1),
      PersistentWorkspaceId::new_test(2),
      PersistentWorkspaceId::new_test(3),
    ];

    hotkey_manager.register_switch_workspace_hotkeys(&workspace_ids);

    testing_logger::validate(|captured_logs| {
      assert_eq!(captured_logs.len(), 3);
      for (i, _) in captured_logs.iter().enumerate() {
        assert_eq!(
          captured_logs[i].body,
          format!(
            "Registered hotkey [{}] + [{}] to switch to workspace [wsp#P_DISPLAY-{}]",
            MAIN_MOD,
            i + 1,
            i + 1
          )
        );
      }
    });
  }

  #[test]
  fn register_switch_workspace_hotkeys_skips_workspace_ids_greater_than_9() {
    testing_logger::setup();
    let mut hotkey_manager = HotkeyManager::new(Arc::new(Mutex::new(ConfigurationProvider::default())));
    let mut workspace_ids = vec![];
    for i in 1..=9 {
      workspace_ids.push(PersistentWorkspaceId::new_test(i));
    }

    hotkey_manager.register_switch_workspace_hotkeys(&workspace_ids);

    testing_logger::validate(|captured_logs| {
      assert_eq!(captured_logs.len(), 9);
      assert_eq!(
        captured_logs[8].body,
        "Cannot bind workspace number [9] to a hotkey because it is greater than 8"
      );
      assert_eq!(captured_logs[8].level, Warn);
    });
  }

  #[test]
  fn register_application_hotkeys_test() {
    testing_logger::setup();
    let hotkeys = vec![
      CustomHotkey {
        name: "Test App 1".to_string(),
        path: "C:\\test1.exe".to_string(),
        hotkey: "y".to_string(),
        execute_as_admin: true,
      },
      CustomHotkey {
        name: "Test App 2".to_string(),
        path: "C:\\test2.exe".to_string(),
        hotkey: "invalid".to_string(),
        execute_as_admin: true,
      },
    ];
    let custom_config = ConfigurationProvider::default_with_hotkeys(hotkeys);
    let mut hotkey_manager = HotkeyManager::new(Arc::new(Mutex::new(custom_config)));

    hotkey_manager.register_application_hotkeys();

    testing_logger::validate(|captured_logs| {
      assert_eq!(captured_logs.len(), 2);
      assert_eq!(
        captured_logs[0].body,
        "Registered hotkey for [Test App 1] to open [C:\\test1.exe] as admin [true]"
      );
      assert_eq!(captured_logs[0].level, Debug);
      assert_eq!(
        captured_logs[1].body,
        "Failed to parse hotkey [invalid] for [Test App 2]: Invalid key name `INVALID`"
      );
      assert_eq!(captured_logs[1].level, Warn);
    });
  }
}
