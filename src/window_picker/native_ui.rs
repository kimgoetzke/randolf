use super::window_picker::{PickerSessionUi, PickerUi, SelectionDialogChoice, selection_dialog_content};
use crate::api::WindowMetadata;
use crate::common::{Command, Point};
use crossbeam_channel::Sender;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
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

const TOOLTIP_OFFSET_X: i32 = 18;
const TOOLTIP_OFFSET_Y: i32 = 24;
pub(super) const PICK_AGAIN_BUTTON_ID: i32 = 1001;
pub(super) const CLOSE_BUTTON_ID: i32 = 1002;
static G_HOOK_CALLBACK_STATE: HookCallbackState = HookCallbackState::new();

/// Allows one outcome command per picker session.
#[derive(Debug, Default)]
pub(super) struct CompletionGate {
  is_completed: AtomicBool,
}

impl CompletionGate {
  /// Creates an incomplete gate.
  const fn new() -> Self {
    Self {
      is_completed: AtomicBool::new(false),
    }
  }

  /// Opens the gate for a new picker session.
  pub(super) fn open(&self) {
    self.is_completed.store(false, Ordering::Release);
  }

  /// Closes the gate without producing an outcome.
  pub(super) fn close(&self) {
    self.is_completed.store(true, Ordering::Release);
  }

  /// Claims completion unless another attempt completed first.
  pub(super) fn try_complete(&self) -> bool {
    !self.is_completed.swap(true, Ordering::AcqRel)
  }
}

/// Stores state shared with Win32 callbacks, which cannot capture a picker session.
#[derive(Debug)]
pub(super) struct HookCallbackState {
  command_sender: OnceLock<Sender<Command>>,
  is_active: AtomicBool,
  is_left_button_down: AtomicBool,
  is_right_button_down: AtomicBool,
  is_escape_down: AtomicBool,
  selection_x: AtomicI32,
  selection_y: AtomicI32,
  completion_gate: CompletionGate,
}

impl HookCallbackState {
  pub(super) const fn new() -> Self {
    Self {
      command_sender: OnceLock::new(),
      is_active: AtomicBool::new(false),
      is_left_button_down: AtomicBool::new(false),
      is_right_button_down: AtomicBool::new(false),
      is_escape_down: AtomicBool::new(false),
      selection_x: AtomicI32::new(0),
      selection_y: AtomicI32::new(0),
      completion_gate: CompletionGate::new(),
    }
  }

  /// Enables event capture for a new session.
  pub(super) fn activate(&self, command_sender: Sender<Command>) {
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
    self.is_active.store(true, Ordering::Release);
  }

  /// Stops event capture and prevents late completion commands.
  fn deactivate(&self) {
    self.is_active.store(false, Ordering::Release);
    self.reset_pressed_inputs();
    self.completion_gate.close();
  }

  pub(super) fn is_active(&self) -> bool {
    self.is_active.load(Ordering::Acquire)
  }

  pub(super) fn press_left_button(&self, point: POINT) {
    self.selection_x.store(point.x, Ordering::Relaxed);
    self.selection_y.store(point.y, Ordering::Relaxed);
    self.is_left_button_down.store(true, Ordering::Relaxed);
  }

  pub(super) fn release_left_button(&self) -> Option<Point> {
    self.is_left_button_down.swap(false, Ordering::Relaxed).then(|| {
      Point::new(
        self.selection_x.load(Ordering::Relaxed),
        self.selection_y.load(Ordering::Relaxed),
      )
    })
  }

  pub(super) fn press_right_button(&self) {
    self.is_right_button_down.store(true, Ordering::Relaxed);
  }

  pub(super) fn release_right_button(&self) -> bool {
    self.is_right_button_down.swap(false, Ordering::Relaxed)
  }

  pub(super) fn press_escape(&self) {
    self.is_escape_down.store(true, Ordering::Relaxed);
  }

  pub(super) fn release_escape(&self) -> bool {
    self.is_escape_down.swap(false, Ordering::Relaxed)
  }

  /// Sends one outcome and immediately stops suppressing further input.
  pub(super) fn complete(&self, command: Command) {
    if !self.completion_gate.try_complete() {
      return;
    }
    self.is_active.store(false, Ordering::Release);
    if let Some(sender) = self.command_sender.get()
      && let Err(err) = sender.send(command)
    {
      error!("Failed to send Window Picker hook command: {err}");
    }
  }

  fn reset_pressed_inputs(&self) {
    self.is_left_button_down.store(false, Ordering::Relaxed);
    self.is_right_button_down.store(false, Ordering::Relaxed);
    self.is_escape_down.store(false, Ordering::Relaxed);
  }
}

/// Adapts the picker UI seam to Win32 hooks, tooltips, and task dialogues.
pub(super) struct NativePickerUi;

impl PickerUi for NativePickerUi {
  fn start_session(&mut self, command_sender: Sender<Command>) -> WindowsResult<Box<dyn PickerSessionUi>> {
    // Create all visible UI before capturing input. If hook installation fails, the tooltip drops automatically.
    let hover_tooltip = TrackingTooltip::new()?;
    let input_hooks = NativePickerHooks::install(command_sender)?;
    Ok(Box::new(NativePickerSessionUi {
      _input_hooks: input_hooks,
      hover_tooltip,
    }))
  }

  fn show_selection(&mut self, metadata: &WindowMetadata) -> WindowsResult<SelectionDialogChoice> {
    show_selection_dialog(metadata)
  }

  fn show_error(&mut self, message: &str) -> WindowsResult<()> {
    show_picker_error_dialog(message)
  }
}

/// Owns every native resource used while the user is choosing a window.
struct NativePickerSessionUi {
  _input_hooks: NativePickerHooks,
  hover_tooltip: TrackingTooltip,
}

impl PickerSessionUi for NativePickerSessionUi {
  fn show_hover_preview(&mut self, point: Point, text: &str) {
    self.hover_tooltip.show_at(point, text);
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
pub(super) fn selection_dialog_choice(button_id: i32) -> SelectionDialogChoice {
  if button_id == PICK_AGAIN_BUTTON_ID {
    SelectionDialogChoice::PickAgain
  } else {
    SelectionDialogChoice::Close
  }
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

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum KeyTransition {
  Pressed,
  Released,
}

/// Decodes regular and Alt-modified key messages into one transition model.
pub(super) fn escape_key_transition(message: u32) -> Option<KeyTransition> {
  match message {
    WM_KEYDOWN | WM_SYSKEYDOWN => Some(KeyTransition::Pressed),
    WM_KEYUP | WM_SYSKEYUP => Some(KeyTransition::Released),
    _ => None,
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

  match escape_key_transition(message.0 as u32) {
    Some(KeyTransition::Pressed) => {
      G_HOOK_CALLBACK_STATE.press_escape();
      LRESULT(1)
    }
    Some(KeyTransition::Released) if G_HOOK_CALLBACK_STATE.release_escape() => {
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
pub(super) fn is_input_passthrough_window_class(class_name: &str) -> bool {
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
pub(super) fn null_terminated_utf16(value: &str) -> Vec<u16> {
  value.encode_utf16().chain(std::iter::once(0)).collect()
}
