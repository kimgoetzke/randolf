use crate::common::{Direction, PersistentWorkspaceId};
use std::fmt::Display;

#[derive(Debug)]
pub enum Command {
  CloseWindow,
  NearMaximiseWindow,
  MinimiseWindow,
  MoveWindow(Direction),
  MoveCursor(Direction),
  SwitchWorkspace(PersistentWorkspaceId),
  MoveWindowToWorkspace(PersistentWorkspaceId),
  OpenApplication(String, bool),
  OpenRandolfFolder,
  RestartRandolf,
  Exit,
}

impl Display for Command {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Command::CloseWindow => write!(f, "Close window"),
      Command::NearMaximiseWindow => write!(f, "Near maximise window"),
      Command::MinimiseWindow => write!(f, "Minimise window"),
      Command::MoveWindow(direction) => write!(f, "Move window [{:?}]", direction),
      Command::MoveCursor(direction) => write!(f, "Move cursor [{:?}]", direction),
      Command::SwitchWorkspace(id) => write!(f, "Switch to workspace [{id}]"),
      Command::MoveWindowToWorkspace(id) => write!(f, "Move window to workspace [{id}]"),
      Command::OpenApplication(path, as_admin) => write!(f, "Open [{path}] as admin [{as_admin}]"),
      Command::OpenRandolfFolder => write!(f, "Open Randolf folder in Explorer"),
      Command::RestartRandolf => write!(f, "Restart Randolf"),
      Command::Exit => write!(f, "Exit application"),
    }
  }
}
