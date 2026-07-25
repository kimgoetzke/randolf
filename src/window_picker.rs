//! Coordinates interactive window identification and its short-lived native Windows UI.

use crate::api::{WindowLookupError, WindowMetadata, WindowsApi};
use crate::common::{Command, Point};
use crossbeam_channel::Sender;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{E_FAIL, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
  InitCommonControls, TASKDIALOG_BUTTON, TASKDIALOGCONFIG, TDCBF_CLOSE_BUTTON, TDF_ALLOW_DIALOG_CANCELLATION,
  TDF_SIZE_TO_CONTENT, TDF_USE_COMMAND_LINKS, TOOLTIPS_CLASSW, TTF_ABSOLUTE, TTF_TRACK, TTM_ADDTOOLW, TTM_SETMAXTIPWIDTH,
  TTM_TRACKACTIVATE, TTM_TRACKPOSITION, TTM_UPDATETIPTEXTW, TTS_ALWAYSTIP, TTS_NOPREFIX, TTTOOLINFOW, TaskDialog,
  TaskDialogIndirect,
};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
use windows::Win32::UI::WindowsAndMessaging::{
  CallNextHookEx, CreateWindowExW, DestroyWindow, GA_ROOT, GetAncestor, GetClassNameW, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT,
  MSLLHOOKSTRUCT, SendMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL, WINDOW_EX_STYLE,
  WINDOW_STYLE, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN,
  WM_SYSKEYUP, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WindowFromPoint,
};
use windows::core::{Error as WindowsError, PCWSTR, PWSTR, Result as WindowsResult, w};

const HOVER_INTERVAL: Duration = Duration::from_millis(75);
const TOOLTIP_OFFSET_X: i32 = 18;
const TOOLTIP_OFFSET_Y: i32 = 24;
const PICK_AGAIN_BUTTON_ID: i32 = 1001;
const CLOSE_BUTTON_ID: i32 = 1002;
static G_HOOK_CALLBACK_STATE: HookCallbackState = HookCallbackState::new();

/// Allows one outcome command per picker session.
#[derive(Debug, Default)]
struct CompletionGate {
  completed: AtomicBool,
}

impl CompletionGate {
  /// Creates an incomplete gate.
  const fn new() -> Self {
    Self {
      completed: AtomicBool::new(false),
    }
  }

  /// Opens the gate for a new picker session.
  fn open(&self) {
    self.completed.store(false, Ordering::Release);
  }

  /// Closes the gate without producing an outcome.
  fn close(&self) {
    self.completed.store(true, Ordering::Release);
  }

  /// Claims completion unless another attempt completed first.
  fn try_complete(&self) -> bool {
    !self.completed.swap(true, Ordering::AcqRel)
  }
}

/// Stores state shared with Win32 callbacks, which cannot capture a picker session.
#[derive(Debug)]
struct HookCallbackState {
  command_sender: OnceLock<Sender<Command>>,
  active: AtomicBool,
  left_button_down: AtomicBool,
  right_button_down: AtomicBool,
  escape_down: AtomicBool,
  selection_x: AtomicI32,
  selection_y: AtomicI32,
  completion_gate: CompletionGate,
}

impl HookCallbackState {
  const fn new() -> Self {
    Self {
      command_sender: OnceLock::new(),
      active: AtomicBool::new(false),
      left_button_down: AtomicBool::new(false),
      right_button_down: AtomicBool::new(false),
      escape_down: AtomicBool::new(false),
      selection_x: AtomicI32::new(0),
      selection_y: AtomicI32::new(0),
      completion_gate: CompletionGate::new(),
    }
  }

  /// Enables event capture for a new session.
  fn activate(&self, command_sender: Sender<Command>) {
    if let Err(command_sender) = self.command_sender.set(command_sender) {
      debug_assert!(
        self
          .command_sender
          .get()
          .is_some_and(|registered_sender| registered_sender.same_channel(&command_sender)),
        "one process-wide Window Picker command channel must be used"
      );
    }
    self.reset_pressed_inputs();
    self.completion_gate.open();
    self.active.store(true, Ordering::Release);
  }

  /// Stops event capture and prevents late completion commands.
  fn deactivate(&self) {
    self.active.store(false, Ordering::Release);
    self.reset_pressed_inputs();
    self.completion_gate.close();
  }

  fn is_active(&self) -> bool {
    self.active.load(Ordering::Acquire)
  }

