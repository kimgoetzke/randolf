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
}
