use crate::common::Point;
use std::fmt::Display;
use windows::Win32::Foundation::RECT;

#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone, Default)]
pub struct Rect {
  pub left: i32,
  pub top: i32,
  pub right: i32,
  pub bottom: i32,
}

impl Rect {
  pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
    Self {
      top,
      left,
      right,
      bottom,
    }
  }

  pub fn width(&self) -> i32 {
    self.right - self.left
  }

  pub fn height(&self) -> i32 {
    self.bottom - self.top
  }

  pub fn area(&self) -> i32 {
    (self.right - self.left) * (self.bottom - self.top)
  }

  pub fn center(&self) -> Point {
    Point::new((self.left + self.right) / 2, (self.top + self.bottom) / 2)
  }

  pub fn contains(&self, point: &Point) -> bool {
    point.x() >= self.left && point.x() <= self.right && point.y() >= self.top && point.y() <= self.bottom
  }

  pub fn intersects(&self, other: &Self) -> bool {
    self.left < other.right && self.right > other.left && self.top < other.bottom && self.bottom > other.top
  }

  pub fn clamp(&self, other: &Self, margin: i32) -> Self {
    Self {
      left: self.left.max(other.left + margin),
      top: self.top.max(other.top + margin),
      right: self.right.min(other.right - margin),
      bottom: self.bottom.min(other.bottom - margin),
    }
  }
}

impl From<RECT> for Rect {
  fn from(value: RECT) -> Self {
    Self {
      left: value.left,
      top: value.top,
      right: value.right,
      bottom: value.bottom,
    }
  }
}

#[allow(clippy::from_over_into)]
impl Into<RECT> for Rect {
  fn into(self) -> RECT {
    RECT {
      left: self.left,
      top: self.top,
      right: self.right,
      bottom: self.bottom,
    }
  }
}

impl Display for Rect {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "Rect[({}, {})-({}, {}), width: {}, height: {}]",
      self.left,
      self.top,
      self.right,
      self.bottom,
      self.width(),
      self.height()
    )
  }
}

#[cfg(test)]
mod tests {
  use crate::common::Rect;
  use windows::Win32::Foundation::RECT;

  #[test]
  fn new_creates_rect_with_correct_coordinates() {
    let rect = Rect::new(1, 2, 3, 4);

    assert_eq!(rect.left, 1);
    assert_eq!(rect.top, 2);
    assert_eq!(rect.right, 3);
    assert_eq!(rect.bottom, 4);
  }

  #[test]
  fn from_windows_rect_converts_correctly() {
    let windows_rect = RECT {
      left: 5,
      top: 6,
      right: 7,
      bottom: 8,
    };
    let rect: Rect = windows_rect.into();

    assert_eq!(rect.left, 5);
    assert_eq!(rect.top, 6);
    assert_eq!(rect.right, 7);
    assert_eq!(rect.bottom, 8);
  }

  #[test]
  fn into_windows_rect_converts_correctly() {
    let rect = Rect::new(9, 10, 11, 12);
    let windows_rect: RECT = rect.into();

    assert_eq!(windows_rect.left, 9);
    assert_eq!(windows_rect.top, 10);
    assert_eq!(windows_rect.right, 11);
    assert_eq!(windows_rect.bottom, 12);
  }

  #[test]
  fn width_and_height_calculates_correctly_1() {
    let rect = Rect::new(0, 0, 20, 10);

    assert_eq!(rect.width(), 20);
    assert_eq!(rect.height(), 10);
  }

  #[test]
  fn width_and_height_calculates_correctly_2() {
    let rect = Rect::new(-10, -10, 20, 10);

    assert_eq!(rect.width(), 30);
    assert_eq!(rect.height(), 20);
  }

  #[test]
  fn area_calculates_correctly_for_positive_coordinates() {
    let rect = Rect::new(0, 0, 5, 5);

    assert_eq!(rect.area(), 25);
  }

