use crate::common::{Monitor, Point, Rect};
use std::cmp::{max, min};

/// The size of the canvas in characters.
const CANVAS_SIZE: usize = 100;

/// Prints the monitor layout to the console. Each monitor is represented by a digit. Useful to visualise where the
/// monitors are.
pub fn print_monitor_layout_to_canvas(monitors: &[Monitor]) {
  if monitors.is_empty() {
    debug!("No monitors to visualize");
    return;
  }

  // Canvas
  let canvas = calculate_canvas_bounds(monitors);
  let canvas_width = canvas.right - canvas.left;
  let canvas_height = canvas.bottom - canvas.top;
  let canvas_x = canvas_width as f32 / CANVAS_SIZE as f32;
  let scaled_height = (canvas_height as f32 / canvas_x).ceil() as usize;
  let scaled_height = min(scaled_height, CANVAS_SIZE);

  // Legend
  debug!("The total canvas size is {}x{}", canvas_width, canvas_height);
  debug!("");
  debug!("Legend:");
  for (i, monitor) in monitors.iter().enumerate() {
    debug!("{}. {}", i + 1, monitor);
  }
  debug!("");

  // Top border and coordinates
  print_left_and_right_coordinates(Point::new(canvas.left, canvas.top), Point::new(canvas.right, canvas.top));
  debug!("┌{}┐", "─".repeat(CANVAS_SIZE));

  // Grid
  let mut grid = vec![vec![' '; CANVAS_SIZE]; scaled_height];
  for (i, monitor) in monitors.iter().enumerate() {
    let digit = char::from_digit((i + 1) as u32, 10).unwrap_or('?');
    let left = ((monitor.monitor_area.left - canvas.left) as f32 / canvas_x).floor() as usize;
    let top = ((monitor.monitor_area.top - canvas.top) as f32 / canvas_x).floor() as usize;
    let right = ((monitor.monitor_area.right - canvas.left) as f32 / canvas_x).ceil() as usize;
    let bottom = ((monitor.monitor_area.bottom - canvas.top) as f32 / canvas_x).ceil() as usize;
    let right = min(right, CANVAS_SIZE - 1);
    let bottom = min(bottom, scaled_height - 1);
    for y in top..=bottom {
      for x in left..=right {
        if y < grid.len() && x < grid[0].len() {
          grid[y][x] = digit;
        }
      }
    }
  }

  for row in grid.iter() {
    let line: String = row.iter().collect();
    let mut final_line = String::from("│");
    final_line.push_str(&line);
    final_line.push('│');
    debug!("{}", final_line);
  }

  // Bottom border and coordinates
  debug!("└{}┘", "─".repeat(CANVAS_SIZE));
  print_left_and_right_coordinates(
    Point::new(canvas.left, canvas.bottom),
    Point::new(canvas.right, canvas.bottom),
  );
}

fn print_left_and_right_coordinates(left: Point, right: Point) {
  let left = format!("{}", left);
  let right = format!("{}", right);
  let padding = CANVAS_SIZE + 2 - left.len() - right.len();
  debug!("{}{}{}", left, " ".repeat(padding), right);
}

fn calculate_canvas_bounds(monitors: &[Monitor]) -> Rect {
  if monitors.is_empty() {
    return Rect {
      left: 0,
      top: 0,
      right: 0,
      bottom: 0,
    };
  }

  let mut canvas = Rect {
    left: monitors[0].monitor_area.left,
    top: monitors[0].monitor_area.top,
    right: monitors[0].monitor_area.right,
    bottom: monitors[0].monitor_area.bottom,
  };

  for monitor in monitors {
    canvas.left = min(canvas.left, monitor.monitor_area.left);
    canvas.top = min(canvas.top, monitor.monitor_area.top);
    canvas.right = max(canvas.right, monitor.monitor_area.right);
    canvas.bottom = max(canvas.bottom, monitor.monitor_area.bottom);
  }

  canvas
}
