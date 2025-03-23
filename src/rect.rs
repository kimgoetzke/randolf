use windows::Win32::Foundation::RECT;

#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone)]
pub(crate) struct Rect {
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
