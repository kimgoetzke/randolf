use crate::common::WindowHandle;

/// Result of atomically positioning a window batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WindowPositioningResult {
  Applied,
  Rejected(Vec<WindowHandle>),
  BatchFailed,
}
