use crate::api::WindowMetadata;
use crate::common::{Rect, WindowHandle};
use crate::window_picker::native_ui::*;
use crate::window_picker::window_picker::SelectionDialogChoice;
use windows::Win32::Foundation::{S_FALSE, S_OK};

fn metadata() -> WindowMetadata {
  WindowMetadata {
    handle: WindowHandle::new(42),
    title: "Document — 東京".to_string(),
    class_name: "EditorWindow".to_string(),
    rect: Rect::default(),
  }
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
fn handle_selection_dialog_button_writes_the_exact_title_and_keeps_the_dialogue_open_when_copying_title() {
  let mut copied_text = None;

  let result = handle_selection_dialog_button(COPY_TITLE_BUTTON_ID, &metadata(), |text| {
    copied_text = Some(text.to_string());
    Ok(())
  });

  assert_eq!(result, S_FALSE);
  assert_eq!(copied_text.as_deref(), Some("Document — 東京"));
}

#[test]
fn handle_selection_dialog_button_writes_the_exact_class_and_keeps_the_dialogue_open_when_copying_class() {
  let mut copied_text = None;

  let result = handle_selection_dialog_button(COPY_CLASS_BUTTON_ID, &metadata(), |text| {
    copied_text = Some(text.to_string());
    Ok(())
  });

  assert_eq!(result, S_FALSE);
  assert_eq!(copied_text.as_deref(), Some("EditorWindow"));
}

#[test]
fn handle_selection_dialog_button_closes_without_touching_the_clipboard_when_not_copying() {
  let result = handle_selection_dialog_button(CLOSE_BUTTON_ID, &metadata(), |_| {
    panic!("non-copy actions must not write to the clipboard")
  });

  assert_eq!(result, S_OK);
}

#[test]
fn utf16_encoder_appends_one_null_terminator() {
  assert_eq!(null_terminated_utf16("A"), vec![65, 0]);
}