  #[test]
  fn area_is_zero_when_width_or_height_is_zero() {
    let rect_zero_width = Rect::new(1, 2, 1, 6);
    let rect_zero_height = Rect::new(1, 2, 4, 2);

    assert_eq!(rect_zero_width.area(), 0);
    assert_eq!(rect_zero_height.area(), 0);
  }

  #[test]
  fn area_handles_negative_coordinates_correctly() {
    let rect = Rect::new(-3, -2, 1, 2);

    assert_eq!(rect.area(), 16);
  }

  #[test]
  fn center_calculates_correctly_for_zero_size_rect() {
    let rect = Rect::new(3, 3, 3, 3);
    let center = rect.center();

    assert_eq!(center.x(), 3);
    assert_eq!(center.y(), 3);
  }

  #[test]
  fn center_calculates_correctly_for_mixed_coordinates() {
    let rect = Rect::new(-4, -4, 4, 4);
    let center = rect.center();

    assert_eq!(center.x(), 0);
    assert_eq!(center.y(), 0);
  }

  #[test]
  fn clamp_restricts_rect_within_bounds() {
    let rect = Rect::new(0, 0, 1920, 1080);
    let bounds = Rect::new(0, 0, 1024, 768);

    let clamped = rect.clamp(&bounds, 0);

    assert_eq!(clamped.left, 0);
    assert_eq!(clamped.top, 0);
    assert_eq!(clamped.right, 1024);
    assert_eq!(clamped.bottom, 768);
  }

  #[test]
  fn clamp_applies_margin() {
    let rect = Rect::new(0, 0, 1920, 1080);
    let bounds = Rect::new(0, 0, 1024, 768);

    let clamped = rect.clamp(&bounds, 20);

    assert_eq!(clamped.left, 20);
    assert_eq!(clamped.top, 20);
    assert_eq!(clamped.right, 1004);
    assert_eq!(clamped.bottom, 748);
  }

  #[test]
  fn clamp_handles_negative_values() {
    let rect = Rect::new(-1920, -1080, 0, 0);
    let bounds = Rect::new(-800, -600, 0, 0);

    let clamped = rect.clamp(&bounds, 20);

    assert_eq!(clamped.left, -780);
    assert_eq!(clamped.top, -580);
    assert_eq!(clamped.right, -20);
    assert_eq!(clamped.bottom, -20);
  }

  #[test]
  fn intersects_returns_true_for_overlapping_rects() {
    let rect1 = Rect::new(0, 0, 10, 10);
    let rect2 = Rect::new(5, 5, 15, 15);

    assert!(rect1.intersects(&rect2));
  }

  #[test]
  fn intersects_returns_false_for_non_overlapping_rects() {
    let rect1 = Rect::new(0, 0, 10, 10);
    let rect2 = Rect::new(20, 20, 30, 30);

    assert!(!rect1.intersects(&rect2));
  }

  #[test]
  fn intersects_returns_true_for_touching_rects() {
    let rect1 = Rect::new(0, 0, 10, 10);
    let rect2 = Rect::new(10, 10, 20, 20);

    assert!(!rect1.intersects(&rect2));
  }

  #[test]
  fn intersects_returns_false_for_adjacent_rects_without_overlap() {
    let rect1 = Rect::new(0, 0, 10, 10);
    let rect2 = Rect::new(10, 0, 20, 10);

    assert!(!rect1.intersects(&rect2));
  }

  #[test]
  fn intersects_handles_negative_coordinates_correctly() {
    let rect1 = Rect::new(-10, -10, 0, 0);
    let rect2 = Rect::new(-5, -5, 5, 5);

    assert!(rect1.intersects(&rect2));
  }

  #[test]
  fn intersects_returns_true_for_completely_contained_rects_1() {
    let rect1 = Rect::new(0, 0, 10, 10);
    let rect2 = Rect::new(2, 2, 8, 8);

    assert!(rect2.intersects(&rect1));
  }

  #[test]
  fn intersects_returns_true_for_completely_contained_rects_2() {
    let rect1 = Rect::new(0, 0, 10, 10);
    let rect2 = Rect::new(2, 2, 8, 8);

    assert!(rect1.intersects(&rect2));
  }
}
