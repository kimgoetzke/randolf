use crate::utils::{Rect, Sizing};
use std::fmt::Formatter;
use windows::Win32::Foundation::POINT;

#[derive(Clone, Copy, Debug, PartialEq)]
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

  pub fn from_center_of_sizing(sizing: &Sizing) -> Self {
    Self {
      x: sizing.x + sizing.width / 2,
      y: sizing.y + sizing.height / 2,
    }
  }

  pub fn distance_to(&self, other: &Point) -> f64 {
    let x = (other.x - self.x) as f64;
    let y = (other.y - self.y) as f64;
    (x * x + y * y).sqrt()
  }

  pub fn x(&self) -> i32 {
    self.x
  }

  pub fn y(&self) -> i32 {
    self.y
  }

  pub fn as_point(&self) -> POINT {
    POINT { x: self.x, y: self.y }
  }
}

#[allow(clippy::from_over_into)]
impl Into<POINT> for &Point {
  fn into(self) -> POINT {
    POINT { x: self.x, y: self.y }
  }
}

impl std::fmt::Display for Point {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "({}, {})", self.x, self.y)
  }
}
