use super::native_hooks::NativeHooks;
use crate::common::{Command, Point};
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use windows::Win32::Foundation::E_FAIL;
use windows::core::{Error as WindowsError, Result as WindowsResult};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum InputEvent {
  LeftPressed(Point),
  LeftReleased,
  RightPressed,
  RightReleased,
  EscapePressed,
  EscapeReleased,
  PassthroughPointerPressed,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum InputDisposition {
  PassThrough,
  Suppress,
}

#[derive(Debug)]
enum CaptureStartError {
  AlreadyActive,
  DifferentCommandChannel,
}

/// Routes process-wide callbacks to the single active session.
pub(super) struct CaptureEndpoint {
  command_sender: OnceLock<Sender<Command>>,
  active_session: RwLock<Option<Arc<CaptureSession>>>,
}

impl CaptureEndpoint {
  const fn new() -> Self {
    Self {
      command_sender: OnceLock::new(),
      active_session: RwLock::new(None),
    }
  }

  fn activate(&self, command_sender: Sender<Command>) -> Result<ActiveCapture<'_>, CaptureStartError> {
    let mut active_session = self.active_session.write().unwrap_or_else(std::sync::PoisonError::into_inner);
    if active_session.is_some() {
      return Err(CaptureStartError::AlreadyActive);
    }
    if let Some(registered_sender) = self.command_sender.get() {
      if !registered_sender.same_channel(&command_sender) {
        return Err(CaptureStartError::DifferentCommandChannel);
      }
    } else {
      let _ = self.command_sender.set(command_sender.clone());
    }
    let session = Arc::new(CaptureSession::new(command_sender));
    *active_session = Some(Arc::clone(&session));
    Ok(ActiveCapture { endpoint: self, session })
  }

  pub(super) fn active_ingress(&self) -> Option<CaptureIngress> {
    let session = self
      .active_session
      .read()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .clone()?;
    session.is_active().then_some(CaptureIngress { session })
  }

  #[cfg(test)]
  fn dispatch(&self, event: InputEvent) -> InputDisposition {
    self
      .active_ingress()
      .map_or(InputDisposition::PassThrough, |ingress| ingress.dispatch(event))
  }

  fn deactivate(&self, session: &Arc<CaptureSession>) {
    session.deactivate();
    let mut active_session = self.active_session.write().unwrap_or_else(std::sync::PoisonError::into_inner);
    if active_session
      .as_ref()
      .is_some_and(|active_session| Arc::ptr_eq(active_session, session))
    {
      *active_session = None;
    }
  }
}

/// Owns one endpoint registration and deactivates it on drop.
struct ActiveCapture<'endpoint> {
  endpoint: &'endpoint CaptureEndpoint,
  session: Arc<CaptureSession>,
}

impl ActiveCapture<'_> {
  #[cfg(test)]
  fn ingress(&self) -> CaptureIngress {
    CaptureIngress {
      session: Arc::clone(&self.session),
    }
  }
}

impl Drop for ActiveCapture<'_> {
  fn drop(&mut self) {
    self.endpoint.deactivate(&self.session);
  }
}

/// Provides the event Seam shared by native callbacks and synthetic tests.
#[derive(Clone)]
pub(super) struct CaptureIngress {
  session: Arc<CaptureSession>,
}

impl CaptureIngress {
  pub(super) fn dispatch(&self, event: InputEvent) -> InputDisposition {
    self.session.dispatch(event)
  }
}

/// Implements gesture pairing and exactly-once completion for one session.
struct CaptureSession {
  command_sender: Sender<Command>,
  is_active: AtomicBool,
  is_left_button_down: AtomicBool,
  is_right_button_down: AtomicBool,
  is_escape_down: AtomicBool,
  selection_x: AtomicI32,
  selection_y: AtomicI32,
}

impl CaptureSession {
  fn new(command_sender: Sender<Command>) -> Self {
    Self {
      command_sender,
      is_active: AtomicBool::new(true),
      is_left_button_down: AtomicBool::new(false),
      is_right_button_down: AtomicBool::new(false),
      is_escape_down: AtomicBool::new(false),
      selection_x: AtomicI32::new(0),
      selection_y: AtomicI32::new(0),
    }
  }

  fn is_active(&self) -> bool {
    self.is_active.load(Ordering::Acquire)
  }

