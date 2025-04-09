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
}