  fn press_left_button(&self, point: POINT) {
    self.selection_x.store(point.x, Ordering::Relaxed);
    self.selection_y.store(point.y, Ordering::Relaxed);
    self.left_button_down.store(true, Ordering::Relaxed);
  }

  fn release_left_button(&self) -> Option<Point> {
    self.left_button_down.swap(false, Ordering::Relaxed).then(|| {
      Point::new(
        self.selection_x.load(Ordering::Relaxed),
        self.selection_y.load(Ordering::Relaxed),
      )
    })
  }

  fn press_right_button(&self) {
    self.right_button_down.store(true, Ordering::Relaxed);
  }

  fn release_right_button(&self) -> bool {
    self.right_button_down.swap(false, Ordering::Relaxed)
  }

  fn press_escape(&self) {
    self.escape_down.store(true, Ordering::Relaxed);
  }

  fn release_escape(&self) -> bool {
    self.escape_down.swap(false, Ordering::Relaxed)
  }

  /// Sends one outcome and immediately stops suppressing further input.
  fn complete(&self, command: Command) {
    if !self.completion_gate.try_complete() {
      return;
    }
    self.active.store(false, Ordering::Release);
    if let Some(sender) = self.command_sender.get()
      && let Err(err) = sender.send(command)
    {
      error!("Failed to send Window Picker hook command: {err}");
    }
  }

  fn reset_pressed_inputs(&self) {
    self.left_button_down.store(false, Ordering::Relaxed);
    self.right_button_down.store(false, Ordering::Relaxed);
    self.escape_down.store(false, Ordering::Relaxed);
  }
}

/// User choice from the dialogue showing the selected window's metadata.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum SelectionDialogChoice {
  PickAgain,
  Close,
}

/// Owns every native resource used while the user is choosing a window.
///
/// Grouping hooks, tooltip, and refresh timing makes resource presence the sole source of truth for whether picking is
/// active. Dropping this value releases all session resources.
struct ActivePickerSession {
  _input_hooks: NativePickerHooks,
  hover_tooltip: TrackingTooltip,
  last_hover_refresh: Instant,
}

impl ActivePickerSession {
  fn start(command_sender: Sender<Command>) -> WindowsResult<Self> {
    // Create all visible UI before capturing input. If hook installation fails, the tooltip drops automatically.
    let hover_tooltip = TrackingTooltip::new()?;
    let input_hooks = NativePickerHooks::install(command_sender)?;
    Ok(Self {
      _input_hooks: input_hooks,
      hover_tooltip,
      last_hover_refresh: Instant::now() - HOVER_INTERVAL,
    })
  }

  fn refresh_hover_preview<Api: WindowsApi>(&mut self, api: &Api) {
    if self.last_hover_refresh.elapsed() < HOVER_INTERVAL {
      return;
    }
    self.last_hover_refresh = Instant::now();

    let cursor_position = api.get_cursor_position();
    let text = match api.get_window_at_point(cursor_position) {
      Ok(metadata) => hover_text(&metadata),
      Err(error) => hover_error_text(error),
    };
    self.hover_tooltip.show_at(cursor_position, &text);
  }
}

/// Coordinates Window Picker commands and owns the current native picking session.
///
/// Create one instance per process: Win32 callbacks register its command channel in process-wide callback state.
pub struct WindowPicker<Api: WindowsApi> {
  windows_api: Api,
  command_sender: Sender<Command>,
  active_session: Option<ActivePickerSession>,
}

impl<Api: WindowsApi> std::fmt::Debug for WindowPicker<Api> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("WindowPicker")
      .field("is_active", &self.active_session.is_some())
      .finish_non_exhaustive()
  }
}

impl<Api: WindowsApi> WindowPicker<Api> {
  /// Creates the process's inactive picker.
  pub fn new(windows_api: Api, command_sender: Sender<Command>) -> Self {
    Self {
      windows_api,
      command_sender,
      active_session: None,
    }
  }

  /// Starts picking, or cancels the current session when already active.
  pub fn handle_toggle(&mut self) {
    if self.active_session.is_some() {
      self.cancel();
      return;
    }

    if let Err(error) = self.activate() {
      report_picker_error("Failed to start Window Picker", &error);
    }
  }

