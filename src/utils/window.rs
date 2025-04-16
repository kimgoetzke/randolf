use crate::utils::window_handle::WindowHandle;
use crate::utils::{Point, Rect};
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
  use crate::utils::window::CHAR_LIMIT;
  use crate::utils::window_handle::WindowHandle;
  use crate::utils::{Point, Rect, Window};

  impl Window {
    pub fn from(window_handle: isize, title: String, rect: Rect) -> Self {
      Window {
        handle: WindowHandle::new(window_handle),
        title,
        center: Point::from_center_of_rect(&rect),
        rect,
      }
    }
  }

  #[test]
  fn title_trunc_returns_full_title_when_within_limit() {
    let window = Window::from(1, "Short Title".to_string(), Rect::default());

    assert_eq!(window.title_trunc(), "Short Title");
  }

  #[test]
  fn title_trunc_truncates_long_title_correctly() {
    let long_title = "This is a very long window title that exceeds the character limit".to_string();
    let window = Window::from(1, long_title, Rect::default());

    assert_eq!(window.title_trunc(), "This is a very...character limit");
  }

  #[test]
  fn title_trunc_handles_exactly_double_char_limit() {
    let exact_title = "A".repeat(CHAR_LIMIT * 2);
    let window = Window::from(1, exact_title.clone(), Rect::default());

    assert_eq!(window.title_trunc(), exact_title);
  }

  #[test]
  fn title_trunc_handles_empty_title() {
    let window = Window::from(1, "".to_string(), Rect::default());

    assert_eq!(window.title_trunc(), "");
  }
}
