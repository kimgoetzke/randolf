use crate::window_picker::native_ui::*;
use crate::window_picker::window_picker::SelectionDialogChoice;

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
fn utf16_encoder_appends_one_null_terminator() {
  assert_eq!(null_terminated_utf16("A"), vec![65, 0]);
}
