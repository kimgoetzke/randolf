use super::input_capture::NativeInputSession;
use super::window_picker::{PickerSessionUi, PickerUi, SelectionDialogChoice, selection_dialog_content};
use crate::api::WindowMetadata;
use crate::common::{Command, Point};
use crossbeam_channel::Sender;
use windows::Win32::Foundation::{E_FAIL, HWND, LPARAM, WPARAM};
use windows::Win32::UI::Controls::{
  InitCommonControls, TASKDIALOG_BUTTON, TASKDIALOGCONFIG, TDCBF_CLOSE_BUTTON, TDF_ALLOW_DIALOG_CANCELLATION,
  TDF_SIZE_TO_CONTENT, TDF_USE_COMMAND_LINKS, TOOLTIPS_CLASSW, TTF_ABSOLUTE, TTF_TRACK, TTM_ADDTOOLW, TTM_SETMAXTIPWIDTH,
  TTM_TRACKACTIVATE, TTM_TRACKPOSITION, TTM_UPDATETIPTEXTW, TTS_ALWAYSTIP, TTS_NOPREFIX, TTTOOLINFOW, TaskDialog,
  TaskDialogIndirect,
};
use windows::Win32::UI::WindowsAndMessaging::{
  CreateWindowExW, DestroyWindow, SendMessageW, WINDOW_EX_STYLE, WINDOW_STYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
  WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{Error as WindowsError, PCWSTR, PWSTR, Result as WindowsResult, w};

const TOOLTIP_OFFSET_X: i32 = 18;
const TOOLTIP_OFFSET_Y: i32 = 24;
pub(super) const PICK_AGAIN_BUTTON_ID: i32 = 1001;
pub(super) const CLOSE_BUTTON_ID: i32 = 1002;
/// Adapts the picker UI seam to Win32 hooks, tooltips, and task dialogues.
pub(super) struct NativePickerUi;

impl PickerUi for NativePickerUi {
  fn start_session(&mut self, command_sender: Sender<Command>) -> WindowsResult<Box<dyn PickerSessionUi>> {
    // Create all visible UI before capturing input. If hook installation fails, the tooltip drops automatically.
    let hover_tooltip = TrackingTooltip::new()?;
    let input_capture = NativeInputSession::start(command_sender)?;
    Ok(Box::new(NativePickerSessionUi {
      _input_capture: input_capture,
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
  _input_capture: NativeInputSession,
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
