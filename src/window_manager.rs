use crate::native_api;
use crate::point::Point;
use crate::window::{Window, WindowId};
use std::collections::HashMap;
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::MONITORINFO;
use windows::Win32::UI::Shell::IVirtualDesktopManager;
use windows::Win32::UI::WindowsAndMessaging::{SW_SHOWNORMAL, WINDOWPLACEMENT, WINDOWPLACEMENT_FLAGS};

const TOLERANCE_IN_PX: i32 = 4;
const DEFAULT_MARGIN: i32 = 20;

pub(crate) struct WindowManager {
  known_windows: HashMap<String, WINDOWPLACEMENT>,
  virtual_desktop_manager: IVirtualDesktopManager,
}

struct Sizing {
  x: i32,
  y: i32,
  width: i32,
  height: i32,
}

#[derive(Debug, Clone, Copy)]
pub enum Direction {
  Left,
  Right,
  Up,
  Down,
}

// TODO: Add feature where pressing hotkey to move window to any side twice will move it to the next monitor in that
//  direction, if available
impl WindowManager {
  pub fn new() -> Self {
    Self {
      known_windows: HashMap::new(),
      virtual_desktop_manager: native_api::get_virtual_desktop_manager().expect("Failed to get the virtual desktop manager"),
    }
  }

  pub fn near_maximise_or_restore(&mut self) {
    info!("Hotkey pressed - action: near-maximise window");
    let (window, placement, monitor_info) = match get_window_and_monitor_info() {
      Some(value) => value,
      None => return,
    };

    match is_near_maximized(&placement, window, monitor_info) {
      true => restore_previous_placement(&self.known_windows, window),
      false => {
        add_or_update_previous_placement(&mut self.known_windows, window, placement);
        near_maximize_window(window, monitor_info, DEFAULT_MARGIN);
      }
    }
  }

  pub fn move_to_right_half_of_screen(&mut self) {
    info!("Hotkey pressed - action: move window to right half of screen");
    let (window, _, monitor_info) = match get_window_and_monitor_info() {
      Some(value) => value,
      None => return,
    };

    // Resize the window to the expected size
    let work_area = monitor_info.rcWork;
    let sizing = Sizing {
      x: work_area.left + (work_area.right - work_area.left) / 2 + DEFAULT_MARGIN / 2,
      y: work_area.top + DEFAULT_MARGIN,
      width: (work_area.right - work_area.left) / 2 - DEFAULT_MARGIN - DEFAULT_MARGIN / 2,
      height: work_area.bottom - work_area.top - DEFAULT_MARGIN * 2,
    };
    execute_window_resizing(window, sizing);
  }

  pub fn move_to_top_half_of_screen(&mut self) {
    info!("Hotkey pressed - action: move window to top half of screen");
    let (window, _, monitor_info) = match get_window_and_monitor_info() {
      Some(value) => value,
      None => return,
    };

    // Resize the window to the expected size
    let work_area = monitor_info.rcWork;
    let sizing = Sizing {
      x: work_area.left + DEFAULT_MARGIN,
      y: work_area.top + DEFAULT_MARGIN,
      width: work_area.right - work_area.left - DEFAULT_MARGIN * 2,
      height: (work_area.bottom - work_area.top) / 2 - DEFAULT_MARGIN - DEFAULT_MARGIN / 2,
    };
    execute_window_resizing(window, sizing);
  }

  pub fn move_to_bottom_half_of_screen(&mut self) {
    info!("Hotkey pressed - action: move window to bottom half of screen");
    let (window, _, monitor_info) = match get_window_and_monitor_info() {
      Some(value) => value,
      None => return,
    };

    // Resize the window to the expected size
    let work_area = monitor_info.rcWork;
    let sizing = Sizing {
      x: work_area.left + DEFAULT_MARGIN,
      y: work_area.top + (work_area.bottom - work_area.top) / 2 + DEFAULT_MARGIN / 2,
      width: work_area.right - work_area.left - DEFAULT_MARGIN * 2,
      height: (work_area.bottom - work_area.top) / 2 - DEFAULT_MARGIN - DEFAULT_MARGIN / 2,
    };
    execute_window_resizing(window, sizing);
  }

  pub fn move_to_left_half_of_screen(&mut self) {
    info!("Hotkey pressed - action: move window to left half of screen");
    let (window, _, monitor_info) = match get_window_and_monitor_info() {
      Some(value) => value,
      None => return,
    };

    // Resize the window to the expected size
    let work_area = monitor_info.rcWork;
    let sizing = Sizing {
      x: work_area.left + DEFAULT_MARGIN,
      y: work_area.top + DEFAULT_MARGIN,
      width: (work_area.right - work_area.left) / 2 - DEFAULT_MARGIN - DEFAULT_MARGIN / 2,
      height: work_area.bottom - work_area.top - DEFAULT_MARGIN * 2,
    };
    execute_window_resizing(window, sizing);
  }

