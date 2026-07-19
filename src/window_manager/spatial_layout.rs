use super::navigation;
use crate::api::WindowsApi;
use crate::common::{Direction, Monitor, MonitorInfo, Placement, Point, Rect, Sizing, WindowHandle, WindowPlacement};
use crate::utils::MINIMUM_WINDOW_DIMENSION;

/// A layout that does not manage any windows. Handles geometry-based window movement, resizing, and follow-up focus.
#[derive(Debug, Default)]
pub(super) struct SpatialLayout;

impl SpatialLayout {
  /// Places the foreground window on half a monitor or moves it to the next monitor.
  pub(super) fn move_window<T: WindowsApi>(&self, api: &T, placement: &Placement, direction: Direction, margin: i32) {
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
        self.move_window_to_monitor(api, placement, handle, target_monitor, margin);
      } else {
        debug!("No monitor found in [{:?}] direction, did not move window", direction);
      }
      return;
    }

    let cursor_target = Point::from_center_of_sizing(&sizing);
    placement.resize(api, handle, sizing, margin);
    api.set_cursor_position(&cursor_target);
  }

  /// Moves and near-maximises a window on a target monitor.
  pub(super) fn move_window_to_monitor<T: WindowsApi>(
    &self,
    api: &T,
    placement: &Placement,
    handle: WindowHandle,
    target: &Monitor,
    margin: i32,
  ) {
    api.set_window_position(handle, target.work_area);
    placement.near_maximise(api, handle, MonitorInfo::from(target), margin);
    api.set_cursor_position(&target.center);
  }

  /// Steps the foreground window through the spatial sizes for a direction.
  pub(super) fn resize_window<T: WindowsApi>(&self, api: &T, placement: &Placement, direction: Direction, margin: i32) {
    let Some((handle, current_placement, monitor_info)) = window_and_monitor_info(api) else {
      return;
    };
    let work_area = monitor_info.work_area;
    let current_sizing = Sizing::from(current_placement.normal_position);

    // Calculate desired size
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

    // Calculate minimum permitted dimensions
    let (mut min_width, mut min_height) = calculate_minimum_resize_dimensions(work_area, margin);
    if let Some((application_min_width, application_min_height)) = api.get_minimum_window_dimensions(handle) {
      min_width = min_width.max(application_min_width);
      min_height = min_height.max(application_min_height);
    }
    if new_sizing.width < min_width || new_sizing.height < min_height {
      debug!(
        "Not resizing {} because resulting size ({}x{}) is below minimum ({}x{})",
        handle, new_sizing.width, new_sizing.height, min_width, min_height
      );
      return;
    }

    // Action resizing and revert if it does not succeed
    let cursor_target = Point::from_center_of_sizing(&new_sizing);
    placement.resize(api, handle, new_sizing.clone(), margin);
    let has_resize_succeeded = api
      .get_window_placement(handle)
      .is_some_and(|actual| placement.is_of_expected_size(api, handle, &actual, &new_sizing, margin));
    if !has_resize_succeeded {
      warn!(
        "Restoring {} because Windows did not apply the complete requested resize",
        handle
      );
      api.do_restore_window_placement(handle, current_placement);
      return;
    }
    api.set_cursor_position(&cursor_target);
  }

  /// Focuses the nearest remaining window after a close or minimise when enabled.
  pub(super) fn after_close_or_minimise<T: WindowsApi>(&self, api: &T, window: WindowHandle, move_cursor: bool) {
    if move_cursor {
      navigation::find_and_select_closest_window(api, window);
    }
  }
}

fn calculate_minimum_resize_dimensions(work_area: Rect, margin: i32) -> (i32, i32) {
  let quarter_width = Sizing::left_half_of_screen(work_area, margin)
    .halved(Direction::Left, margin)
    .width;
  let quarter_height = Sizing::top_half_of_screen(work_area, margin)
    .halved(Direction::Up, margin)
    .height;
  (
    MINIMUM_WINDOW_DIMENSION.max(quarter_width),
    MINIMUM_WINDOW_DIMENSION.max(quarter_height),
  )
}

fn window_and_monitor_info<T: WindowsApi>(api: &T) -> Option<(WindowHandle, WindowPlacement, MonitorInfo)> {
  let window = api.get_foreground_window()?;
  let placement = api.get_window_placement(window)?;
  let monitor_info = api.get_monitor_info_for_window(window)?;
  Some((window, placement, monitor_info))
}
