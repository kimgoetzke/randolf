use super::press_latch::PressLatch;
use crossbeam_channel::{Sender, bounded};
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};
use std::thread::{self, JoinHandle};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
  CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, PM_NOREMOVE, PeekMessageW,
  PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYUP, WM_QUIT,
  WM_SYSKEYUP,
};

/// Shares per-key latches with the Windows callback, which cannot receive Rust context.
static RELEASE_LATCHES: LazyLock<RwLock<HashMap<u32, PressLatch>>> = LazyLock::new(|| RwLock::new(HashMap::new()));

/// Owns a Windows low-level keyboard hook and the thread pumping its message queue.
///
/// Windows invokes the hook callback for system-wide keyboard events. Only physical key releases registered in
/// [`RELEASE_LATCHES`] affect application shortcuts.
pub(super) struct KeyReleaseHook {
  thread_id: u32,
  thread: Option<JoinHandle<()>>,
}

impl KeyReleaseHook {
  /// Installs the release hook on a dedicated thread and waits until it is ready.
  ///
  /// Returns an error if Windows rejects the hook or the thread stops during start-up.
  pub(super) fn start(latches: HashMap<u32, PressLatch>) -> Result<Self, String> {
    *RELEASE_LATCHES.write().unwrap_or_else(|poisoned| poisoned.into_inner()) = latches;

    let (ready_sender, ready_receiver) = bounded(1);
    let thread = thread::spawn(move || install_key_release_hook(ready_sender));
    match ready_receiver.recv() {
      Ok(Ok(thread_id)) => Ok(Self {
        thread_id,
        thread: Some(thread),
      }),
      Ok(Err(error)) => {
        let _ = thread.join();
        Err(error)
      }
      Err(error) => {
        let _ = thread.join();
        Err(format!(
          "Application hotkey release-hook thread stopped during startup: {error}"
        ))
      }
    }
  }
}

impl Drop for KeyReleaseHook {
  /// Posts [`WM_QUIT`] to the hook's message queue and waits for its thread to finish.
  fn drop(&mut self) {
    if let Err(error) = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) } {
      error!("Failed to stop application hotkey release hook: {error}");
      return;
    }

    if let Some(thread) = self.thread.take()
      && thread.join().is_err()
    {
      error!("Application hotkey release-hook thread panicked while stopping");
    }
  }
}

/// Installs the hook, reports readiness, then:
/// 1. Waits for messages using [`GetMessageW`].
/// 2. Processes them with [`TranslateMessage`] and [`DispatchMessageW`].
/// 3. Stops when it receives [`WM_QUIT`].  
///
/// [`PeekMessageW`] creates the thread's message queue before its ID is published, making a later
/// [`PostThreadMessageW`] safe from the queue-not-yet-created race.
fn install_key_release_hook(ready_sender: Sender<Result<u32, String>>) {
  let hook = match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_release_hook), None, 0) } {
    Ok(hook) => hook,
    Err(error) => {
      let _ = ready_sender.send(Err(format!("Failed to install application hotkey release hook: {error}")));
      return;
    }
  };
  let thread_id = unsafe { GetCurrentThreadId() };
  let mut message = MSG::default();
  unsafe {
    let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
  }
  if ready_sender.send(Ok(thread_id)).is_err() {
    unsafe {
      let _ = UnhookWindowsHookEx(hook);
    }
    return;
  }

  loop {
    let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
    if result.0 <= 0 {
      if result.0 < 0 {
        error!("Application hotkey release-hook message loop failed");
      }
      break;
    }
    unsafe {
      let _ = TranslateMessage(&message);
      DispatchMessageW(&message);
    }
  }

  unsafe {
    if let Err(error) = UnhookWindowsHookEx(hook) {
      error!("Failed to remove application hotkey release hook: {error}");
    }
  }
}

/// Handles Windows `WH_KEYBOARD_LL` events and always continues the hook chain.
///
/// `hook_data` points to a [`KBDLLHOOKSTRUCT`] only when `code == HC_ACTION`. Injected events are ignored so synthetic
/// input cannot rearm a physical key's latch.
extern "system" fn key_release_hook(code: i32, message: WPARAM, hook_data: LPARAM) -> LRESULT {
  if code == HC_ACTION as i32 {
    // Windows guarantees `KBDLLHOOKSTRUCT` data for actionable low-level keyboard-hook events
    let event = unsafe { &*(hook_data.0 as *const KBDLLHOOKSTRUCT) };
    if let Some(key_code) = get_released_key_code(message.0 as u32, event.vkCode, event.flags.0 & LLKHF_INJECTED.0 != 0) {
      let latches = RELEASE_LATCHES.read().unwrap_or_else(|poisoned| poisoned.into_inner());
      rearm_released_key(&latches, key_code);
    }
  }

  unsafe { CallNextHookEx(None, code, message, hook_data) }
}

/// Extracts the virtual-key code from a physical regular or system key-release event.
///
/// [`WM_SYSKEYUP`] covers releases involving system modifiers such as `Alt`.
fn get_released_key_code(message: u32, key_code: u32, is_injected: bool) -> Option<u32> {
  if !is_injected && matches!(message, WM_KEYUP | WM_SYSKEYUP) {
    Some(key_code)
  } else {
    None
  }
}

/// Rearms the application shortcut associated with a released Windows virtual key.
pub(super) fn rearm_released_key(latches: &HashMap<u32, PressLatch>, key_code: u32) {
  if let Some(latch) = latches.get(&key_code) {
    latch.release();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn decodes_only_physical_regular_and_system_key_releases() {
    assert_eq!(get_released_key_code(WM_KEYUP, 0x46, false), Some(0x46));
    assert_eq!(get_released_key_code(WM_SYSKEYUP, 0x46, false), Some(0x46));
    assert_eq!(
      get_released_key_code(windows::Win32::UI::WindowsAndMessaging::WM_KEYDOWN, 0x46, false),
      None
    );
    assert_eq!(get_released_key_code(WM_KEYUP, 0x46, true), None);
  }

  #[test]
  fn released_key_rearms_only_its_registered_latch() {
    let released_latch = PressLatch::default();
    let held_latch = PressLatch::default();
    assert!(released_latch.try_press());
    assert!(held_latch.try_press());
    let latches = HashMap::from([(0x46, released_latch.clone()), (0x4D, held_latch.clone())]);

    rearm_released_key(&latches, 0x46);

    assert!(released_latch.try_press());
    assert!(!held_latch.try_press());
  }
}
