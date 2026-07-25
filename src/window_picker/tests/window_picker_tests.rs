use crate::api::{MockWindowsApi, WindowLookupError, WindowMetadata};
use crate::common::{Command, Point};
use crate::window_picker::window_picker::*;
use crossbeam_channel::Sender;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::E_FAIL;
use windows::core::{Error as WindowsError, Result as WindowsResult};

#[derive(Default)]
struct FakePickerUiState {
  start_attempts: usize,
  active_sessions: usize,
  ended_sessions: usize,
  fail_next_start: bool,
  fail_selection_dialog: bool,
  selection_choice: Option<SelectionDialogChoice>,
  shown_selections: Vec<WindowMetadata>,
  shown_errors: Vec<String>,
  hover_previews: Vec<(Point, String)>,
}

struct FakePickerUi {
  state: Rc<RefCell<FakePickerUiState>>,
}

struct FakePickerSessionUi {
  state: Rc<RefCell<FakePickerUiState>>,
}

impl Drop for FakePickerSessionUi {
  fn drop(&mut self) {
    let mut state = self.state.borrow_mut();
    state.active_sessions -= 1;
    state.ended_sessions += 1;
  }
}

impl PickerSessionUi for FakePickerSessionUi {
  fn show_hover_preview(&mut self, point: Point, text: &str) {
    self.state.borrow_mut().hover_previews.push((point, text.to_string()));
  }
}

impl PickerUi for FakePickerUi {
  fn start_session(&mut self, _command_sender: Sender<Command>) -> WindowsResult<Box<dyn PickerSessionUi>> {
    let mut state = self.state.borrow_mut();
    state.start_attempts += 1;
    if state.fail_next_start {
      state.fail_next_start = false;
      return Err(WindowsError::from_hresult(E_FAIL));
    }
    state.active_sessions += 1;
    drop(state);
    Ok(Box::new(FakePickerSessionUi {
      state: Rc::clone(&self.state),
    }))
  }

  fn show_selection(&mut self, metadata: &WindowMetadata) -> WindowsResult<SelectionDialogChoice> {
    let mut state = self.state.borrow_mut();
    state.shown_selections.push(metadata.clone());
    if state.fail_selection_dialog {
      return Err(WindowsError::from_hresult(E_FAIL));
    }
    Ok(state.selection_choice.take().unwrap_or(SelectionDialogChoice::Close))
  }

  fn show_error(&mut self, message: &str) -> WindowsResult<()> {
    self.state.borrow_mut().shown_errors.push(message.to_string());
    Ok(())
  }
}

fn picker() -> (WindowPicker<MockWindowsApi>, Rc<RefCell<FakePickerUiState>>) {
  MockWindowsApi::reset();
  let (command_sender, _command_receiver) = crossbeam_channel::unbounded();
  let state = Rc::new(RefCell::new(FakePickerUiState::default()));
  let picker = WindowPicker::with_ui(
    MockWindowsApi::new(),
    command_sender,
    Box::new(FakePickerUi {
      state: Rc::clone(&state),
    }),
  );
  (picker, state)
}

fn metadata(title: &str, class_name: &str) -> WindowMetadata {
  WindowMetadata {
    handle: crate::common::WindowHandle::new(42),
    title: title.to_string(),
    class_name: class_name.to_string(),
    rect: crate::common::Rect::default(),
  }
}

fn selectable_window(point: Point) {
  let handle = crate::common::WindowHandle::new(42);
  MockWindowsApi::add_or_update_window_with_class(
    handle,
    "Document".to_string(),
    "EditorWindow".to_string(),
    crate::common::Sizing::new(10, 20, 800, 600),
    false,
    false,
    true,
  );
  MockWindowsApi::set_point_target(point, handle, handle);
}

#[test]
fn toggle_starts_one_session_and_second_toggle_cancels_it() {
  let (mut picker, state) = picker();

  picker.handle_toggle();
  assert_eq!(state.borrow().start_attempts, 1);
  assert_eq!(state.borrow().active_sessions, 1);

  picker.handle_toggle();
  assert_eq!(state.borrow().active_sessions, 0);
  assert_eq!(state.borrow().ended_sessions, 1);
}

#[test]
fn hover_preview_uses_current_window_metadata() {
  let (mut picker, state) = picker();
  let point = Point::new(10, 20);
  selectable_window(point);
  MockWindowsApi::set_cursor_position(point);
  picker.handle_toggle();

  picker.refresh_hover_preview();

  assert_eq!(
    state.borrow().hover_previews,
    vec![(
      point,
      "Window title: Document\nWindow class name: EditorWindow\nClick to select · Esc/right-click cancels".to_string()
    )]
  );
}