  fn dispatch(&self, event: InputEvent) -> InputDisposition {
    if !self.is_active() {
      return InputDisposition::PassThrough;
    }
    match event {
      InputEvent::LeftPressed(point) => {
        self.selection_x.store(point.x(), Ordering::Relaxed);
        self.selection_y.store(point.y(), Ordering::Relaxed);
        self.is_left_button_down.store(true, Ordering::Relaxed);
        InputDisposition::Suppress
      }
      InputEvent::LeftReleased if self.is_left_button_down.swap(false, Ordering::Relaxed) => {
        let point = Point::new(
          self.selection_x.load(Ordering::Relaxed),
          self.selection_y.load(Ordering::Relaxed),
        );
        self.complete(Command::WindowPickerSelected(point));
        InputDisposition::Suppress
      }
      InputEvent::LeftReleased => InputDisposition::PassThrough,
      InputEvent::RightPressed => {
        self.is_right_button_down.store(true, Ordering::Relaxed);
        InputDisposition::Suppress
      }
      InputEvent::RightReleased if self.is_right_button_down.swap(false, Ordering::Relaxed) => {
        self.complete(Command::CancelWindowPicker);
        InputDisposition::Suppress
      }
      InputEvent::RightReleased => InputDisposition::PassThrough,
      InputEvent::EscapePressed => {
        self.is_escape_down.store(true, Ordering::Relaxed);
        InputDisposition::Suppress
      }
      InputEvent::EscapeReleased if self.is_escape_down.swap(false, Ordering::Relaxed) => {
        self.complete(Command::CancelWindowPicker);
        InputDisposition::Suppress
      }
      InputEvent::EscapeReleased | InputEvent::PassthroughPointerPressed => InputDisposition::PassThrough,
    }
  }

  fn deactivate(&self) {
    self.is_active.store(false, Ordering::Release);
    self.is_left_button_down.store(false, Ordering::Relaxed);
    self.is_right_button_down.store(false, Ordering::Relaxed);
    self.is_escape_down.store(false, Ordering::Relaxed);
  }

  fn complete(&self, command: Command) {
    if self.is_active.swap(false, Ordering::AcqRel)
      && let Err(error) = self.command_sender.send(command)
    {
      error!("Failed to send Window Picker hook command: {error}");
    }
  }
}

pub(super) static G_INPUT_CAPTURE: CaptureEndpoint = CaptureEndpoint::new();

/// Exposes the native input-capture Module's drop-owned Interface.
pub(super) struct NativeInputSession {
  _capture: ActiveCapture<'static>,
  _hooks: NativeHooks,
}

impl NativeInputSession {
  /// Installs native hooks and starts one input session.
  pub(super) fn start(command_sender: Sender<Command>) -> WindowsResult<Self> {
    let hooks = NativeHooks::install()?;
    let capture = G_INPUT_CAPTURE
      .activate(command_sender)
      .map_err(CaptureStartError::into_windows_error)?;
    Ok(Self {
      _capture: capture,
      _hooks: hooks,
    })
  }
}

