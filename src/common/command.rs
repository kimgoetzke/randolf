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
  DragWindows(bool),
  OpenApplication(String, bool),
  OpenRandolfExecutableFolder,
  OpenRandolfConfigFolder,
  OpenRandolfDataFolder,
  RestartRandolf(bool),
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
      Command::DragWindows(is_allowed) => write!(f, "Allow window dragging [{}]", is_allowed),
      Command::OpenApplication(path, as_admin) => write!(f, "Open [{path}] as admin [{as_admin}]"),
      Command::OpenRandolfExecutableFolder => write!(f, "Open Randolf's executable folder in Explorer"),
      Command::OpenRandolfConfigFolder => write!(f, "Open Randolf's config folder in Explorer"),
      Command::OpenRandolfDataFolder => write!(f, "Open Randolf's data folder in Explorer"),
      Command::RestartRandolf(as_admin) => write!(f, "Restart Randolf as admin [{as_admin}]"),
      Command::Exit => write!(f, "Exit application"),
    }
  }
}
