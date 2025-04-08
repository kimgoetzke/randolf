use crate::utils::{Point, Rect, Sizing};
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::WindowsAndMessaging::{SW_SHOWNORMAL, WINDOWPLACEMENT, WINDOWPLACEMENT_FLAGS};

#[derive(Debug, Clone)]
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