  /// Resolves an active session's selected point and ignores stale outcomes after cancellation.
  pub fn handle_selection(&mut self, point: Point) {
    if self.active_session.is_none() {
      return;
    }

    let metadata = match self.select(point) {
      Ok(metadata) => metadata,
      Err(error) => {
        report_picker_error("Window Picker selection failed", &error);
        return;
      }
    };

    match show_selection_dialog(&metadata) {
      Ok(SelectionDialogChoice::PickAgain) => {
        if let Err(error) = self.activate() {
          report_picker_error("Failed to restart Window Picker", &error);
        }
      }
      Ok(SelectionDialogChoice::Close) => {}
      Err(error) => error!("Failed to show Window Picker result dialogue: {error}"),
    }
  }

  /// Cancels picking and releases its native resources.
  pub fn cancel(&mut self) {
    self.active_session = None;
  }

  /// Refreshes the hover preview when picking and its throttle interval has elapsed.
  pub fn refresh_hover_preview(&mut self) {
    if let Some(active_session) = self.active_session.as_mut() {
      active_session.refresh_hover_preview(&self.windows_api);
    }
  }

  fn activate(&mut self) -> WindowsResult<()> {
    debug_assert!(self.active_session.is_none(), "activation requires an inactive Window Picker");
    self.active_session = Some(ActivePickerSession::start(self.command_sender.clone())?);
    Ok(())
  }

  /// Releases native resources before resolving the selected window.
  fn select(&mut self, point: Point) -> Result<WindowMetadata, WindowLookupError> {
    self.cancel();
    self.windows_api.get_window_at_point(point)
  }
}

/// Logs a picker failure and attempts to present it to the user.
fn report_picker_error(context: &str, error: &impl std::fmt::Display) {
  let message = format!("{context}: {error}");
  error!("{message}");
  if let Err(dialog_error) = show_picker_error_dialog(&message) {
    error!("Failed to show Window Picker error dialogue: {dialog_error}");
  }
}

/// Shows selected window metadata and returns the requested follow-up choice.
fn show_selection_dialog(metadata: &WindowMetadata) -> WindowsResult<SelectionDialogChoice> {
  let title = null_terminated_utf16("Randolf Window Picker");
  let instruction = null_terminated_utf16("Selected top-level window");
  let content = null_terminated_utf16(&selection_dialog_content(metadata));
  let pick_again = null_terminated_utf16("Pick another window");
  let close = null_terminated_utf16("Close");
  let buttons = [
    TASKDIALOG_BUTTON {
      nButtonID: PICK_AGAIN_BUTTON_ID,
      pszButtonText: PCWSTR(pick_again.as_ptr()),
    },
    TASKDIALOG_BUTTON {
      nButtonID: CLOSE_BUTTON_ID,
      pszButtonText: PCWSTR(close.as_ptr()),
    },
  ];
  let config = TASKDIALOGCONFIG {
    cbSize: size_of::<TASKDIALOGCONFIG>() as u32,
    dwFlags: TDF_ALLOW_DIALOG_CANCELLATION | TDF_SIZE_TO_CONTENT | TDF_USE_COMMAND_LINKS,
    pszWindowTitle: PCWSTR(title.as_ptr()),
    pszMainInstruction: PCWSTR(instruction.as_ptr()),
    pszContent: PCWSTR(content.as_ptr()),
    cButtons: buttons.len() as u32,
    pButtons: buttons.as_ptr(),
    nDefaultButton: CLOSE_BUTTON_ID,
    ..Default::default()
  };
  let mut selected_button = CLOSE_BUTTON_ID;
  unsafe {
    TaskDialogIndirect(&config, Some(&mut selected_button), None, None)?;
  }
  Ok(selection_dialog_choice(selected_button))
}

/// Shows a picker error in a native task dialogue.
fn show_picker_error_dialog(message: &str) -> WindowsResult<()> {
  let message = null_terminated_utf16(message);
  let mut selected_button = 0;
  unsafe {
    TaskDialog(
      None,
      None,
      w!("Randolf Window Picker"),
      w!("Unable to identify window"),
      PCWSTR(message.as_ptr()),
      TDCBF_CLOSE_BUTTON,
      PCWSTR::null(),
      Some(&mut selected_button),
    )
  }
}

/// Maps a native dialogue button to a picker action.
fn selection_dialog_choice(button_id: i32) -> SelectionDialogChoice {
  if button_id == PICK_AGAIN_BUTTON_ID {
    SelectionDialogChoice::PickAgain
  } else {
    SelectionDialogChoice::Close
  }
}

/// Formats selected window metadata for the result dialogue.
fn selection_dialog_content(metadata: &WindowMetadata) -> String {
  format!(
    "Window title:\n{}\n\nWindow class name:\n{}",
    display_value(&metadata.title),
    display_value(&metadata.class_name)
  )
}

