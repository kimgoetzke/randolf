use crate::api::{WindowLookupError, WindowMetadata, WindowsApi};
use crate::common::{Command, Point};
use crossbeam_channel::Sender;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
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
  WINDOW_STYLE, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_RBUTTONDOWN, WM_RBUTTONUP, WS_EX_NOACTIVATE,
  WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WindowFromPoint,
};
use windows::core::{PCWSTR, PWSTR, Result as WindowsResult, w};

const HOVER_INTERVAL: Duration = Duration::from_millis(75);
const TOOLTIP_OFFSET_X: i32 = 18;
const TOOLTIP_OFFSET_Y: i32 = 24;
const PICK_AGAIN_BUTTON_ID: i32 = 1001;
const CLOSE_BUTTON_ID: i32 = 1002;
static G_PICKER_SENDER: OnceLock<Sender<Command>> = OnceLock::new();
static G_PICKER_ACTIVE: AtomicBool = AtomicBool::new(false);
static G_LEFT_BUTTON_DOWN: AtomicBool = AtomicBool::new(false);
static G_RIGHT_BUTTON_DOWN: AtomicBool = AtomicBool::new(false);
static G_ESCAPE_DOWN: AtomicBool = AtomicBool::new(false);
static G_SELECTION_X: AtomicI32 = AtomicI32::new(0);
static G_SELECTION_Y: AtomicI32 = AtomicI32::new(0);
static G_COMPLETION_GATE: CompletionGate = CompletionGate::new();

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

  /// Allows the next completion attempt.
  fn reset(&self) {
    self.completed.store(false, Ordering::Release);
  }

  /// Claims completion unless another attempt completed first.
  fn try_complete(&self) -> bool {
    !self.completed.swap(true, Ordering::AcqRel)
  }
}

/// Window Picker lifecycle state.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum PickerStatus {
  Inactive,
  Active,
}

/// Native work required by a picker transition.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum PickerAction {
  Activate,
  Cancel,
}

/// Tracks one non-stacking Window Picker session.
#[derive(Debug, Default)]
struct PickerSession {
  active: bool,
}

impl PickerSession {
  /// Toggles active mode, cancelling an existing session.
  fn toggle(&mut self) -> PickerAction {
    self.active = !self.active;
    if self.active {
      PickerAction::Activate
    } else {
      PickerAction::Cancel
    }
  }

  /// Finishes selection or cancellation.
  fn finish(&mut self) {
    self.active = false;
  }

  /// Returns the current lifecycle state.
  fn status(&self) -> PickerStatus {
    if self.active {
      PickerStatus::Active
    } else {
      PickerStatus::Inactive
    }
  }
}

/// Action selected from the frozen result dialogue.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ResultDialogAction {
  PickAgain,
  Close,
}

/// Owns one short-lived native Window Picker session.
pub struct WindowPicker<A: WindowsApi> {
  api: A,
  command_sender: Sender<Command>,
  session: PickerSession,
  hooks: Option<NativePickerHooks>,
  tooltip: Option<TrackingTooltip>,
  last_hover_update: Instant,
}

impl<A: WindowsApi> WindowPicker<A> {
  /// Creates an inactive picker.
  pub fn new(api: A, command_sender: Sender<Command>) -> Self {
    Self {
      api,
      command_sender,
      session: PickerSession::default(),
      hooks: None,
      tooltip: None,
      last_hover_update: Instant::now() - HOVER_INTERVAL,
    }
  }

  /// Handles a picker toggle command.
  pub fn handle_toggle(&mut self) {
    if let Err(err) = self.toggle() {
      self.cancel();
      error!("Failed to start Window Picker: {err}");
      if let Err(dialog_err) = show_picker_error_dialog(&format!("Failed to start Window Picker: {err}")) {
        error!("Failed to show Window Picker start error: {dialog_err}");
      }
    }
  }

  /// Transitions between active and inactive picker states.
  fn toggle(&mut self) -> WindowsResult<bool> {
    match self.session.toggle() {
      PickerAction::Activate => {
        let tooltip = match TrackingTooltip::new() {
          Ok(tooltip) => tooltip,
          Err(err) => {
            self.session.finish();
            return Err(err);
          }
        };
        match NativePickerHooks::install(self.command_sender.clone()) {
          Ok(hooks) => {
            self.tooltip = Some(tooltip);
            self.hooks = Some(hooks);
            self.last_hover_update = Instant::now() - HOVER_INTERVAL;
            Ok(true)
          }
          Err(err) => {
            self.session.finish();
            Err(err)
          }
        }
      }
      PickerAction::Cancel => {
        self.deactivate();
        Ok(false)
      }
    }
  }

