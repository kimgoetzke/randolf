use windows::Win32::Foundation::RECT;

#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone)]
pub struct Rect {
  pub left: i32,
  pub top: i32,
  pub right: i32,
  pub bottom: i32,
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