/// Formats selected window metadata for the tracking tooltip.
fn hover_text(metadata: &WindowMetadata) -> String {
  format!(
    "Window title: {}\nWindow class name: {}\nClick to select · Esc/right-click cancels",
    display_value(&metadata.title),
    display_value(&metadata.class_name)
  )
}

/// Formats a lookup failure for the tracking tooltip.
fn hover_error_text(error: WindowLookupError) -> String {
  format!("Window Picker\n{error}\nClick a window · Esc/right-click cancels")
}

/// Replaces an empty metadata value with an explicit placeholder.
fn display_value(value: &str) -> &str {
  if value.is_empty() { "(empty)" } else { value }
}

/// Owns the Win32 hooks that capture one selection or cancellation gesture.
///
/// Win32 requires plain `extern "system"` callbacks, so callback data lives in `G_HOOK_CALLBACK_STATE` rather than
/// borrowing this value.
struct NativePickerHooks {
  mouse_hook: HHOOK,
  keyboard_hook: HHOOK,
}

impl NativePickerHooks {
  /// Installs both hooks before enabling callback event capture.
  fn install(command_sender: Sender<Command>) -> WindowsResult<Self> {
    let module = unsafe { GetModuleHandleW(None)? };
    let instance = HINSTANCE(module.0);
    let keyboard_hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(picker_keyboard_hook), Some(instance), 0)? };
    let mouse_hook = match unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(picker_mouse_hook), Some(instance), 0) } {
      Ok(hook) => hook,
      Err(error) => {
        unsafe {
          let _ = UnhookWindowsHookEx(keyboard_hook);
        }
        return Err(error);
      }
    };
    G_HOOK_CALLBACK_STATE.activate(command_sender);
    Ok(Self {
      mouse_hook,
      keyboard_hook,
    })
  }
}

impl Drop for NativePickerHooks {
  /// Stops callbacks before removing both native hooks.
  fn drop(&mut self) {
    G_HOOK_CALLBACK_STATE.deactivate();
    unsafe {
      if let Err(error) = UnhookWindowsHookEx(self.mouse_hook) {
        error!("Failed to remove Window Picker mouse hook: {error}");
      }
      if let Err(error) = UnhookWindowsHookEx(self.keyboard_hook) {
        error!("Failed to remove Window Picker keyboard hook: {error}");
      }
    }
  }
}

/// Suppresses picker mouse gestures and emits one selection or cancellation command.
extern "system" fn picker_mouse_hook(code: i32, message: WPARAM, hook_data: LPARAM) -> LRESULT {
  if code != HC_ACTION as i32 || !G_HOOK_CALLBACK_STATE.is_active() {
    return unsafe { CallNextHookEx(None, code, message, hook_data) };
  }
  if matches!(message.0 as u32, WM_LBUTTONDOWN | WM_RBUTTONDOWN) {
    // Windows guarantees `MSLLHOOKSTRUCT` data for actionable low-level mouse-hook events.
    let mouse_event = unsafe { &*(hook_data.0 as *const MSLLHOOKSTRUCT) };
    if should_pass_mouse_input_through(mouse_event.pt) {
      return unsafe { CallNextHookEx(None, code, message, hook_data) };
    }
  }

  match message.0 as u32 {
    WM_LBUTTONDOWN => {
      let mouse_event = unsafe { &*(hook_data.0 as *const MSLLHOOKSTRUCT) };
      G_HOOK_CALLBACK_STATE.press_left_button(mouse_event.pt);
      LRESULT(1)
    }
    WM_LBUTTONUP => match G_HOOK_CALLBACK_STATE.release_left_button() {
      Some(selection_point) => {
        G_HOOK_CALLBACK_STATE.complete(Command::WindowPickerSelected(selection_point));
        LRESULT(1)
      }
      None => unsafe { CallNextHookEx(None, code, message, hook_data) },
    },
    WM_RBUTTONDOWN => {
      G_HOOK_CALLBACK_STATE.press_right_button();
      LRESULT(1)
    }
    WM_RBUTTONUP if G_HOOK_CALLBACK_STATE.release_right_button() => {
      G_HOOK_CALLBACK_STATE.complete(Command::CancelWindowPicker);
      LRESULT(1)
    }
    _ => unsafe { CallNextHookEx(None, code, message, hook_data) },
  }
}