  /// Handles a completed picker selection.
  pub fn handle_selection(&mut self, point: Point) {
    let selection = self.select(point);
    match selection {
      Ok(metadata) => match show_result_dialog(&metadata) {
        Ok(ResultDialogAction::PickAgain) => match self.toggle() {
          Ok(_) => (),
          Err(err) => {
            error!("Failed to restart Window Picker: {err}");
            if let Err(dialog_err) = show_picker_error_dialog(&format!("Failed to restart Window Picker: {err}")) {
              error!("Failed to show Window Picker restart error: {dialog_err}");
            }
          }
        },
        Ok(ResultDialogAction::Close) => {}
        Err(err) => error!("Failed to show Window Picker result dialogue: {err}"),
      },
      Err(err) => {
        error!("Window Picker selection failed: {err}");
        if let Err(dialog_err) = show_picker_error_dialog(&err.to_string()) {
          error!("Failed to show Window Picker selection error: {dialog_err}");
        }
      }
    }
  }

  /// Cancels active mode and releases native resources.
  pub fn cancel(&mut self) {
    self.deactivate();
  }

  /// Releases native resources and resolves the selected window.
  fn select(&mut self, point: Point) -> Result<WindowMetadata, WindowLookupError> {
    self.deactivate();
    self.api.get_window_at_point(point)
  }

  /// Refreshes hover metadata at a bounded rate.
  pub fn update_hover_tooltip_if_active(&mut self) {
    if self.session.status() != PickerStatus::Active || self.last_hover_update.elapsed() < HOVER_INTERVAL {
      return;
    }
    self.last_hover_update = Instant::now();
    let point = self.api.get_cursor_position();
    let text = match self.api.get_window_at_point(point) {
      Ok(metadata) => hover_text(&metadata),
      Err(error) => format!("Window Picker\n{error}\nClick a window · Esc/right-click cancels"),
    };
    if let Some(tooltip) = self.tooltip.as_mut() {
      tooltip.update(point, &text);
    }
  }

  /// Ends the session and releases native resources.
  fn deactivate(&mut self) {
    self.session.finish();
    self.hooks = None;
    self.tooltip = None;
  }
}

impl<A: WindowsApi> Drop for WindowPicker<A> {
  /// Releases native picker resources.
  fn drop(&mut self) {
    self.deactivate();
  }
}

/// Shows selected window metadata and returns the requested follow-up action.
fn show_result_dialog(metadata: &WindowMetadata) -> WindowsResult<ResultDialogAction> {
  let title = wide("Randolf Window Picker");
  let instruction = wide("Selected top-level window");
  let content = wide(&result_content(metadata));
  let pick_again = wide("Pick another window");
  let close = wide("Close");
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
  Ok(dialog_action(selected_button))
}

/// Shows a picker error in a native task dialogue.
fn show_picker_error_dialog(message: &str) -> WindowsResult<()> {
  let message = wide(message);
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
fn dialog_action(button_id: i32) -> ResultDialogAction {
  if button_id == PICK_AGAIN_BUTTON_ID {
    ResultDialogAction::PickAgain
  } else {
    ResultDialogAction::Close
  }
}

/// Formats selected window metadata for the result dialogue.
fn result_content(metadata: &WindowMetadata) -> String {
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

/// Replaces an empty metadata value with an explicit placeholder.
fn display_value(value: &str) -> &str {
  if value.is_empty() { "(empty)" } else { value }
}

/// Owns installed low-level mouse and keyboard hooks.
struct NativePickerHooks {
  mouse: HHOOK,
  keyboard: HHOOK,
}

impl NativePickerHooks {
  /// Installs native hooks and enables picker callbacks.
  fn install(command_sender: Sender<Command>) -> WindowsResult<Self> {
    let _ = G_PICKER_SENDER.set(command_sender);
    let module = unsafe { GetModuleHandleW(None)? };
    let instance = HINSTANCE(module.0);
    let keyboard = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_callback), Some(instance), 0)? };
    let mouse = match unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_callback), Some(instance), 0) } {
      Ok(hook) => hook,
      Err(err) => {
        unsafe {
          let _ = UnhookWindowsHookEx(keyboard);
        }
        return Err(err);
      }
    };
    G_COMPLETION_GATE.reset();
    G_PICKER_ACTIVE.store(true, Ordering::Release);
    Ok(Self { mouse, keyboard })
  }
}

