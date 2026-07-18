use crate::api::WindowsApi;
use crate::common::{Direction, Monitor, Point, Window, WindowHandle};
use windows::Win32::UI::Shell::IVirtualDesktopManager;

pub(super) fn move_cursor<T: WindowsApi>(
  api: &T,
  direction: Direction,
  windows: &[&Window],
  virtual_desktop_manager: Option<&IVirtualDesktopManager>,
  allow_selecting_same_center_windows: bool,
) {
  let cursor_position = api.get_cursor_position();
  let (reference_point, reference_window) = match find_window_at_cursor(api, &cursor_position, windows) {
    Some(window) => (Point::from_center_of_rect(&window.rect), Some(window)),
    None => (cursor_position, None),
  };

  let target = virtual_desktop_manager.and_then(|vdm| {
    // Keep only current-desktop windows
    let current_desktop = windows
      .iter()
      .copied()
      .filter(|window| api.is_window_on_current_desktop(vdm, window))
      .collect::<Vec<_>>();
    select_window_in_direction(
      &reference_point,
      direction,
      &current_desktop,
      reference_window,
      allow_selecting_same_center_windows,
    )
  });

  if let Some(target_window) = target {
    let target_point = Point::from_center_of_rect(&target_window.rect);
    move_focus_to_window(api, direction, target_window, &target_point);
    return;
  }

  trace!("No window found in [{:?}] direction, attempting to find monitor", direction);
  let monitors = api.get_all_monitors();
  let current_monitor = api.get_monitor_handle_for_point(&cursor_position);
  match monitors.get(direction, current_monitor) {
    Some(target_monitor) => move_focus_to_monitor(api, direction, target_monitor),
    None => info!(
      "No window or monitor found in [{:?}] direction, did not move cursor",
      direction
    ),
  }
}

pub(super) fn find_and_select_closest_window<T: WindowsApi>(api: &T, ignored_window: WindowHandle) {
  let cursor_position = api.get_cursor_position();
  if let Some(window) = find_closest_window(api, cursor_position, Some(ignored_window)) {
    api.set_foreground_window(window);
    let window_info = api
      .get_window_placement(window)
      .expect("selected visible window has a placement");
    api.set_cursor_position(&Point::from_center_of_rect(&window_info.normal_position));
  } else {
    info!("No window found to move focus to after closing the current window");
  }
}

pub(super) fn find_closest_window<T: WindowsApi>(
  api: &T,
  cursor_position: Point,
  ignored_window: Option<WindowHandle>,
) -> Option<WindowHandle> {
  let mut closest_windows = Vec::new();
  let mut minimum_distance = f64::MAX;
  for window in api
    .get_all_visible_windows()
    .iter()
    .filter(|window| ignored_window != Some(window.handle))
  {
    let distance = cursor_position.distance_to(&window.center);
    trace!(
      "Distance from cursor position {} to window {} \"{}\" is {}",
      cursor_position,
      window.handle,
      window.title_trunc(),
      distance
    );
    if distance == minimum_distance {
      closest_windows.push(window.clone());
    } else if distance < minimum_distance {
      closest_windows.clear();
      closest_windows.push(window.clone());
      minimum_distance = distance;
    }
  }

  match closest_windows.as_slice() {
    [] => {
      trace!("No windows found close to {}", cursor_position);
      None
    }
    [window] => Some(window.handle),
    windows => {
      let smallest = windows
        .iter()
        .min_by_key(|window| window.rect.area())
        .expect("non-empty window slice has a minimum")
        .handle;
      trace!(
        "Found multiple windows closest to cursor position {}, returning {} which is the smallest one",
        cursor_position, smallest
      );
      Some(smallest)
    }
  }
}