/// Suppresses Escape gestures and emits one cancellation command.
extern "system" fn picker_keyboard_hook(code: i32, message: WPARAM, hook_data: LPARAM) -> LRESULT {
  if code != HC_ACTION as i32 || !G_HOOK_CALLBACK_STATE.is_active() {
    return unsafe { CallNextHookEx(None, code, message, hook_data) };
  }
  // Windows guarantees `KBDLLHOOKSTRUCT` data for actionable low-level keyboard-hook events.
  let keyboard_event = unsafe { &*(hook_data.0 as *const KBDLLHOOKSTRUCT) };
  if keyboard_event.vkCode != VK_ESCAPE.0 as u32 {
    return unsafe { CallNextHookEx(None, code, message, hook_data) };
  }

  match message.0 as u32 {
    WM_KEYDOWN | WM_SYSKEYDOWN => {
      G_HOOK_CALLBACK_STATE.press_escape();
      LRESULT(1)
    }
    WM_KEYUP | WM_SYSKEYUP if G_HOOK_CALLBACK_STATE.release_escape() => {
      G_HOOK_CALLBACK_STATE.complete(Command::CancelWindowPicker);
      LRESULT(1)
    }
    _ => unsafe { CallNextHookEx(None, code, message, hook_data) },
  }
}

/// Reports whether a taskbar or native-menu window should receive the mouse input.
fn should_pass_mouse_input_through(point: POINT) -> bool {
  let hit_window = unsafe { WindowFromPoint(point) };
  if hit_window.0.is_null() {
    return false;
  }
  let root_window = unsafe { GetAncestor(hit_window, GA_ROOT) };
  let target_window = if root_window.0.is_null() { hit_window } else { root_window };
  let mut class_name = [0_u16; 256];
  let length = unsafe { GetClassNameW(target_window, &mut class_name) };
  is_input_passthrough_window_class(&String::from_utf16_lossy(&class_name[..length as usize]))
}

