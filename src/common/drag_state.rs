use crate::common::{Point, WindowHandle};

/// Represents the state of a mouse-based window move operation. Not used for any keyboard operations.
#[derive(Default)]
pub struct DragState {
  drag_start_position: Point,
  window_start_position: Point,
  window_handle: Option<WindowHandle>,
}

impl DragState {
  /// Sets the drag state when starting the drag operation. Only called after a window is selected for dragging.
  pub(crate) fn set(&mut self, cursor_position: Point, window_handle: WindowHandle, window_position: Point) {
    self.drag_start_position = cursor_position;
    self.window_start_position = window_position;
    self.window_handle = Some(window_handle);
  }

  /// Returns the starting position of the cursor at the beginning of the drag operation.
  pub(crate) fn get_drag_start_position(&self) -> Point {
    self.drag_start_position
  }

  /// Returns the starting position of the window at the beginning of the drag operation.
  pub(crate) fn get_window_start_position(&self) -> Point {
    self.window_start_position
  }

  /// Returns the window handle if available, otherwise returns `None`.
  pub(crate) fn get_window_handle(&self) -> Option<&WindowHandle> {
    if let Some(handle) = &self.window_handle {
      Some(handle)
    } else {
      error!("You have introduced a bug by trying to retrieve the handle before having set the drag state");

      None
    }
  }

  /// Resets the drag state. Should be called after the drag operation ends.
  pub(crate) fn reset(&mut self) {
    self.drag_start_position = Point::default();
    self.window_start_position = Point::default();
    self.window_handle = None;
  }
}

#[cfg(test)]
mod tests {
  use crate::common::{DragState, Point, WindowHandle};

  #[test]
  fn drag_state_has_default_values() {
    let drag_state = DragState::default();
    assert_eq!(drag_state.get_drag_start_position(), Point::default());
    assert_eq!(drag_state.get_window_start_position(), Point::default());
    assert!(drag_state.get_window_handle().is_none());
  }

  #[test]
  fn drag_state_can_be_set() {
    let mut drag_state = DragState::default();
    let cursor_position = Point::new(100, 100);
    let window_handle = WindowHandle::new(12345);
    let window_position = Point::new(200, 200);

    drag_state.set(cursor_position, window_handle, window_position);

    assert_eq!(drag_state.get_drag_start_position(), cursor_position);
    assert_eq!(drag_state.get_window_start_position(), window_position);
    assert_eq!(drag_state.get_window_handle().unwrap(), &window_handle);
  }

  #[test]
  fn drag_state_can_be_reset() {
    let mut drag_state = DragState::default();
    let cursor_position = Point::new(100, 100);
    let window_handle = WindowHandle::new(12345);
    let window_position = Point::new(200, 200);

    drag_state.set(cursor_position, window_handle, window_position);
    drag_state.reset();

    assert_eq!(drag_state.get_drag_start_position(), Point::default());
    assert_eq!(drag_state.get_window_start_position(), Point::default());
    assert!(drag_state.get_window_handle().is_none());
  }

  #[test]
  fn get_window_handle_returns_none_if_not_set() {
    let resize_state = DragState::default();

    assert!(resize_state.get_window_handle().is_none());
  }

  #[test]
  fn get_window_handle_logs_error_when_not_set() {
    testing_logger::setup();
    let resize_state = DragState::default();

    assert!(resize_state.get_window_handle().is_none());
    testing_logger::validate(|captured_logs| {
      assert_eq!(captured_logs.len(), 1);
      assert_eq!(
        captured_logs[0].body,
        "You have introduced a bug by trying to retrieve the handle before having set the drag state"
      );
    });
  }
}