/// Returns the window under the cursor, if any. If there are multiple windows under the cursor, the foreground window
/// is returned if it's in the list. Otherwise, the window with the closest centre point to the cursor is returned.
fn find_window_at_cursor<'window, T: WindowsApi>(
  api: &T,
  point: &Point,
  windows: &[&'window Window],
) -> Option<&'window Window> {
  let windows_under_cursor = windows
    .iter()
    .copied()
    .filter(|window| {
      point.x() >= window.rect.left
        && point.x() <= window.rect.right
        && point.y() >= window.rect.top
        && point.y() <= window.rect.bottom
    })
    .collect::<Vec<_>>();

  if windows_under_cursor.is_empty() {
    return None;
  }
  if let Some(foreground) = api.get_foreground_window()
    && let Some(window) = windows_under_cursor.iter().find(|window| window.handle == foreground)
  {
    debug!(
      "Cursor is currently over foreground window {} \"{}\" at {point}",
      window.handle,
      window.title_trunc()
    );
    return Some(window);
  }

  let closest = windows_under_cursor
    .into_iter()
    .min_by(|left, right| {
      let left_distance = f64::from(left.center.x().pow(2) + left.center.y().pow(2)).sqrt().trunc();
      let right_distance = f64::from(right.center.x().pow(2) + right.center.y().pow(2)).sqrt().trunc();
      left_distance.total_cmp(&right_distance)
    })
    .expect("non-empty windows-under-cursor list has a closest member");
  debug!(
    "Cursor is currently over window {} \"{}\" at {point}",
    closest.handle,
    closest.title_trunc()
  );
  Some(closest)
}

fn move_focus_to_window<T: WindowsApi>(api: &T, direction: Direction, target: &Window, target_point: &Point) {
  api.set_cursor_position(target_point);
  api.set_foreground_window(target.handle);
  info!(
    "Moved cursor in direction [{:?}] to {} \"{}\" at {target_point}",
    direction,
    target.handle,
    target.title_trunc()
  );
}

fn move_focus_to_monitor<T: WindowsApi>(api: &T, direction: Direction, monitor: &Monitor) {
  api.set_cursor_position(&monitor.center);
  info!(
    "Moved cursor in direction [{:?}] to {} on [{}]",
    direction, monitor.center, monitor
  );
}

pub(super) fn select_window_in_direction<'window>(
  reference_point: &Point,
  direction: Direction,
  windows: &[&'window Window],
  reference_window: Option<&Window>,
  allow_selecting_same_center_windows: bool,
) -> Option<&'window Window> {
  // Cycle same-centre windows first
  if allow_selecting_same_center_windows
    && let Some(reference_window) = reference_window
    && let Some(next_window) = find_next_same_center_window(reference_window, windows)
  {
    return Some(next_window);
  }

  let mut closest_window = None;
  let mut closest_score = f64::MAX;
  for &window in windows {
    let target_center_x = window.rect.left + (window.rect.right - window.rect.left) / 2;
    let target_center_y = window.rect.top + (window.rect.bottom - window.rect.top) / 2;
    let dx = i64::from(target_center_x) - i64::from(reference_point.x());
    let dy = i64::from(target_center_y) - i64::from(reference_point.y());
    let should_filter = !allow_selecting_same_center_windows
      || reference_window.is_some_and(|reference| reference.center != window.center || reference.handle == window.handle);
    if should_filter {
      // Skip windows outside the requested direction
      match direction {
        Direction::Left if dx >= 0 => continue,
        Direction::Right if dx <= 0 => continue,
        Direction::Up if dy >= 0 => continue,
        Direction::Down if dy <= 0 => continue,
        _ => {}
      }
    }

    // Score by distance and directional alignment
    let distance = ((dx.pow(2) + dy.pow(2)) as f64).sqrt().trunc();
    let angle = match direction {
      Direction::Left => (dy as f64).atan2((-dx) as f64).abs(),
      Direction::Right => (dy as f64).atan2(dx as f64).abs(),
      Direction::Up => (dx as f64).atan2((-dy) as f64).abs(),
      Direction::Down => (dx as f64).atan2(dy as f64).abs(),
    };
    let score = distance + angle;
    trace!(
      "Score for {} is [{}] (i.e. normalised_angle={}, distance={})",
      window.handle,
      score.trunc(),
      angle,
      distance
    );
    if score < closest_score {
      closest_score = score;
      closest_window = Some(window);
    }
  }
  closest_window
}

fn find_next_same_center_window<'window>(reference_window: &Window, windows: &[&'window Window]) -> Option<&'window Window> {
  let mut same_center = windows
    .iter()
    .copied()
    .filter(|window| window.center == reference_window.center)
    .collect::<Vec<_>>();
  if same_center.len() < 2 {
    return None;
  }

  // Keep cycling independent of Z-order
  same_center.sort_unstable_by_key(|window| window.handle.hwnd);
  let reference_index = same_center
    .iter()
    .position(|window| window.handle == reference_window.handle)?;
  same_center.get((reference_index + 1) % same_center.len()).copied()
}
