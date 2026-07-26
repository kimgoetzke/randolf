use crate::api::{NativePickerUi, WindowLookupError, WindowMetadata, WindowsApi};
use crate::common::{Command, Point};
use crossbeam_channel::Sender;
use std::time::{Duration, Instant};
use windows::core::Result as WindowsResult;

pub(super) const HOVER_INTERVAL: Duration = Duration::from_millis(75);

/// User choice from the dialogue showing the selected window's metadata.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum SelectionDialogChoice {
  PickAgain,
  Close,
}

/// Displays session UI without exposing native Win32 resources to picker orchestration.
pub(crate) trait PickerSessionUi {
  fn show_hover_preview(&mut self, point: Point, text: &str);
}

/// Creates picker UI sessions and presents blocking dialogues.
pub(crate) trait PickerUi {
  fn start_session(&mut self, command_sender: Sender<Command>) -> WindowsResult<Box<dyn PickerSessionUi>>;
  fn show_selection(&mut self, metadata: &WindowMetadata) -> WindowsResult<SelectionDialogChoice>;
  fn show_error(&mut self, message: &str) -> WindowsResult<()>;
}

/// Limits hover lookups and native tooltip updates to one per configured interval.
pub(super) struct HoverRefreshTimer {
  last_refresh: Instant,
}

impl HoverRefreshTimer {
  pub(super) fn ready() -> Self {
    Self {
      last_refresh: Instant::now() - HOVER_INTERVAL,
    }
  }

  pub(super) fn try_begin_refresh(&mut self, now: Instant) -> bool {
    if now.duration_since(self.last_refresh) < HOVER_INTERVAL {
      return false;
    }
    self.last_refresh = now;
    true
  }
}

/// Owns one active UI session and its hover-refresh timing.
struct ActivePickerSession {
  ui: Box<dyn PickerSessionUi>,
  hover_refresh_timer: HoverRefreshTimer,
}

impl ActivePickerSession {
  fn new(ui: Box<dyn PickerSessionUi>) -> Self {
    Self {
      ui,
      hover_refresh_timer: HoverRefreshTimer::ready(),
    }
  }

  fn refresh_hover_preview<Api: WindowsApi>(&mut self, api: &Api) {
    if !self.hover_refresh_timer.try_begin_refresh(Instant::now()) {
      return;
    }
    let cursor_position = api.get_cursor_position();
    let text = match api.get_window_at_point(cursor_position) {
      Ok(metadata) => hover_text(&metadata),
      Err(error) => hover_error_text(error),
    };
    self.ui.show_hover_preview(cursor_position, &text);
  }
}

/// Coordinates Window Picker commands and owns the current native picking session.
///
/// Create one instance per process: Win32 callbacks register its command channel in process-wide callback state.
pub struct WindowPicker<Api: WindowsApi> {
  windows_api: Api,
  command_sender: Sender<Command>,
  ui: Box<dyn PickerUi>,
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
    Self::with_ui(windows_api, command_sender, Box::new(NativePickerUi))
  }

  pub(super) fn with_ui(windows_api: Api, command_sender: Sender<Command>, ui: Box<dyn PickerUi>) -> Self {
    Self {
      windows_api,
      command_sender,
      ui,
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
      self.report_error("Failed to start Window Picker", &error);
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
        self.report_error("Window Picker selection failed", &error);
        return;
      }
    };
    match self.ui.show_selection(&metadata) {
      Ok(SelectionDialogChoice::PickAgain) => {
        if let Err(error) = self.activate() {
          self.report_error("Failed to restart Window Picker", &error);
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
    let session_ui = self.ui.start_session(self.command_sender.clone())?;
    self.active_session = Some(ActivePickerSession::new(session_ui));
    Ok(())
  }

  /// Releases native resources before resolving the selected window.
  fn select(&mut self, point: Point) -> Result<WindowMetadata, WindowLookupError> {
    self.cancel();
    self.windows_api.get_window_at_point(point)
  }

  /// Logs a picker failure and attempts to present it to the user.
  fn report_error(&mut self, context: &str, error: &impl std::fmt::Display) {
    let message = format!("{context}: {error}");
    error!("{message}");
    if let Err(dialog_error) = self.ui.show_error(&message) {
      error!("Failed to show Window Picker error dialogue: {dialog_error}");
    }
  }
}

/// Formats selected window metadata for the result dialogue.
pub(crate) fn selection_dialog_content(metadata: &WindowMetadata) -> String {
  format!(
    "Window title:\n{}\n\nWindow class name:\n{}",
    display_value(&metadata.title),
    display_value(&metadata.class_name)
  )
}

/// Formats selected window metadata for the tracking tooltip.
pub(super) fn hover_text(metadata: &WindowMetadata) -> String {
  format!(
    "Window title: {}\nWindow class name: {}\nClick to select · Esc/right-click cancels",
    display_value(&metadata.title),
    display_value(&metadata.class_name)
  )
}

/// Formats a lookup failure for the tracking tooltip.
pub(super) fn hover_error_text(error: WindowLookupError) -> String {
  format!("Window Picker\n{error}\nClick a window · Esc/right-click cancels")
}

/// Replaces an empty metadata value with an explicit placeholder.
fn display_value(value: &str) -> &str {
  if value.is_empty() { "(empty)" } else { value }
}
