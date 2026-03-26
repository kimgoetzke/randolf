use crate::common::{Direction, Rect};

/// Represents the size and position of a window, as does [`Rect`], but expresses it in terms of its top-left corner,
/// and width and height. (Could be merged with [`Rect`] but I have kept it separate for now because [`Sizing`] is
/// easier to understand and use in Randolf while [`Rect`] is more in line with the Windows API.)
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

  /// Returns a new [`Sizing`] that is half the size of the current one in the dimension corresponding to the given
  /// direction, keeping the edge on the arrow-key side fixed and contracting the opposite edge inward. A gap of
  /// `margin / 2` is subtracted from each side of the split point, resulting in a total gap of `margin` between the
  /// two halves (consistent with the half-screen margin system).
  pub fn halved(&self, direction: Direction, margin: i32) -> Self {
    let half_margin = margin / 2;
    match direction {
      Direction::Left => Self {
        x: self.x,
        y: self.y,
        width: self.width / 2 - half_margin,
        height: self.height,
      },
      Direction::Right => Self {
        x: self.x + self.width / 2 + half_margin,
        y: self.y,
        width: self.width / 2 - half_margin,
        height: self.height,
      },
      Direction::Up => Self {
        x: self.x,
        y: self.y,
        width: self.width,
        height: self.height / 2 - half_margin,
      },
      Direction::Down => Self {
        x: self.x,
        y: self.y + self.height / 2 + half_margin,
        width: self.width,
        height: self.height / 2 - half_margin,
      },
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
  use crate::common::{Direction, Rect};

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

  #[test]
  fn halved_left_keeps_left_edge_and_halves_width() {
    let sizing = Sizing::new(10, 10, 80, 180);
    let result = sizing.halved(Direction::Left, 10);

    assert_eq!(result.x, 10);
    assert_eq!(result.y, 10);
    assert_eq!(result.width, 35);
    assert_eq!(result.height, 180);
  }

  #[test]
  fn halved_right_keeps_right_edge_and_halves_width() {
    let sizing = Sizing::new(10, 10, 80, 180);
    let result = sizing.halved(Direction::Right, 10);

    assert_eq!(result.x, 55);
    assert_eq!(result.y, 10);
    assert_eq!(result.width, 35);
    assert_eq!(result.height, 180);
  }

  #[test]
  fn halved_up_keeps_top_edge_and_halves_height() {
    let sizing = Sizing::new(10, 10, 80, 180);
    let result = sizing.halved(Direction::Up, 10);

    assert_eq!(result.x, 10);
    assert_eq!(result.y, 10);
    assert_eq!(result.width, 80);
    assert_eq!(result.height, 85);
  }

  #[test]
  fn halved_down_keeps_bottom_edge_and_halves_height() {
    let sizing = Sizing::new(10, 10, 80, 180);
    let result = sizing.halved(Direction::Down, 10);

    assert_eq!(result.x, 10);
    assert_eq!(result.y, 105);
    assert_eq!(result.width, 80);
    assert_eq!(result.height, 85);
  }

  #[test]
  fn halved_produces_correct_gap_between_halves() {
    let sizing = Sizing::new(10, 10, 80, 180);
    let left = sizing.halved(Direction::Left, 10);
    let right = sizing.halved(Direction::Right, 10);
    let up = sizing.halved(Direction::Up, 10);
    let down = sizing.halved(Direction::Down, 10);

    // Horizontal gap = right.x - (left.x + left.width) = margin
    assert_eq!(right.x - (left.x + left.width), 10);

    // Vertical gap = down.y - (up.y + up.height) = margin
    assert_eq!(down.y - (up.y + up.height), 10);
  }

  #[test]
  fn halved_near_maximised_left_equals_left_half_of_screen() {
    let work_area = Rect::new(0, 0, 100, 200);
    let near_max = Sizing::near_maximised(work_area, 10);
    let halved = near_max.halved(Direction::Left, 10);
    let left_half = Sizing::left_half_of_screen(work_area, 10);

    assert_eq!(halved, left_half);
  }

  #[test]
  fn halved_near_maximised_right_equals_right_half_of_screen() {
    let work_area = Rect::new(0, 0, 100, 200);
    let near_max = Sizing::near_maximised(work_area, 10);
    let halved = near_max.halved(Direction::Right, 10);
    let right_half = Sizing::right_half_of_screen(work_area, 10);

    assert_eq!(halved, right_half);
  }

  #[test]
  fn halved_near_maximised_up_equals_top_half_of_screen() {
    let work_area = Rect::new(0, 0, 100, 200);
    let near_max = Sizing::near_maximised(work_area, 10);
    let halved = near_max.halved(Direction::Up, 10);
    let top_half = Sizing::top_half_of_screen(work_area, 10);

    assert_eq!(halved, top_half);
  }

  #[test]
  fn halved_near_maximised_down_equals_bottom_half_of_screen() {
    let work_area = Rect::new(0, 0, 100, 200);
    let near_max = Sizing::near_maximised(work_area, 10);
    let halved = near_max.halved(Direction::Down, 10);
    let bottom_half = Sizing::bottom_half_of_screen(work_area, 10);

    assert_eq!(halved, bottom_half);
  }

  #[test]
  fn halved_left_half_left_produces_leftmost_quarter() {
    let work_area = Rect::new(0, 0, 100, 200);
    let left_half = Sizing::left_half_of_screen(work_area, 10);
    let result = left_half.halved(Direction::Left, 10);

    assert_eq!(result.x, 10);
    assert_eq!(result.width, 12);
    assert_eq!(result.height, 180);
  }

  #[test]
  fn halved_left_half_right_produces_second_column() {
    let work_area = Rect::new(0, 0, 100, 200);
    let left_half = Sizing::left_half_of_screen(work_area, 10);
    let result = left_half.halved(Direction::Right, 10);

    // Second column starts after the leftmost quarter + gap
    assert_eq!(result.x, 32);
    assert_eq!(result.width, 12);
    assert_eq!(result.height, 180);
  }

  #[test]
  fn halved_with_zero_margin_produces_exact_halves() {
    let sizing = Sizing::new(0, 0, 100, 200);

    let left = sizing.halved(Direction::Left, 0);
    assert_eq!(left, Sizing::new(0, 0, 50, 200));

    let right = sizing.halved(Direction::Right, 0);
    assert_eq!(right, Sizing::new(50, 0, 50, 200));

    let up = sizing.halved(Direction::Up, 0);
    assert_eq!(up, Sizing::new(0, 0, 100, 100));

    let down = sizing.halved(Direction::Down, 0);
    assert_eq!(down, Sizing::new(0, 100, 100, 100));
  }

  #[test]
  fn halved_with_zero_margin_has_no_gap() {
    let sizing = Sizing::new(0, 0, 100, 200);
    let left = sizing.halved(Direction::Left, 0);
    let right = sizing.halved(Direction::Right, 0);

    assert_eq!(right.x - (left.x + left.width), 0);
  }
}