/// Reports whether a native window class must remain interactive during picking.
fn is_input_passthrough_window_class(class_name: &str) -> bool {
  matches!(class_name, "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "#32768")
}

/// Owns a native tracking tooltip and the UTF-16 text referenced by Win32.
struct TrackingTooltip {
  native_window: HWND,
  tool_info: TTTOOLINFOW,
  utf16_text: Vec<u16>,
}

impl TrackingTooltip {
  /// Creates an inactive topmost tracking tooltip.
  fn new() -> WindowsResult<Self> {
    unsafe {
      InitCommonControls();
    }
    let extended_style = WINDOW_EX_STYLE(WS_EX_TOPMOST.0 | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0);
    let style = WINDOW_STYLE(WS_POPUP.0 | TTS_ALWAYSTIP | TTS_NOPREFIX);
    let window = unsafe {
      CreateWindowExW(
        extended_style,
        TOOLTIPS_CLASSW,
        w!(""),
        style,
        0,
        0,
        0,
        0,
        None,
        None,
        None,
        None,
      )?
    };
    let mut utf16_text = null_terminated_utf16("");
    let tool_info = TTTOOLINFOW {
      cbSize: size_of::<TTTOOLINFOW>() as u32,
      uFlags: TTF_TRACK | TTF_ABSOLUTE,
      hwnd: window,
      uId: 1,
      lpszText: PWSTR(utf16_text.as_mut_ptr()),
      ..Default::default()
    };
    let mut tooltip = Self {
      native_window: window,
      tool_info,
      utf16_text,
    };
    let tool_added = unsafe {
      SendMessageW(
        window,
        TTM_ADDTOOLW,
        Some(WPARAM(0)),
        Some(LPARAM(&mut tooltip.tool_info as *mut TTTOOLINFOW as isize)),
      )
    };
    if tool_added.0 == 0 {
      return Err(WindowsError::new(
        E_FAIL,
        "failed to register the Window Picker hover tooltip",
      ));
    }
    unsafe {
      SendMessageW(window, TTM_SETMAXTIPWIDTH, Some(WPARAM(0)), Some(LPARAM(600)));
    }
    Ok(tooltip)
  }

  /// Updates and displays the tooltip beside a screen point.
  fn show_at(&mut self, point: Point, text: &str) {
    self.utf16_text = null_terminated_utf16(text);
    self.tool_info.lpszText = PWSTR(self.utf16_text.as_mut_ptr());
    let x = point.x() + TOOLTIP_OFFSET_X;
    let y = point.y() + TOOLTIP_OFFSET_Y;
    let packed_position = ((y as u32 & 0xffff) << 16) | (x as u32 & 0xffff);
    unsafe {
      SendMessageW(
        self.native_window,
        TTM_UPDATETIPTEXTW,
        Some(WPARAM(0)),
        Some(LPARAM(&mut self.tool_info as *mut TTTOOLINFOW as isize)),
      );
      SendMessageW(
        self.native_window,
        TTM_TRACKPOSITION,
        Some(WPARAM(0)),
        Some(LPARAM(packed_position as isize)),
      );
      SendMessageW(
        self.native_window,
        TTM_TRACKACTIVATE,
        Some(WPARAM(1)),
        Some(LPARAM(&mut self.tool_info as *mut TTTOOLINFOW as isize)),
      );
    }
  }
}

impl Drop for TrackingTooltip {
  /// Hides and destroys the native tooltip.
  fn drop(&mut self) {
    unsafe {
      SendMessageW(
        self.native_window,
        TTM_TRACKACTIVATE,
        Some(WPARAM(0)),
        Some(LPARAM(&mut self.tool_info as *mut TTTOOLINFOW as isize)),
      );
      if let Err(error) = DestroyWindow(self.native_window) {
        error!("Failed to destroy Window Picker tooltip: {error}");
      }
    }
  }
}

/// Encodes text as a null-terminated UTF-16 string.
fn null_terminated_utf16(value: &str) -> Vec<u16> {
  value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn metadata(title: &str, class_name: &str) -> WindowMetadata {
    WindowMetadata {
      handle: crate::common::WindowHandle::new(42),
      title: title.to_string(),
      class_name: class_name.to_string(),
      rect: crate::common::Rect::default(),
    }
  }

  #[test]
  fn selection_dialog_content_preserves_unicode_and_labels_empty_values() {
    assert_eq!(
      selection_dialog_content(&metadata("Résumé — 東京", "")),
      "Window title:\nRésumé — 東京\n\nWindow class name:\n(empty)"
    );
  }

  #[test]
  fn hover_preview_labels_empty_values_and_explains_controls() {
    assert_eq!(
      hover_text(&metadata("", "EditorWindow")),
      "Window title: (empty)\nWindow class name: EditorWindow\nClick to select · Esc/right-click cancels"
    );
    assert_eq!(
      hover_error_text(WindowLookupError::AccessDenied),
      "Window Picker\nRandolf does not have permission to inspect that window\nClick a window · Esc/right-click cancels"
    );
  }

  #[test]
  fn selection_dialog_choice_maps_custom_actions_and_window_close() {
    assert_eq!(
      selection_dialog_choice(PICK_AGAIN_BUTTON_ID),
      SelectionDialogChoice::PickAgain
    );
    assert_eq!(selection_dialog_choice(CLOSE_BUTTON_ID), SelectionDialogChoice::Close);
    assert_eq!(selection_dialog_choice(2), SelectionDialogChoice::Close);
  }

  #[test]
  fn input_passthrough_classes_cover_taskbars_and_native_menus() {
    assert!(is_input_passthrough_window_class("Shell_TrayWnd"));
    assert!(is_input_passthrough_window_class("Shell_SecondaryTrayWnd"));
    assert!(is_input_passthrough_window_class("#32768"));
    assert!(!is_input_passthrough_window_class("EditorWindow"));
  }

  #[test]
  fn completion_gate_allows_one_outcome_per_open_session() {
    let gate = CompletionGate::default();
    assert!(gate.try_complete());
    assert!(!gate.try_complete());

    gate.open();
    assert!(gate.try_complete());
    gate.close();
    assert!(!gate.try_complete());
  }

  #[test]
  fn hook_callback_state_pairs_clicks_and_stops_capture_after_completion() {
    let (command_sender, command_receiver) = crossbeam_channel::unbounded();
    let state = HookCallbackState::new();
    state.activate(command_sender);

    assert!(state.is_active());
    assert_eq!(state.release_left_button(), None);
    state.press_left_button(POINT { x: 12, y: 34 });
    assert_eq!(state.release_left_button(), Some(Point::new(12, 34)));

    state.complete(Command::CancelWindowPicker);
    state.complete(Command::WindowPickerSelected(Point::new(12, 34)));

    assert!(!state.is_active());
    assert!(matches!(command_receiver.try_recv(), Ok(Command::CancelWindowPicker)));
    assert!(command_receiver.try_recv().is_err());
  }

  #[test]
  fn utf16_encoder_appends_one_null_terminator() {
    assert_eq!(null_terminated_utf16("A"), vec![65, 0]);
  }
}
