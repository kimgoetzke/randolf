use crate::common::{Point, Rect, ResizeMode, WindowHandle};

/// Represents the state of a mouse-based window resize operation. Not used for any keyboard operations.
#[derive(Default)]
pub struct ResizeState {
  cursor_start_position: Point,
  window_start_rect: Rect,
  window_handle: Option<WindowHandle>,
  resize_mode: ResizeMode,
}

impl ResizeState {
  /// Sets the resize state when starting a resize operation. Called after a window is selected for resizing.
  pub(crate) fn set(
    &mut self,
    cursor_position: Point,
    window_handle: WindowHandle,
    window_rect: Rect,
    resize_mode: ResizeMode,
  ) {
    self.cursor_start_position = cursor_position;
    self.window_start_rect = window_rect;
    self.window_handle = Some(window_handle);
    self.resize_mode = resize_mode;
  }

  /// Returns the starting position of the cursor at the beginning of the resize operation.
  pub(crate) fn get_cursor_start_position(&self) -> Point {
    self.cursor_start_position
  }

  /// Returns the `Rect` of the window at the start of the resize operation.
  pub(crate) fn get_window_start_rect(&self) -> Rect {
    self.window_start_rect
  }

  /// Returns the resize mode for the current resize operation.
  pub(crate) fn get_resize_mode(&self) -> ResizeMode {
    self.resize_mode
  }

  /// Returns the window handle if available, otherwise returns `None`.
  pub(crate) fn get_window_handle(&self) -> Option<&WindowHandle> {
    if let Some(handle) = &self.window_handle {
      Some(handle)
    } else {
      error!("You have introduced a bug by trying to retrieve the handle before having set the resize state");

      None
    }
  }

  /// Resets the resize state. Should be called after the resize operation ends.
  pub(crate) fn reset(&mut self) {
    self.cursor_start_position = Point::default();
    self.window_start_rect = Rect::default();
    self.window_handle = None;
  }
}

#[cfg(test)]
mod tests {
  use crate::common::{Point, Rect, ResizeMode, ResizeState, WindowHandle};

  #[test]
  fn resize_state_has_default_values() {
    let state = ResizeState::default();
    assert_eq!(state.get_cursor_start_position(), Point::default());
    assert_eq!(state.get_window_start_rect(), Rect::default());
    assert!(state.get_window_handle().is_none());
    assert_eq!(state.get_resize_mode(), ResizeMode::default());
  }

  #[test]
  fn resize_state_can_be_set() {
    let mut state = ResizeState::default();
    let cursor_position = Point::new(100, 200);
    let window_handle = WindowHandle::from(12345);
    let window_rect = Rect::new(10, 20, 300, 400);
    let resize_mode = ResizeMode::TopLeft;

    state.set(cursor_position, window_handle, window_rect, resize_mode);

    assert_eq!(state.get_cursor_start_position(), cursor_position);
    assert_eq!(state.get_window_start_rect(), window_rect);
    assert_eq!(state.get_resize_mode(), resize_mode);
    assert_eq!(state.get_window_handle().unwrap().hwnd, 12345);
  }

  #[test]
  fn resize_state_can_be_reset() {
    let mut state = ResizeState::default();
    let cursor_position = Point::new(1000, 2000);
    let window_handle = WindowHandle::from(3);
    let window_rect = Rect::new(10, 20, 300, 400);
    let resize_mode = ResizeMode::TopRight;

    state.set(cursor_position, window_handle, window_rect, resize_mode);
    state.reset();

    assert_eq!(state.get_cursor_start_position(), Point::default());
    assert_eq!(state.get_window_start_rect(), Rect::default());
    assert!(state.get_window_handle().is_none());
  }

  #[test]
  fn get_window_handle_returns_none_if_not_set() {
    let resize_state = ResizeState::default();

    assert!(resize_state.get_window_handle().is_none());
  }

  #[test]
  fn get_window_handle_logs_error_when_not_set() {
    testing_logger::setup();
    let resize_state = ResizeState::default();

    assert!(resize_state.get_window_handle().is_none());
    testing_logger::validate(|captured_logs| {
      assert_eq!(captured_logs.len(), 1);
      assert_eq!(
        captured_logs[0].body,
        "You have introduced a bug by trying to retrieve the handle before having set the resize state"
      );
    });
  }
}