#[test]
fn hover_refresh_timer_suppresses_updates_until_the_interval_elapses() {
  let mut timer = HoverRefreshTimer::ready();
  let start = Instant::now();

  assert!(timer.try_begin_refresh(start));
  assert!(!timer.try_begin_refresh(start + HOVER_INTERVAL - Duration::from_millis(1)));
  assert!(timer.try_begin_refresh(start + HOVER_INTERVAL));
}

#[test]
fn hover_lookup_failure_is_presented_without_ending_the_session() {
  let (mut picker, state) = picker();
  MockWindowsApi::set_cursor_position(Point::new(10, 20));
  picker.handle_toggle();

  picker.refresh_hover_preview();

  let state = state.borrow();
  assert_eq!(state.active_sessions, 1);
  assert_eq!(state.hover_previews.len(), 1);
  assert!(state.hover_previews[0].1.contains("No window was found at that point"));
}

#[test]
fn selection_releases_the_session_and_presents_frozen_metadata() {
  let (mut picker, state) = picker();
  let point = Point::new(10, 20);
  selectable_window(point);
  picker.handle_toggle();

  picker.handle_selection(point);

  let state = state.borrow();
  assert_eq!(state.active_sessions, 0);
  assert_eq!(state.ended_sessions, 1);
  let [selection] = state.shown_selections.as_slice() else {
    panic!("expected exactly one selected window");
  };
  assert_eq!(selection.title, "Document");
  assert_eq!(selection.class_name, "EditorWindow");
  assert_eq!(selection.rect, crate::common::Rect::new(10, 20, 810, 620));
}

#[test]
fn pick_again_replaces_the_completed_session_with_a_fresh_session() {
  let (mut picker, state) = picker();
  let point = Point::new(10, 20);
  selectable_window(point);
  state.borrow_mut().selection_choice = Some(SelectionDialogChoice::PickAgain);
  picker.handle_toggle();

  picker.handle_selection(point);

  let state = state.borrow();
  assert_eq!(state.start_attempts, 2);
  assert_eq!(state.active_sessions, 1);
  assert_eq!(state.ended_sessions, 1);
}

#[test]
fn selection_dialogue_failure_leaves_the_completed_session_inactive() {
  let (mut picker, state) = picker();
  let point = Point::new(10, 20);
  selectable_window(point);
  state.borrow_mut().fail_selection_dialog = true;
  picker.handle_toggle();

  picker.handle_selection(point);

  let state = state.borrow();
  assert_eq!(state.active_sessions, 0);
  assert_eq!(state.ended_sessions, 1);
  assert_eq!(state.shown_selections.len(), 1);
  assert_eq!(state.start_attempts, 1);
}

#[test]
fn failed_pick_again_restart_is_reported_and_remains_inactive() {
  let (mut picker, state) = picker();
  let point = Point::new(10, 20);
  selectable_window(point);
  picker.handle_toggle();
  {
    let mut state = state.borrow_mut();
    state.selection_choice = Some(SelectionDialogChoice::PickAgain);
    state.fail_next_start = true;
  }

  picker.handle_selection(point);

  let state = state.borrow();
  assert_eq!(state.start_attempts, 2);
  assert_eq!(state.active_sessions, 0);
  assert_eq!(state.ended_sessions, 1);
  assert_eq!(state.shown_errors.len(), 1);
  assert!(state.shown_errors[0].contains("Failed to restart Window Picker"));
}

#[test]
fn stale_selection_after_cancellation_is_ignored() {
  let (mut picker, state) = picker();
  picker.handle_toggle();
  picker.cancel();

  picker.handle_selection(Point::new(10, 20));

  let state = state.borrow();
  assert!(state.shown_selections.is_empty());
  assert!(state.shown_errors.is_empty());
  assert_eq!(state.ended_sessions, 1);
}

#[test]
fn lookup_failure_releases_the_session_and_reports_the_reason() {
  let (mut picker, state) = picker();
  picker.handle_toggle();

  picker.handle_selection(Point::new(10, 20));

  let state = state.borrow();
  assert_eq!(state.active_sessions, 0);
  assert_eq!(state.ended_sessions, 1);
  assert!(state.shown_selections.is_empty());
  assert_eq!(state.shown_errors.len(), 1);
  assert!(state.shown_errors[0].contains("No window was found at that point"));
}

#[test]
fn activation_failure_is_reported_without_leaving_an_active_session() {
  let (mut picker, state) = picker();
  state.borrow_mut().fail_next_start = true;

  picker.handle_toggle();

  let state = state.borrow();
  assert_eq!(state.start_attempts, 1);
  assert_eq!(state.active_sessions, 0);
  assert_eq!(state.shown_errors.len(), 1);
  assert!(state.shown_errors[0].contains("Failed to start Window Picker"));
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
