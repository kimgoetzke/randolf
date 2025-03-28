use crate::utils::Rect;

pub struct Sizing {
  pub x: i32,
  pub y: i32,
  pub width: i32,
  pub height: i32,
}

impl Sizing {
  pub fn right_half_of_screen(work_area: Rect, margin: i32) -> Self {
    Self {
      x: work_area.left + (work_area.right - work_area.left) / 2 + margin / 2,
      y: work_area.top + margin,
      width: (work_area.right - work_area.left) / 2 - margin - margin / 2,
      height: work_area.bottom - work_area.top - margin * 2,
    }
  }

  pub fn left_half_of_screen(work_area: Rect, margin: i32) -> Self {
    Self {
      x: work_area.left + margin,
      y: work_area.top + margin,
      width: (work_area.right - work_area.left) / 2 - margin - margin / 2,
      height: work_area.bottom - work_area.top - margin * 2,
    }
  }

  pub fn top_half_of_screen(work_area: Rect, margin: i32) -> Self {
    Self {
      x: work_area.left + margin,
      y: work_area.top + margin,
      width: work_area.right - work_area.left - margin * 2,
      height: (work_area.bottom - work_area.top) / 2 - margin - margin / 2,
    }
  }

  pub fn bottom_half_of_screen(work_area: Rect, margin: i32) -> Self {
    Self {
      x: work_area.left + margin,
      y: work_area.top + (work_area.bottom - work_area.top) / 2 + margin / 2,
      width: work_area.right - work_area.left - margin * 2,
      height: (work_area.bottom - work_area.top) / 2 - margin - margin / 2,
    }
  }
}
