use std::fmt::{Display, Formatter};

/// Failure to identify a visible top-level window.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum WindowLookupError {
  NoTarget,
  Vanished,
  OwnWindow,
  AccessDenied,
}

impl Display for WindowLookupError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::NoTarget => write!(f, "No window was found at that point"),
      Self::Vanished => write!(f, "The selected window is no longer available"),
      Self::OwnWindow => write!(f, "Randolf's Window Picker cannot select itself"),
      Self::AccessDenied => write!(f, "Randolf does not have permission to inspect that window"),
    }
  }
}

impl std::error::Error for WindowLookupError {}