impl Drop for NativePickerHooks {
  /// Disables callbacks and removes native hooks.
  fn drop(&mut self) {
    G_PICKER_ACTIVE.store(false, Ordering::Release);
    G_LEFT_BUTTON_DOWN.store(false, Ordering::Relaxed);
    G_RIGHT_BUTTON_DOWN.store(false, Ordering::Relaxed);
    G_ESCAPE_DOWN.store(false, Ordering::Relaxed);
    G_COMPLETION_GATE.reset();
    unsafe {
      if let Err(err) = UnhookWindowsHookEx(self.mouse) {
        error!("Failed to remove Window Picker mouse hook: {err}");
      }
      if let Err(err) = UnhookWindowsHookEx(self.keyboard) {
        error!("Failed to remove Window Picker keyboard hook: {err}");
      }
    }
  }
}

/// Suppresses picker mouse input and emits one completion command.
extern "system" fn mouse_callback(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
  if code != HC_ACTION as i32 || !G_PICKER_ACTIVE.load(Ordering::Acquire) {
    return unsafe { CallNextHookEx(None, code, message, data) };
  }
  if matches!(message.0 as u32, WM_LBUTTONDOWN | WM_RBUTTONDOWN) {
    let hook = unsafe { &*(data.0 as *const MSLLHOOKSTRUCT) };
    if is_picker_passthrough_point(hook.pt) {
      return unsafe { CallNextHookEx(None, code, message, data) };
    }
  }

  match message.0 as u32 {
    WM_LBUTTONDOWN => {
      let hook = unsafe { &*(data.0 as *const MSLLHOOKSTRUCT) };
      G_SELECTION_X.store(hook.pt.x, Ordering::Relaxed);
      G_SELECTION_Y.store(hook.pt.y, Ordering::Relaxed);
      G_LEFT_BUTTON_DOWN.store(true, Ordering::Relaxed);
      LRESULT(1)
    }
    WM_LBUTTONUP if G_LEFT_BUTTON_DOWN.swap(false, Ordering::Relaxed) => {
      complete_hook(Command::WindowPickerSelected(Point::new(
        G_SELECTION_X.load(Ordering::Relaxed),
        G_SELECTION_Y.load(Ordering::Relaxed),
      )));
      LRESULT(1)
    }
    WM_RBUTTONDOWN => {
      G_RIGHT_BUTTON_DOWN.store(true, Ordering::Relaxed);
      LRESULT(1)
    }
    WM_RBUTTONUP if G_RIGHT_BUTTON_DOWN.swap(false, Ordering::Relaxed) => {
      complete_hook(Command::CancelWindowPicker);
      LRESULT(1)
    }
    _ => unsafe { CallNextHookEx(None, code, message, data) },
  }
}

/// Suppresses Escape input and emits a cancellation command.
extern "system" fn keyboard_callback(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
  if code != HC_ACTION as i32 || !G_PICKER_ACTIVE.load(Ordering::Acquire) {
    return unsafe { CallNextHookEx(None, code, message, data) };
  }
  let hook = unsafe { &*(data.0 as *const KBDLLHOOKSTRUCT) };
  if hook.vkCode != VK_ESCAPE.0 as u32 {
    return unsafe { CallNextHookEx(None, code, message, data) };
  }

  match message.0 as u32 {
    WM_KEYDOWN => {
      G_ESCAPE_DOWN.store(true, Ordering::Relaxed);
      LRESULT(1)
    }
    WM_KEYUP if G_ESCAPE_DOWN.swap(false, Ordering::Relaxed) => {
      complete_hook(Command::CancelWindowPicker);
      LRESULT(1)
    }
    _ => unsafe { CallNextHookEx(None, code, message, data) },
  }
}

/// Reports whether picker hooks should pass through input at a point.
fn is_picker_passthrough_point(point: POINT) -> bool {
  let hit = unsafe { WindowFromPoint(point) };
  if hit.0.is_null() {
    return false;
  }
  let root = unsafe { GetAncestor(hit, GA_ROOT) };
  let target = if root.0.is_null() { hit } else { root };
  let mut class_name = [0_u16; 256];
  let length = unsafe { GetClassNameW(target, &mut class_name) };
  is_picker_passthrough_class(&String::from_utf16_lossy(&class_name[..length as usize]))
}

