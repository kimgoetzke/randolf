use std::fmt::Formatter;
use crate::utils::Rect;

#[derive(Clone, Debug)]
pub struct Point {
  x: i32,
  y: i32,
}

impl Point {
  pub fn new(x: i32, y: i32) -> Self {
    Self { x, y }
  }
  
  pub fn from_center_of_rect(rect: &Rect) -> Self {
    Self {
      x: rect.left + (rect.right - rect.left) / 2,
      y: rect.top + (rect.bottom - rect.top) / 2,
    }
  }

  pub fn x(&self) -> i32 {
    self.x
  }

  pub fn y(&self) -> i32 {
    self.y
  }
}

impl std::fmt::Display for Point {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "({}, {})", self.x, self.y)
  }
}
