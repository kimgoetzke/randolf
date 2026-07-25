use crate::common::{Command, Point};
use crate::window_picker::native_ui::*;
use crate::window_picker::window_picker::SelectionDialogChoice;
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::WindowsAndMessaging::{WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_SYSKEYDOWN, WM_SYSKEYUP};

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
fn escape_transition_handles_regular_and_alt_modified_key_messages() {
  assert_eq!(escape_key_transition(WM_KEYDOWN), Some(KeyTransition::Pressed));
  assert_eq!(escape_key_transition(WM_SYSKEYDOWN), Some(KeyTransition::Pressed));
  assert_eq!(escape_key_transition(WM_KEYUP), Some(KeyTransition::Released));
  assert_eq!(escape_key_transition(WM_SYSKEYUP), Some(KeyTransition::Released));
  assert_eq!(escape_key_transition(WM_LBUTTONDOWN), None);
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
fn hook_callback_state_pairs_right_click_and_escape_transitions() {
  let state = HookCallbackState::new();

  assert!(!state.release_right_button());
  state.press_right_button();
  assert!(state.release_right_button());
  assert!(!state.release_right_button());

  assert!(!state.release_escape());
  state.press_escape();
  assert!(state.release_escape());
  assert!(!state.release_escape());
}

#[test]
fn utf16_encoder_appends_one_null_terminator() {
  assert_eq!(null_terminated_utf16("A"), vec![65, 0]);
}
