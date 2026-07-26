use crate::common::{Rect, WindowHandle};

/// Information about a visible top-level window.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowMetadata {
  pub handle: WindowHandle,
  pub title: String,
  pub class_name: String,
  pub rect: Rect,
}
