use crate::common::{Direction, Rect, Sizing};

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
fn three_quarter_near_maximised_left_keeps_left_edge_and_returns_three_quarter_width() {
  let work_area = Rect::new(0, 0, 100, 200);
  let result = Sizing::three_quarter_near_maximised(work_area, Direction::Left, 10);

  assert_eq!(result.x, 10);
  assert_eq!(result.y, 10);
  assert_eq!(result.width, 55);
  assert_eq!(result.height, 180);
}

#[test]
fn three_quarter_near_maximised_right_keeps_right_edge_and_returns_three_quarter_width() {
  let work_area = Rect::new(0, 0, 100, 200);
  let result = Sizing::three_quarter_near_maximised(work_area, Direction::Right, 10);

  assert_eq!(result.x, 35);
  assert_eq!(result.y, 10);
  assert_eq!(result.width, 55);
  assert_eq!(result.height, 180);
}

#[test]
fn three_quarter_near_maximised_up_keeps_top_edge_and_returns_three_quarter_height() {
  let work_area = Rect::new(0, 0, 100, 200);
  let result = Sizing::three_quarter_near_maximised(work_area, Direction::Up, 10);

  assert_eq!(result.x, 10);
  assert_eq!(result.y, 10);
  assert_eq!(result.width, 80);
  assert_eq!(result.height, 130);
}

#[test]
fn three_quarter_near_maximised_down_keeps_bottom_edge_and_returns_three_quarter_height() {
  let work_area = Rect::new(0, 0, 100, 200);
  let result = Sizing::three_quarter_near_maximised(work_area, Direction::Down, 10);

  assert_eq!(result.x, 10);
  assert_eq!(result.y, 60);
  assert_eq!(result.width, 80);
  assert_eq!(result.height, 130);
}

#[test]
fn three_quarter_near_maximised_with_zero_margin_produces_exact_three_quarters() {
  let work_area = Rect::new(0, 0, 100, 200);

  let left = Sizing::three_quarter_near_maximised(work_area, Direction::Left, 0);
  assert_eq!(left, Sizing::new(0, 0, 75, 200));

  let right = Sizing::three_quarter_near_maximised(work_area, Direction::Right, 0);
  assert_eq!(right, Sizing::new(25, 0, 75, 200));

  let up = Sizing::three_quarter_near_maximised(work_area, Direction::Up, 0);
  assert_eq!(up, Sizing::new(0, 0, 100, 150));

  let down = Sizing::three_quarter_near_maximised(work_area, Direction::Down, 0);
  assert_eq!(down, Sizing::new(0, 50, 100, 150));
}

#[test]
fn three_quarter_near_maximised_deducts_half_margin_at_split_edge() {
  let work_area = Rect::new(0, 0, 100, 200);
  let margin = 10;
  let half_margin = margin / 2;
  let near_max = Sizing::near_maximised(work_area, margin);

  let left = Sizing::three_quarter_near_maximised(work_area, Direction::Left, margin);
  let right = Sizing::three_quarter_near_maximised(work_area, Direction::Right, margin);
  let up = Sizing::three_quarter_near_maximised(work_area, Direction::Up, margin);
  let down = Sizing::three_quarter_near_maximised(work_area, Direction::Down, margin);

  // Left: split (right) edge is half_margin before the 3/4 split point
  assert_eq!(near_max.x + near_max.width * 3 / 4 - (left.x + left.width), half_margin);

  // Right: split (left) edge is half_margin after the 1/4 split point
  assert_eq!(right.x - (near_max.x + near_max.width / 4), half_margin);

  // Up: split (bottom) edge is half_margin before the 3/4 split point
  assert_eq!(near_max.y + near_max.height * 3 / 4 - (up.y + up.height), half_margin);

  // Down: split (top) edge is half_margin after the 1/4 split point
  assert_eq!(down.y - (near_max.y + near_max.height / 4), half_margin);
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

#[test]
fn centre_near_maximised_left_and_right_produce_identical_horizontal_centre() {
  let work_area = Rect::new(0, 0, 100, 200);
  let left = Sizing::centre_near_maximised(work_area, Direction::Left, 10);
  let right = Sizing::centre_near_maximised(work_area, Direction::Right, 10);

  assert_eq!(left, right);
}

#[test]
fn centre_near_maximised_up_and_down_produce_identical_vertical_centre() {
  let work_area = Rect::new(0, 0, 100, 200);
  let up = Sizing::centre_near_maximised(work_area, Direction::Up, 10);
  let down = Sizing::centre_near_maximised(work_area, Direction::Down, 10);

  assert_eq!(up, down);
}

#[test]
fn centre_near_maximised_horizontal_is_intersection_of_three_quarter_left_and_right() {
  let work_area = Rect::new(0, 0, 100, 200);
  let margin = 10;
  let tq_left = Sizing::three_quarter_near_maximised(work_area, Direction::Left, margin);
  let tq_right = Sizing::three_quarter_near_maximised(work_area, Direction::Right, margin);
  let centre = Sizing::centre_near_maximised(work_area, Direction::Left, margin);

  // Centre left edge equals three_quarter_right left edge
  assert_eq!(centre.x, tq_right.x);
  // Centre right edge equals three_quarter_left right edge
  assert_eq!(centre.x + centre.width, tq_left.x + tq_left.width);
}

#[test]
fn centre_near_maximised_vertical_is_intersection_of_three_quarter_up_and_down() {
  let work_area = Rect::new(0, 0, 100, 200);
  let margin = 10;
  let tq_up = Sizing::three_quarter_near_maximised(work_area, Direction::Up, margin);
  let tq_down = Sizing::three_quarter_near_maximised(work_area, Direction::Down, margin);
  let centre = Sizing::centre_near_maximised(work_area, Direction::Up, margin);

  // Centre top edge equals three_quarter_down top edge
  assert_eq!(centre.y, tq_down.y);
  // Centre bottom edge equals three_quarter_up bottom edge
  assert_eq!(centre.y + centre.height, tq_up.y + tq_up.height);
}

#[test]
fn centre_near_maximised_with_zero_margin_occupies_exact_middle_half() {
  let work_area = Rect::new(0, 0, 100, 200);
  let h = Sizing::centre_near_maximised(work_area, Direction::Left, 0);
  assert_eq!(h, Sizing::new(25, 0, 50, 200));

  let v = Sizing::centre_near_maximised(work_area, Direction::Up, 0);
  assert_eq!(v, Sizing::new(0, 50, 100, 100));
}
