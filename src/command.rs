use crate::direction::Direction;
use std::fmt::Display;

#[derive(Debug)]
pub enum Command {
  CloseWindow,
  NearMaximiseWindow,
  MoveWindow(Direction),
  MoveCursorToWindowInDirection(Direction),
  Exit,
}

impl Display for Command {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Command::CloseWindow => write!(f, "Close window"),
      Command::NearMaximiseWindow => write!(f, "Near maximise window"),
      Command::MoveWindow(direction) => write!(f, "Move window [{:?}]", direction),
      Command::MoveCursorToWindowInDirection(direction) => {
        write!(f, "Move cursor to window in direction [{:?}]", direction)
      }
      Command::Exit => write!(f, "Exit"),
    }
  }
}
