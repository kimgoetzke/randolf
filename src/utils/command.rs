use crate::utils::Direction;
use crate::utils::permanent_workspace_id::PersistentWorkspaceId;
use std::fmt::Display;

#[derive(Debug)]
pub enum Command {
  CloseWindow,
  NearMaximiseWindow,
  MoveWindow(Direction),
  MoveCursor(Direction),
  SwitchWorkspace(PersistentWorkspaceId),
  MoveWindowToWorkspace(PersistentWorkspaceId),
  OpenApplication(String, bool),
  Exit,
}

impl Display for Command {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Command::CloseWindow => write!(f, "Close window"),
      Command::NearMaximiseWindow => write!(f, "Near maximise window"),
      Command::MoveWindow(direction) => write!(f, "Move window [{:?}]", direction),
      Command::MoveCursor(direction) => write!(f, "Move cursor [{:?}]", direction),
      Command::SwitchWorkspace(id) => write!(f, "Switch to workspace {id}"),
      Command::MoveWindowToWorkspace(id) => write!(f, "Move window to workspace {id}"),
      Command::OpenApplication(path, as_admin) => write!(f, "Open [{path}] as admin [{as_admin}]"),
      Command::Exit => write!(f, "Exit application"),
    }
  }
}
