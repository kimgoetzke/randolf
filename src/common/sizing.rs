use crate::common::Rect;

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Sizing {
  pub x: i32,
  pub y: i32,
  pub width: i32,
  pub height: i32,
}

impl Sizing {
  pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
    Sizing { x, y, width, height }
  }

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

  pub fn near_maximised(work_area: Rect, margin: i32) -> Self {
    Self {
      x: work_area.left + margin,
      y: work_area.top + margin,
      width: work_area.right - work_area.left - margin * 2,
      height: work_area.bottom - work_area.top - margin * 2,
    }
  }
}

impl From<Rect> for Sizing {
  fn from(rect: Rect) -> Self {
    Sizing {
      x: rect.left,
      y: rect.top,
      width: rect.width(),
      height: rect.height(),
    }
  }
}

impl From<Sizing> for Rect {
  fn from(sizing: Sizing) -> Self {
    Rect::new(sizing.x, sizing.y, sizing.x + sizing.width, sizing.y + sizing.height)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::common::Rect;

  #[test]
  fn right_half_of_screen_calculates_correct_sizing() {
    let work_area = Rect::new(0, 0, 100, 200);
    let sizing = Sizing::right_half_of_screen(work_area, 10);

    assert_eq!(sizing.x, 55);
    assert_eq!(sizing.y, 10);
    assert_eq!(sizing.width, 35);
    assert_eq!(sizing.height, 180);
  }

  #[test]
  fn left_half_of_screen_calculates_correct_sizing() {
    let work_area = Rect::new(0, 0, 100, 200);
    let sizing = Sizing::left_half_of_screen(work_area, 10);

    assert_eq!(sizing.x, 10);
    assert_eq!(sizing.y, 10);
    assert_eq!(sizing.width, 35);
    assert_eq!(sizing.height, 180);
  }

  #[test]
  fn top_half_of_screen_calculates_correct_sizing() {
    let work_area = Rect::new(0, 0, 100, 200);
    let sizing = Sizing::top_half_of_screen(work_area, 10);

    assert_eq!(sizing.x, 10);
    assert_eq!(sizing.y, 10);
    assert_eq!(sizing.width, 80);
    assert_eq!(sizing.height, 85);
  }

  #[test]
  fn bottom_half_of_screen_calculates_correct_sizing() {
    let work_area = Rect::new(0, 0, 100, 200);
    let sizing = Sizing::bottom_half_of_screen(work_area, 10);

    assert_eq!(sizing.x, 10);
    assert_eq!(sizing.y, 105);
    assert_eq!(sizing.width, 80);
    assert_eq!(sizing.height, 85);
  }

  #[test]
  fn near_maximised_calculates_correct_sizing() {
    let work_area = Rect::new(0, 0, 100, 200);
    let sizing = Sizing::near_maximised(work_area, 10);

    assert_eq!(sizing.x, 10);
    assert_eq!(sizing.y, 10);
    assert_eq!(sizing.width, 80);
    assert_eq!(sizing.height, 180);
  }
}