/// Reports whether a native window class receives picker input.
fn is_picker_passthrough_class(class_name: &str) -> bool {
  matches!(class_name, "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "#32768")
}

/// Sends the first completion command emitted by the active hooks.
fn complete_hook(command: Command) {
  if !G_COMPLETION_GATE.try_complete() {
    return;
  }
  if let Some(sender) = G_PICKER_SENDER.get()
    && let Err(err) = sender.send(command)
  {
    error!("Failed to send Window Picker hook command: {err}");
  }
}

/// Owns a tracking tooltip and its backing UTF-16 text.
struct TrackingTooltip {
  window: HWND,
  tool: TTTOOLINFOW,
  text: Vec<u16>,
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
    let mut text = wide("");
    let mut tool = TTTOOLINFOW {
      cbSize: size_of::<TTTOOLINFOW>() as u32,
      uFlags: TTF_TRACK | TTF_ABSOLUTE,
      hwnd: window,
      uId: 1,
      lpszText: PWSTR(text.as_mut_ptr()),
      ..Default::default()
    };
    unsafe {
      SendMessageW(
        window,
        TTM_ADDTOOLW,
        Some(WPARAM(0)),
        Some(LPARAM(&mut tool as *mut TTTOOLINFOW as isize)),
      );
      SendMessageW(window, TTM_SETMAXTIPWIDTH, Some(WPARAM(0)), Some(LPARAM(600)));
    }
    Ok(Self { window, tool, text })
  }

  /// Updates and displays the tooltip beside a screen point.
  fn update(&mut self, point: Point, text: &str) {
    self.text = wide(text);
    self.tool.lpszText = PWSTR(self.text.as_mut_ptr());
    let x = point.x() + TOOLTIP_OFFSET_X;
    let y = point.y() + TOOLTIP_OFFSET_Y;
    let packed_position = ((y as u32 & 0xffff) << 16) | (x as u32 & 0xffff);
    unsafe {
      SendMessageW(
        self.window,
        TTM_UPDATETIPTEXTW,
        Some(WPARAM(0)),
        Some(LPARAM(&mut self.tool as *mut TTTOOLINFOW as isize)),
      );
      SendMessageW(
        self.window,
        TTM_TRACKPOSITION,
        Some(WPARAM(0)),
        Some(LPARAM(packed_position as isize)),
      );
      SendMessageW(
        self.window,
        TTM_TRACKACTIVATE,
        Some(WPARAM(1)),
        Some(LPARAM(&mut self.tool as *mut TTTOOLINFOW as isize)),
      );
    }
  }
}

impl Drop for TrackingTooltip {
  /// Hides and destroys the native tooltip.
  fn drop(&mut self) {
    unsafe {
      SendMessageW(
        self.window,
        TTM_TRACKACTIVATE,
        Some(WPARAM(0)),
        Some(LPARAM(&mut self.tool as *mut TTTOOLINFOW as isize)),
      );
      if let Err(err) = DestroyWindow(self.window) {
        error!("Failed to destroy Window Picker tooltip: {err}");
      }
    }
  }
}

/// Encodes text as a null-terminated UTF-16 string.
fn wide(value: &str) -> Vec<u16> {
  value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn toggle_repeated_activation_cancels_instead_of_stacking_picker_sessions() {
    let mut session = PickerSession::default();
    assert_eq!(session.toggle(), PickerAction::Activate);
    assert_eq!(session.toggle(), PickerAction::Cancel);
    assert_eq!(session.status(), PickerStatus::Inactive);
  }

  #[test]
  fn finish_finishes_the_active_picker_session() {
    let mut session = PickerSession::default();
    session.toggle();
    session.finish();
    assert_eq!(session.status(), PickerStatus::Inactive);
  }

  #[test]
  fn result_content_preserves_unicode_and_explicit_empty_values() {
    let metadata = WindowMetadata {
      handle: crate::common::WindowHandle::new(42),
      title: "Résumé — 東京".to_string(),
      class_name: String::new(),
      rect: crate::common::Rect::default(),
    };

    assert_eq!(
      result_content(&metadata),
      "Window title:\nRésumé — 東京\n\nWindow class name:\n(empty)"
    );
  }

  #[test]
  fn dialog_action_maps_custom_actions_and_window_close() {
    assert_eq!(dialog_action(PICK_AGAIN_BUTTON_ID), ResultDialogAction::PickAgain);
    assert_eq!(dialog_action(CLOSE_BUTTON_ID), ResultDialogAction::Close);
    assert_eq!(dialog_action(2), ResultDialogAction::Close);
  }

  #[test]
  fn is_picker_passthrough_class_handles_tray_and_native_menu_clicks() {
    assert!(is_picker_passthrough_class("Shell_TrayWnd"));
    assert!(is_picker_passthrough_class("Shell_SecondaryTrayWnd"));
    assert!(is_picker_passthrough_class("#32768"));
    assert!(!is_picker_passthrough_class("EditorWindow"));
  }

  #[test]
  fn completion_gate_allows_only_one_outcome() {
    let gate = CompletionGate::default();
    assert!(gate.try_complete());
    assert!(!gate.try_complete());
    gate.reset();
    assert!(gate.try_complete());
  }
}
