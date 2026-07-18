use super::navigation;
use super::placement::Placement;
use crate::api::WindowsApi;
use crate::common::{Direction, MonitorInfo, Point, Sizing, WindowHandle, WindowPlacement};
use crate::utils::{MINIMUM_WINDOW_DIMENSION, MINIMUM_WINDOW_DIMENSION_DIVISOR};

pub(super) fn move_window<T: WindowsApi>(api: &T, placement: &Placement, direction: Direction, margin: i32) {
  let Some((handle, current_placement, monitor_info)) = window_and_monitor_info(api) else {
    return;
  };
  let sizing = match direction {
    Direction::Left => Sizing::left_half_of_screen(monitor_info.work_area, margin),
    Direction::Right => Sizing::right_half_of_screen(monitor_info.work_area, margin),
    Direction::Up => Sizing::top_half_of_screen(monitor_info.work_area, margin),
    Direction::Down => Sizing::bottom_half_of_screen(monitor_info.work_area, margin),
  };

  if placement.is_of_expected_size(api, handle, &current_placement, &sizing, margin) {
    let monitors = api.get_all_monitors();
    let current_monitor = api.get_monitor_handle_for_window_handle(handle);
    if let Some(target_monitor) = monitors.get(direction, current_monitor) {
      debug!("Moving window to [{}]", target_monitor);
      api.set_window_position(handle, target_monitor.work_area);
      placement.near_maximise(api, handle, MonitorInfo::from(target_monitor), margin);
      api.set_cursor_position(&target_monitor.center);
    } else {
      debug!("No monitor found in [{:?}] direction, did not move window", direction);
    }
    return;
  }

  let cursor_target = Point::from_center_of_sizing(&sizing);
  placement.resize(api, handle, sizing, margin);
  api.set_cursor_position(&cursor_target);
}

pub(super) fn resize_window<T: WindowsApi>(api: &T, placement: &Placement, direction: Direction, margin: i32) {
  let Some((handle, current_placement, monitor_info)) = window_and_monitor_info(api) else {
    return;
  };
  let work_area = monitor_info.work_area;
  let current_sizing = Sizing::from(current_placement.normal_position);
  let new_sizing = if placement.is_near_maximised(api, &current_placement, &handle, &monitor_info, margin) {
    Sizing::three_quarter_near_maximised(work_area, direction, margin)
  } else if placement.is_three_quarter_near_maximised(api, &handle, &monitor_info, direction, margin) {
    Sizing::near_maximised(work_area, margin).halved(direction, margin)
  } else if placement.is_three_quarter_near_maximised(api, &handle, &monitor_info, direction.opposite(), margin) {
    Sizing::centre_near_maximised(work_area, direction, margin)
  } else {
    current_sizing.halved(direction, margin)
  };
  debug!(
    "Expected size of {}: ({},{})x({},{})",
    handle, new_sizing.x, new_sizing.y, new_sizing.width, new_sizing.height
  );
  let min_width = MINIMUM_WINDOW_DIMENSION.max((work_area.right - work_area.left) / MINIMUM_WINDOW_DIMENSION_DIVISOR);
  let min_height = MINIMUM_WINDOW_DIMENSION.max((work_area.bottom - work_area.top) / MINIMUM_WINDOW_DIMENSION_DIVISOR);
  if new_sizing.width < min_width || new_sizing.height < min_height {
    let is_below_constant = min_width <= MINIMUM_WINDOW_DIMENSION || min_height <= MINIMUM_WINDOW_DIMENSION;
    debug!(
      "Not resizing {} because resulting size ({}x{}) is below minimum ({}x{}) (hit {} threshold)",
      handle,
      new_sizing.width,
      new_sizing.height,
      min_width,
      min_height,
      if is_below_constant { "constant" } else { "dynamic" }
    );
    return;
  }
  let cursor_target = Point::from_center_of_sizing(&new_sizing);
  placement.resize(api, handle, new_sizing, margin);
  api.set_cursor_position(&cursor_target);
}

pub(super) fn after_close_or_minimise<T: WindowsApi>(api: &T, window: WindowHandle, move_cursor: bool) {
  if move_cursor {
    navigation::find_and_select_closest_window(api, window);
  }
}

fn window_and_monitor_info<T: WindowsApi>(api: &T) -> Option<(WindowHandle, WindowPlacement, MonitorInfo)> {
  let window = api.get_foreground_window()?;
  let placement = api.get_window_placement(window)?;
  let monitor_info = api.get_monitor_info_for_window(window)?;
  Some((window, placement, monitor_info))
}
