use crate::common::{Point, Rect, WindowHandle};
use windows::Win32::Foundation::HWND;

const CHAR_LIMIT: usize = 15;

#[derive(Debug, Clone)]
pub struct Window {
  pub handle: WindowHandle,
  pub title: String,
  pub rect: Rect,
  pub center: Point,
}

impl Window {
  pub fn new(hwnd: HWND, title: String, rect: Rect) -> Self {
    Self {
      title,
      center: Point::from_center_of_rect(&rect),
      rect,
      handle: WindowHandle::from(hwnd),
    }
  }

  pub fn title_trunc(&self) -> String {
    let char_count = self.title.chars().count();
    if char_count <= CHAR_LIMIT + CHAR_LIMIT {
      self.title.to_string()
    } else {
      let prefix: String = self.title.chars().take(CHAR_LIMIT).collect::<String>().trim_end().to_string();
      let suffix: String = self
        .title
        .chars()
        .skip(char_count - CHAR_LIMIT)
        .collect::<String>()
        .trim_start()
        .to_string();
      format!("{}...{}", prefix, suffix)
    }
  }
}

impl PartialEq for Window {
  fn eq(&self, other: &Self) -> bool {
    self.handle == other.handle && self.title == other.title && self.rect == other.rect
  }
}

#[cfg(test)]
mod tests {
  use crate::common::window::CHAR_LIMIT;
  use crate::common::{Point, Rect, Window, WindowHandle};

  impl Window {
    pub fn new_test(isize: isize, rect: Rect) -> Self {
      Window {
        handle: WindowHandle::new(isize),
        title: format!("Test Window {}", isize),
        center: Point::from_center_of_rect(&rect),
        rect,
      }
    }

    pub fn new_test_with_title(isize: isize, title: String, rect: Rect) -> Self {
      Window {
        handle: WindowHandle::new(isize),
        title,
        center: Point::from_center_of_rect(&rect),
        rect,
      }
    }
  }

  #[test]
  fn title_trunc_returns_full_title_when_within_limit() {
    let window = Window::new_test(2, Rect::default());

    assert_eq!(window.title_trunc(), "Test Window 2");
  }

  #[test]
  fn title_trunc_truncates_long_title_correctly() {
    let long_title = "This is a very long window title that exceeds the character limit".to_string();
    let window = Window::new_test_with_title(1, long_title, Rect::default());

    assert_eq!(window.title_trunc(), "This is a very...character limit");
  }

  #[test]
  fn title_trunc_handles_exactly_double_char_limit() {
    let exact_title = "A".repeat(CHAR_LIMIT * 2);
    let window = Window::new_test_with_title(1, exact_title.clone(), Rect::default());

    assert_eq!(window.title_trunc(), exact_title);
  }

  #[test]
  fn title_trunc_handles_empty_title() {
    let window = Window::new_test_with_title(1, "".into(), Rect::default());

    assert_eq!(window.title_trunc(), "");
  }
}
