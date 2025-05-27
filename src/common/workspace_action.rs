/// A simple enum representing actions that can be performed on a window by a [`Workspace`][ws]. Only really used to
/// communicate the action taken by the [`Workspace`][ws] back to the caller.
///
/// [ws]: crate::common::Workspace
pub enum WorkspaceAction {
  Moved,
  Stored,
}
