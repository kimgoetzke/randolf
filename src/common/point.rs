use crate::common::{Rect, Sizing};
use std::fmt::Formatter;
use windows::Win32::Foundation::POINT;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn new_creates_point_with_correct_coordinates() {
    let point = Point::new(-10, 20);

    assert_eq!(point.x(), -10);
    assert_eq!(point.y(), 20);
  }

  #[test]
  fn from_center_of_rect_calculates_correct_center() {
    let rect = Rect {
      left: 0,
      top: 0,
      right: 10,
      bottom: 20,
    };
    let point = Point::from_center_of_rect(&rect);

    assert_eq!(point, Point::new(5, 10));
  }

  #[test]
  fn from_center_of_sizing_calculates_correct_center() {
    let sizing = Sizing {
      x: 10,
      y: 20,
      width: 30,
      height: 40,
    };
    let point = Point::from_center_of_sizing(&sizing);

    assert_eq!(point, Point::new(25, 40));
  }

  #[test]
  fn distance_to_calculates_correct_distance() {
    let point1 = Point::new(0, 0);
    let point2 = Point::new(3, 4);

    assert_eq!(point1.distance_to(&point2), 5.0);
  }

  #[test]
  fn distance_to_same_point_is_zero() {
    let point = Point::new(10, 20);

    assert_eq!(point.distance_to(&point), 0.0);
  }

  #[test]
  fn as_point_converts_to_windows_point_correctly() {
    let point = Point::new(15, 25);
    let windows_point = point.as_point();

    assert_eq!(windows_point.x, 15);
    assert_eq!(windows_point.y, 25);
  }

  #[test]
  fn display_formats_point_correctly() {
    let point = Point::new(7, 14);

    assert_eq!(format!("{}", point), "(7, 14)");
  }
}
