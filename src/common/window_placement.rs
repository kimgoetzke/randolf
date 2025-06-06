use crate::common::{Point, Rect, Sizing};
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::WindowsAndMessaging::{SW_SHOWNORMAL, WINDOWPLACEMENT, WINDOWPLACEMENT_FLAGS};

/// A simple wrapper for the Windows [`WINDOWPLACEMENT`]. Its purpose is to abstract away from the Windows API and
/// to provide a handful of utility methods for working with window placements.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowPlacement {
  pub length: u32,
  pub flags: u32,
  pub show_cmd: u32,
  pub min_position: Point,
  pub max_position: Point,
  pub normal_position: Rect,
}

impl WindowPlacement {
  pub fn new_from_sizing(sizing: Sizing) -> Self {
    Self {
      length: size_of::<WINDOWPLACEMENT>() as u32,
      flags: 0,
      show_cmd: SW_SHOWNORMAL.0 as u32,
      min_position: Point::new(0, 0),
      max_position: Point::new(-1, -1),
      normal_position: Rect {
        left: sizing.x,
        top: sizing.y,
        right: sizing.x + sizing.width,
        bottom: sizing.y + sizing.height,
      },
    }
  }
}

impl From<WINDOWPLACEMENT> for WindowPlacement {
  fn from(value: WINDOWPLACEMENT) -> Self {
    Self {
      length: value.length,
      flags: value.flags.0,
      show_cmd: value.showCmd,
      min_position: Point::new(value.ptMinPosition.x, value.ptMinPosition.y),
      max_position: Point::new(value.ptMaxPosition.x, value.ptMaxPosition.y),
      normal_position: Rect::from(value.rcNormalPosition),
    }
  }
}

#[allow(clippy::from_over_into)]
impl Into<WINDOWPLACEMENT> for WindowPlacement {
  fn into(self) -> WINDOWPLACEMENT {
    WINDOWPLACEMENT {
      length: self.length,
      flags: WINDOWPLACEMENT_FLAGS(self.flags),
      showCmd: self.show_cmd,
      ptMinPosition: POINT {
        x: self.min_position.x(),
        y: self.min_position.y(),
      },
      ptMaxPosition: POINT {
        x: self.max_position.x(),
        y: self.max_position.y(),
      },
      rcNormalPosition: self.normal_position.into(),
    }
  }
}

#[allow(clippy::from_over_into)]
impl Into<*const WINDOWPLACEMENT> for WindowPlacement {
  fn into(self) -> *const WINDOWPLACEMENT {
    let wp = Box::new(self.into());
    Box::into_raw(wp)
  }
}

#[cfg(test)]
mod tests {
  use crate::common::{Point, Rect, Sizing, WindowPlacement};
  use windows::Win32::Foundation::POINT;
  use windows::Win32::UI::WindowsAndMessaging::{SW_SHOWNORMAL, WINDOWPLACEMENT, WINDOWPLACEMENT_FLAGS};

  impl WindowPlacement {
    pub fn new_test() -> Self {
      WindowPlacement {
        length: 44,
        flags: 1,
        show_cmd: 2,
        min_position: Point::new(5, 10),
        max_position: Point::new(-5, -10),
        normal_position: Rect::new(10, 20, 30, 40),
      }
    }

    pub fn new_from_rect(rect: Rect) -> Self {
      Self {
        length: size_of::<WINDOWPLACEMENT>() as u32,
        flags: 0,
        show_cmd: SW_SHOWNORMAL.0 as u32,
        min_position: Point::new(0, 0),
        max_position: Point::new(-1, -1),
        normal_position: rect,
      }
    }
  }

  #[test]
  fn new_from_sizing_creates_correct_window_placement() {
    let sizing = Sizing {
      x: 10,
      y: 20,
      width: 100,
      height: 200,
    };
    let placement = WindowPlacement::new_from_sizing(sizing);

    assert_eq!(placement.length, size_of::<WINDOWPLACEMENT>() as u32);
    assert_eq!(placement.flags, 0);
    assert_eq!(placement.show_cmd, SW_SHOWNORMAL.0 as u32);
    assert_eq!(placement.min_position, Point::new(0, 0));
    assert_eq!(placement.max_position, Point::new(-1, -1));
    assert_eq!(placement.normal_position, Rect::new(10, 20, 110, 220));
  }

  #[test]
  fn from_window_placement_converts_correctly() {
    let wp = WINDOWPLACEMENT {
      length: 44,
      flags: WINDOWPLACEMENT_FLAGS(1),
      showCmd: 2,
      ptMinPosition: POINT { x: 5, y: 10 },
      ptMaxPosition: POINT { x: -5, y: -10 },
      rcNormalPosition: Rect::new(10, 20, 30, 40).into(),
    };
    let placement: WindowPlacement = wp.into();

    assert_eq!(placement.length, 44);
    assert_eq!(placement.flags, 1);
    assert_eq!(placement.show_cmd, 2);
    assert_eq!(placement.min_position, Point::new(5, 10));
    assert_eq!(placement.max_position, Point::new(-5, -10));
    assert_eq!(placement.normal_position, Rect::new(10, 20, 30, 40));
  }

  #[test]
  fn into_window_placement_converts_correctly() {
    let placement = WindowPlacement::new_test();
    let wp: WINDOWPLACEMENT = placement.into();

    assert_eq!(wp.length, 44);
    assert_eq!(wp.flags.0, 1);
    assert_eq!(wp.showCmd, 2);
    assert_eq!(wp.ptMinPosition.x, 5);
    assert_eq!(wp.ptMinPosition.y, 10);
    assert_eq!(wp.ptMaxPosition.x, -5);
    assert_eq!(wp.ptMaxPosition.y, -10);
    assert_eq!(Rect::from(wp.rcNormalPosition), Rect::new(10, 20, 30, 40));
  }

  #[test]
  fn into_pointer_creates_valid_pointer() {
    let placement = WindowPlacement::new_test();
    let ptr: *const WINDOWPLACEMENT = placement.into();

    unsafe {
      assert!(!ptr.is_null());
      let deref = *ptr;
      assert_eq!(deref.length, 44);
      assert_eq!(deref.flags.0, 1);
      assert_eq!(deref.showCmd, 2);
      assert_eq!(deref.ptMinPosition.x, 5);
      assert_eq!(deref.ptMinPosition.y, 10);
      assert_eq!(deref.ptMaxPosition.x, -5);
      assert_eq!(deref.ptMaxPosition.y, -10);
      assert_eq!(Rect::from(deref.rcNormalPosition), Rect::new(10, 20, 30, 40));
    }
  }
}
