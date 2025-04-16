use crate::utils::Point;
use std::fmt::Display;
use windows::Win32::Foundation::RECT;

#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone)]
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
  use crate::utils::Rect;
  use windows::Win32::Foundation::RECT;

  impl Rect {
    pub fn default() -> Self {
      Self {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
      }
    }
  }

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
}
