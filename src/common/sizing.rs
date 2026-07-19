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

  /// Returns a new [`Sizing`] that is 75% of the near-maximised size in the dimension corresponding to the given
  /// direction. The edge on the arrow-key side is anchored to the near-maximised edge; a gap of `margin / 2` is
  /// subtracted at the split edge only (matching [`halved`](Self::halved) exactly).
  pub fn three_quarter_near_maximised(work_area: Rect, direction: Direction, margin: i32) -> Self {
    let near_max = Self::near_maximised(work_area, margin);
    let half_margin = margin / 2;
    match direction {
      Direction::Left => Self {
        x: near_max.x,
        y: near_max.y,
        width: near_max.width * 3 / 4 - half_margin,
        height: near_max.height,
      },
      Direction::Right => Self {
        x: near_max.x + near_max.width / 4 + half_margin,
        y: near_max.y,
        width: near_max.width * 3 / 4 - half_margin,
        height: near_max.height,
      },
      Direction::Up => Self {
        x: near_max.x,
        y: near_max.y,
        width: near_max.width,
        height: near_max.height * 3 / 4 - half_margin,
      },
      Direction::Down => Self {
        x: near_max.x,
        y: near_max.y + near_max.height / 4 + half_margin,
        width: near_max.width,
        height: near_max.height * 3 / 4 - half_margin,
      },
    }
  }

  /// Returns a new [`Sizing`] occupying the centre half of the near-maximised area in the axis corresponding to
  /// `direction`. Left/Right produce a horizontally centred window (the intersection of [`three_quarter_near_maximised`]
  /// Left and Right); Up/Down produce a vertically centred window. A gap of `margin / 2` is maintained on each inner
  /// edge, consistent with the rest of the margin system.
  ///
  /// [`three_quarter_near_maximised`]: Self::three_quarter_near_maximised
  pub fn centre_near_maximised(work_area: Rect, direction: Direction, margin: i32) -> Self {
    let near_max = Self::near_maximised(work_area, margin);
    let half_margin = margin / 2;
    match direction {
      Direction::Left | Direction::Right => Self {
        x: near_max.x + near_max.width / 4 + half_margin,
        y: near_max.y,
        width: near_max.width / 2 - margin,
        height: near_max.height,
      },
      Direction::Up | Direction::Down => Self {
        x: near_max.x,
        y: near_max.y + near_max.height / 4 + half_margin,
        width: near_max.width,
        height: near_max.height / 2 - margin,
      },
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