  pub fn close(&mut self) {
    info!("Hotkey pressed - action: close window");
    let Some(window) = native_api::get_foreground_window() else {
      return;
    };

    native_api::close(window);
  }

  pub fn move_cursor_to_window_in_direction(&mut self, direction: Direction) {
    info!(
      "Hotkey pressed - action: move cursor to window in direction [{:?}]",
      direction
    );
    let windows = native_api::get_all_visible_windows();
    let cursor_position = native_api::get_cursor_position();
    let target_point = match find_window_at_cursor(&cursor_position, &windows) {
      Some(window_info) => Point::from_center_of_rect(&window_info.rect),
      None => cursor_position,
    };

    let Some(target_window) =
      find_closest_window_in_direction(&target_point, direction, &windows, &self.virtual_desktop_manager)
    else {
      info!("No window found in [{:?}] direction, did not move cursor", direction);
      return;
    };
    let target_point = Point::from_center_of_rect(&target_window.rect);
    native_api::set_cursor_position(&target_point);
    native_api::set_foreground_window(WindowId::from(target_window.clone()));
    info!(
      "Moved cursor in direction [{:?}] to #{:?} at {target_point}",
      direction, target_window.hwnd
    );
  }
}

fn get_window_and_monitor_info() -> Option<(HWND, WINDOWPLACEMENT, MONITORINFO)> {
  let window = native_api::get_foreground_window_as_hwnd()?;
  let placement = native_api::get_window_placement(window)?;
  let monitor_info = native_api::get_monitor_info(window)?;
  Some((window, placement, monitor_info))
}

fn restore_previous_placement(known_windows: &HashMap<String, WINDOWPLACEMENT>, window: HWND) {
  let window_id = format!("{:?}", window);
  if let Some(previous_placement) = known_windows.get(&window_id) {
    info!("Restoring previous placement for #{}", window_id);
    native_api::restore_window_placement(window, previous_placement);
  } else {
    warn!("No previous placement found for #{}", window_id);
  }
}

fn add_or_update_previous_placement(
  known_windows: &mut HashMap<String, WINDOWPLACEMENT>,
  window: HWND,
  placement: WINDOWPLACEMENT,
) {
  let window_id = format!("{:?}", window);
  if known_windows.contains_key(&window_id) {
    known_windows.remove(&window_id);
    trace!(
      "Removing previous placement for window #{} so that a new value can be added",
      window_id
    );
  }

  known_windows.insert(window_id.clone(), placement);
  trace!("Adding/updating previous placement for window #{}", window_id);
}

fn near_maximize_window(window: HWND, monitor_info: MONITORINFO, margin: i32) {
  info!("Near-maximizing #{:?}", window.0);

  // Maximize first to get the animation effect
  native_api::maximise_window(window);

  // Resize the window to the expected size
  let work_area = monitor_info.rcWork;
  let sizing = Sizing {
    x: work_area.left + margin,
    y: work_area.top + margin,
    width: work_area.right - work_area.left - margin * 2,
    height: work_area.bottom - work_area.top - margin * 2,
  };
  execute_window_resizing(window, sizing);
}

fn is_near_maximized(placement: &WINDOWPLACEMENT, window: HWND, monitor_info: MONITORINFO) -> bool {
  let work_area = monitor_info.rcWork;
  let expected_x = work_area.left + 30;
  let expected_y = work_area.top + 30;
  let expected_width = work_area.right - work_area.left - 30 * 2;
  let expected_height = work_area.bottom - work_area.top - 30 * 2;
  let rc = placement.rcNormalPosition;
  let result = (rc.left - expected_x).abs() <= TOLERANCE_IN_PX
    && (rc.top - expected_y).abs() <= TOLERANCE_IN_PX
    && (rc.right - rc.left - expected_width).abs() <= TOLERANCE_IN_PX
    && (rc.bottom - rc.top - expected_height).abs() <= TOLERANCE_IN_PX;

  trace!(
    "Expected size of #{:?}: ({},{})x({},{})",
    window, expected_x, expected_y, expected_width, expected_height
  );
  trace!(
    "Actual size of #{:?}: ({},{})x({},{})",
    window,
    rc.left,
    rc.top,
    rc.right - rc.left,
    rc.bottom - rc.top
  );
  debug!(
    "#{:?} {} near-maximized (tolerance: {})",
    window,
    if result { "is currently" } else { "is currently NOT" },
    TOLERANCE_IN_PX
  );

  result
}

