/// An enum representing the four cardinal directions. Used for movement and positioning logic in the application
/// e.g. when moving the cursor using keyboard shortcuts, or to locate monitor work areas relative to each other.
#[derive(Debug, Clone, Copy)]
pub enum Direction {
  Left,
  Right,
  Up,
  Down,
}

impl Direction {
  pub fn opposite(self) -> Self {
    match self {
      Direction::Left => Direction::Right,
      Direction::Right => Direction::Left,
      Direction::Up => Direction::Down,
      Direction::Down => Direction::Up,
    }
  }
}