impl CaptureStartError {
  fn into_windows_error(self) -> WindowsError {
    let message = match self {
      Self::AlreadyActive => "a Window Picker input session is already active",
      Self::DifferentCommandChannel => "one process-wide Window Picker command channel must be used",
    };
    WindowsError::new(E_FAIL, message)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::common::{Command, Point};

  #[test]
  fn stale_session_ingress_cannot_affect_a_later_session() {
    let endpoint = CaptureEndpoint::new();
    let (command_sender, command_receiver) = crossbeam_channel::unbounded();
    let first_session = endpoint.activate(command_sender.clone()).unwrap();
    let stale_ingress = first_session.ingress();
    drop(first_session);
    let second_session = endpoint.activate(command_sender).unwrap();

    assert_eq!(
      stale_ingress.dispatch(InputEvent::LeftPressed(Point::new(12, 34))),
      InputDisposition::PassThrough
    );
    assert_eq!(
      second_session.ingress().dispatch(InputEvent::LeftReleased),
      InputDisposition::PassThrough
    );
    assert!(command_receiver.try_recv().is_err());
  }

  #[test]
  fn different_channel_is_rejected_after_a_session_ends() {
    let endpoint = CaptureEndpoint::new();
    let (first_sender, _first_receiver) = crossbeam_channel::unbounded();
    let (second_sender, _second_receiver) = crossbeam_channel::unbounded();
    drop(endpoint.activate(first_sender).unwrap());

    assert!(matches!(
      endpoint.activate(second_sender),
      Err(CaptureStartError::DifferentCommandChannel)
    ));
  }

  #[test]
  fn passthrough_press_does_not_arm_a_selection() {
    let endpoint = CaptureEndpoint::new();
    let (command_sender, command_receiver) = crossbeam_channel::unbounded();
    let _session = endpoint.activate(command_sender).unwrap();

    assert_eq!(
      endpoint.dispatch(InputEvent::PassthroughPointerPressed),
      InputDisposition::PassThrough
    );
    assert_eq!(endpoint.dispatch(InputEvent::LeftReleased), InputDisposition::PassThrough);
    assert!(command_receiver.try_recv().is_err());
  }

  #[test]
  fn second_active_session_is_rejected() {
    let endpoint = CaptureEndpoint::new();
    let (command_sender, _command_receiver) = crossbeam_channel::unbounded();
    let _session = endpoint.activate(command_sender.clone()).unwrap();

    assert!(matches!(
      endpoint.activate(command_sender),
      Err(CaptureStartError::AlreadyActive)
    ));
  }

  #[test]
  fn dropped_session_allows_reuse_with_the_same_channel() {
    let endpoint = CaptureEndpoint::new();
    let (command_sender, command_receiver) = crossbeam_channel::unbounded();
    let first_session = endpoint.activate(command_sender.clone()).unwrap();
    drop(first_session);

    let _second_session = endpoint.activate(command_sender).unwrap();
    assert_eq!(endpoint.dispatch(InputEvent::EscapePressed), InputDisposition::Suppress);
    assert_eq!(endpoint.dispatch(InputEvent::EscapeReleased), InputDisposition::Suppress);
    assert!(matches!(command_receiver.try_recv(), Ok(Command::CancelWindowPicker)));
  }

  #[test]
  fn concurrent_completion_gestures_emit_one_outcome() {
    let endpoint = CaptureEndpoint::new();
    let (command_sender, command_receiver) = crossbeam_channel::unbounded();
    let session = endpoint.activate(command_sender).unwrap();
    let ingress = session.ingress();
    assert_eq!(ingress.dispatch(InputEvent::RightPressed), InputDisposition::Suppress);
    assert_eq!(ingress.dispatch(InputEvent::EscapePressed), InputDisposition::Suppress);
    let barrier = Arc::new(std::sync::Barrier::new(2));

    std::thread::scope(|scope| {
      let right_ingress = ingress.clone();
      let right_barrier = Arc::clone(&barrier);
      let right_release = scope.spawn(move || {
        right_barrier.wait();
        right_ingress.dispatch(InputEvent::RightReleased)
      });
      let escape_barrier = Arc::clone(&barrier);
      let escape_release = scope.spawn(move || {
        escape_barrier.wait();
        ingress.dispatch(InputEvent::EscapeReleased)
      });
      let dispositions = [right_release.join().unwrap(), escape_release.join().unwrap()];
      assert!(dispositions.contains(&InputDisposition::Suppress));
    });

    assert!(matches!(command_receiver.try_recv(), Ok(Command::CancelWindowPicker)));
    assert!(command_receiver.try_recv().is_err());
  }

  #[test]
  fn completion_ignores_late_input() {
    let endpoint = CaptureEndpoint::new();
    let (command_sender, command_receiver) = crossbeam_channel::unbounded();
    let _session = endpoint.activate(command_sender).unwrap();

    assert_eq!(endpoint.dispatch(InputEvent::RightPressed), InputDisposition::Suppress);
    assert_eq!(endpoint.dispatch(InputEvent::RightReleased), InputDisposition::Suppress);
    assert_eq!(endpoint.dispatch(InputEvent::EscapePressed), InputDisposition::PassThrough);
    assert_eq!(endpoint.dispatch(InputEvent::EscapeReleased), InputDisposition::PassThrough);
    assert!(matches!(command_receiver.try_recv(), Ok(Command::CancelWindowPicker)));
    assert!(command_receiver.try_recv().is_err());
  }

  #[test]
  fn escape_cancels_only_after_a_paired_press() {
    let endpoint = CaptureEndpoint::new();
    let (command_sender, command_receiver) = crossbeam_channel::unbounded();
    let _session = endpoint.activate(command_sender).unwrap();

    assert_eq!(endpoint.dispatch(InputEvent::EscapeReleased), InputDisposition::PassThrough);
    assert_eq!(endpoint.dispatch(InputEvent::EscapePressed), InputDisposition::Suppress);
    assert_eq!(endpoint.dispatch(InputEvent::EscapeReleased), InputDisposition::Suppress);
    assert!(matches!(command_receiver.try_recv(), Ok(Command::CancelWindowPicker)));
  }

  #[test]
  fn right_click_cancels_only_after_a_paired_press() {
    let endpoint = CaptureEndpoint::new();
    let (command_sender, command_receiver) = crossbeam_channel::unbounded();
    let _session = endpoint.activate(command_sender).unwrap();

    assert_eq!(endpoint.dispatch(InputEvent::RightReleased), InputDisposition::PassThrough);
    assert_eq!(endpoint.dispatch(InputEvent::RightPressed), InputDisposition::Suppress);
    assert_eq!(endpoint.dispatch(InputEvent::RightReleased), InputDisposition::Suppress);
    assert!(matches!(command_receiver.try_recv(), Ok(Command::CancelWindowPicker)));
  }

  #[test]
  fn left_click_selects_the_press_point() {
    let endpoint = CaptureEndpoint::new();
    let (command_sender, command_receiver) = crossbeam_channel::unbounded();
    let _session = endpoint.activate(command_sender).unwrap();
    let press_point = Point::new(12, 34);

    assert_eq!(
      endpoint.dispatch(InputEvent::LeftPressed(press_point)),
      InputDisposition::Suppress
    );
    assert_eq!(endpoint.dispatch(InputEvent::LeftReleased), InputDisposition::Suppress);
    assert!(matches!(
      command_receiver.try_recv(),
      Ok(Command::WindowPickerSelected(point)) if point == press_point
    ));
  }
}