fn execute_window_resizing(window: HWND, sizing: Sizing) {
  let placement = WINDOWPLACEMENT {
    length: size_of::<WINDOWPLACEMENT>() as u32,
    flags: WINDOWPLACEMENT_FLAGS(0),
    showCmd: SW_SHOWNORMAL.0 as u32,
    ptMaxPosition: POINT { x: 0, y: 0 },
    ptMinPosition: POINT { x: -1, y: -1 },
    rcNormalPosition: RECT {
      left: sizing.x,
      top: sizing.y,
      right: sizing.x + sizing.width,
      bottom: sizing.y + sizing.height,
    },
  };
  native_api::update_window_placement_and_force_repaint(window, &placement);
}

/// Returns the window under the cursor, if any. If there are multiple windows under the cursor, the foreground window
/// is returned if it's in the list. Otherwise, the closest window is returned.
fn find_window_at_cursor<'a>(point: &Point, windows: &'a [Window]) -> Option<&'a Window> {
  let windows_under_cursor = windows
    .iter()
    .filter(|window_info| {
      point.x() >= window_info.rect.left
        && point.x() <= window_info.rect.right
        && point.y() >= window_info.rect.top
        && point.y() <= window_info.rect.bottom
    })
    .collect::<Vec<&Window>>();

  if !windows_under_cursor.is_empty() {
    if let Some(foreground_window) = native_api::get_foreground_window() {
      if let Some(window_info) = windows_under_cursor
        .iter()
        .find(|window_info| window_info.window == foreground_window)
      {
        debug!(
          "Cursor is currently over foreground window #{:?} \"{}\" at {point}",
          window_info.hwnd, window_info.title
        );
        return Some(window_info);
      }
    }

    let mut closest_window = None;
    let mut min_distance = f64::MAX;
    for window_info in windows_under_cursor {
      let distance = ((window_info.center.x().pow(2) + window_info.center.y().pow(2)) as f64)
        .sqrt()
        .trunc();

      if distance < min_distance {
        min_distance = distance;
        closest_window = Some(window_info);
      }
    }
    let closest_window = closest_window.expect("Failed to get the closest window");
    debug!(
      "Cursor is currently over window #{:?} \"{}\" at {point} with a distance of {}",
      closest_window.hwnd,
      closest_window.title,
      min_distance.trunc()
    );
    return Some(closest_window);
  }

  None
}

fn find_closest_window_in_direction<'a>(
  reference_point: &Point,
  direction: Direction,
  windows: &'a Vec<Window>,
  virtual_desktop_manager: &IVirtualDesktopManager,
) -> Option<&'a Window> {
  let mut closest_window = None;
  let mut closest_score = f64::MAX;

  for window_info in windows {
    // Skip windows that are not on the current desktop
    if !native_api::is_window_on_current_desktop(virtual_desktop_manager, window_info)? {
      continue;
    }

    let target_center_x = window_info.rect.left + (window_info.rect.right - window_info.rect.left) / 2;
    let target_center_y = window_info.rect.top + (window_info.rect.bottom - window_info.rect.top) / 2;
    let dx = target_center_x - reference_point.x();
    let dy = target_center_y - reference_point.y();

    // Skip windows that are not in the right direction
    match direction {
      Direction::Left if dx >= 0 => continue,
      Direction::Right if dx <= 0 => continue,
      Direction::Up if dy >= 0 => continue,
      Direction::Down if dy <= 0 => continue,
      _ => {}
    }

    let distance = ((dx.pow(2) + dy.pow(2)) as f64).sqrt().trunc();

    // Calculate angle between the vector and the direction vector
    let angle = match direction {
      Direction::Left => (dy as f64).atan2((-dx) as f64).abs(),
      Direction::Right => (dy as f64).atan2(dx as f64).abs(),
      Direction::Up => (dx as f64).atan2((-dy) as f64).abs(),
      Direction::Down => (dx as f64).atan2(dy as f64).abs(),
    };

    // Calculate a score based on the distance and angle and select the closest window
    let score = distance + angle;
    debug!(
      "Score for #{:?} is [{}] (i.e. normalised_angle={}, distance={})",
      window_info.hwnd,
      score.trunc(),
      angle,
      distance,
    );
    if score < closest_score {
      closest_score = score;
      closest_window = Some(window_info);
    }
  }

  closest_window
}
