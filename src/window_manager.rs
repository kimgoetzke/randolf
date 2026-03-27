use crate::api::WindowsApi;
use crate::common::*;
use crate::configuration_provider::{
  ADDITIONAL_WORKSPACE_COUNT, ALLOW_MOVING_CURSOR_AFTER_OPEN_CLOSE_OR_MINIMISE, ALLOW_SELECTING_SAME_CENTER_WINDOWS,
  ConfigurationProvider, WINDOW_MARGIN,
};
use crate::utils::{
  CONFIGURATION_PROVIDER_LOCK, MINIMUM_WINDOW_DIMENSION, MINIMUM_WINDOW_DIMENSION_DIVISOR, MINIMUM_WINDOW_MARGIN,
};
use crate::workspace_manager::WorkspaceManager;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use windows::Win32::UI::Shell::IVirtualDesktopManager;
use windows::Win32::UI::WindowsAndMessaging::SW_MAXIMIZE;

const REGULAR_TOLERANCE_IN_PX: i32 = 2;
const DWM_TOLERANCE_IN_PX: i32 = 8;

pub struct WindowManager<T: WindowsApi> {
  configuration_provider: Arc<Mutex<ConfigurationProvider>>,
  known_windows: HashMap<String, WindowPlacement>,
  allow_moving_cursor_after_close_or_minimise: bool,
  workspace_manager: WorkspaceManager<T>,
  virtual_desktop_manager: Option<IVirtualDesktopManager>,
  windows_api: T,
}

impl<T: WindowsApi + Clone> WindowManager<T> {
  pub fn new(configuration_provider: Arc<Mutex<ConfigurationProvider>>, api: T) -> Self {
    let guard = match configuration_provider.try_lock() {
      Ok(guard) => guard,
      Err(err) => {
        panic!(
          "{} when trying to create window manager: {}",
          CONFIGURATION_PROVIDER_LOCK, err
        );
      }
    };
    let additional_workspace_count = guard.get_i32(ADDITIONAL_WORKSPACE_COUNT);
    let window_margin = guard.get_i32(WINDOW_MARGIN);
    let allow_moving_cursor_after_close_or_minimise = guard.get_bool(ALLOW_MOVING_CURSOR_AFTER_OPEN_CLOSE_OR_MINIMISE);
    drop(guard);
    let workspace_manager = WorkspaceManager::new(additional_workspace_count, window_margin, api.clone());

    Self {
      known_windows: HashMap::new(),
      allow_moving_cursor_after_close_or_minimise,
      virtual_desktop_manager: Some(
        api
          .get_virtual_desktop_manager()
          .expect("Failed to get the virtual desktop manager"),
      ),
      workspace_manager,
      configuration_provider,
      windows_api: api,
    }
  }

  /// Returns the unique IDs for all desktop containers across all monitors in their natural order.
  pub fn get_ordered_permanent_workspace_ids(&mut self) -> Vec<PersistentWorkspaceId> {
    self.workspace_manager.get_ordered_permanent_workspace_ids()
  }

  pub fn close_window(&mut self) {
    let Some(window) = self.windows_api.get_foreground_window() else {
      return;
    };

    self.windows_api.do_close_window(window);
    if self.allow_moving_cursor_after_close_or_minimise {
      self.find_and_select_closest_window(window);
    }
  }

  pub fn switch_workspace(&mut self, id: PersistentWorkspaceId) {
    self.workspace_manager.switch_workspace(id);
  }

  pub fn move_window_to_workspace(&mut self, id: PersistentWorkspaceId) {
    self.workspace_manager.move_window_to_workspace(id);
  }

  pub fn move_window(&mut self, direction: Direction) {
    let (handle, placement, monitor_info) = match self.get_window_and_monitor_info() {
      Some(value) => value,
      None => return,
    };
    let sizing = match direction {
      Direction::Left => Sizing::left_half_of_screen(monitor_info.work_area, self.margin()),
      Direction::Right => Sizing::right_half_of_screen(monitor_info.work_area, self.margin()),
      Direction::Up => Sizing::top_half_of_screen(monitor_info.work_area, self.margin()),
      Direction::Down => Sizing::bottom_half_of_screen(monitor_info.work_area, self.margin()),
    };

    match self.is_of_expected_size(handle, &placement, &sizing) {
      true => {
        let all_monitors = self.windows_api.get_all_monitors();
        let this_monitor = self.windows_api.get_monitor_handle_for_window_handle(handle);
        let target_monitor = all_monitors.get(direction, this_monitor);
        if let Some(target_monitor) = target_monitor {
          debug!("Moving window to [{}]", target_monitor);
          self.windows_api.set_window_position(handle, target_monitor.work_area);
          self.near_maximise_window(handle, MonitorInfo::from(target_monitor), self.margin());
          self.windows_api.set_cursor_position(&target_monitor.center);
        } else {
          debug!("No monitor found in [{:?}] direction, did not move window", direction);
        }
      }
      false => {
        let cursor_target_point = Point::from_center_of_sizing(&sizing);
        self.execute_window_resizing(handle, sizing);
        self.windows_api.set_cursor_position(&cursor_target_point);
      }
    }
  }

  pub fn resize_window(&mut self, direction: Direction) {
    let (handle, placement, monitor_info) = match self.get_window_and_monitor_info() {
      Some(value) => value,
      None => return,
    };
    let work_area = monitor_info.work_area;
    let current_sizing = Sizing::from(placement.normal_position);
    let new_sizing = if self.is_near_maximised(&placement, &handle, &monitor_info) {
      Sizing::three_quarter_near_maximised(work_area, direction, self.margin())
    } else if self.is_three_quarter_near_maximised(&handle, &monitor_info, direction) {
      Sizing::near_maximised(work_area, self.margin()).halved(direction, self.margin())
    } else {
      current_sizing.halved(direction, self.margin())
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
    let cursor_target_point = Point::from_center_of_sizing(&new_sizing);
    self.execute_window_resizing(handle, new_sizing);
    self.windows_api.set_cursor_position(&cursor_target_point);
  }

  pub fn move_cursor(&mut self, direction: Direction) {
    let windows = self.windows_api.get_all_visible_windows();
    let cursor_position = self.windows_api.get_cursor_position();
    let (ref_point, ref_window) = match self.find_window_at_cursor(&cursor_position, &windows) {
      Some(window_info) => (Point::from_center_of_rect(&window_info.rect), Some(window_info)),
      None => (cursor_position, None),
    };

    if let Some(target_window) = if let Some(vdm) = &self.virtual_desktop_manager {
      self.find_closest_window_in_direction(&ref_point, direction, &windows, vdm, ref_window)
    } else {
      None
    } {
      let target_point = Point::from_center_of_rect(&target_window.rect);
      self.move_focus_to_window(direction, target_window, &target_point);
    } else {
      trace!("No window found in [{:?}] direction, attempting to find monitor", direction);
      let all_monitors = self.windows_api.get_all_monitors();
      let this_monitor = self.windows_api.get_monitor_handle_for_point(&cursor_position);
      match all_monitors.get(direction, this_monitor) {
        Some(target_monitor) => self.move_focus_to_monitor(direction, target_monitor),
        None => {
          info!(
            "No window or monitor found in [{:?}] direction, did not move cursor",
            direction
          );
        }
      }
    };
  }

  pub fn near_maximise_or_restore(&mut self) {
    let (handle, placement, monitor_info) = match self.get_window_and_monitor_info() {
      Some(value) => value,
      None => return,
    };

    match self.is_near_maximised(&placement, &handle, &monitor_info) {
      true => self.restore_previous_placement(&self.known_windows, handle),
      false => {
        add_or_update_previous_placement(&mut self.known_windows, handle, placement);
        self.near_maximise_window(handle, monitor_info, self.margin());
      }
    }
  }

  pub fn minimise_window(&mut self) {
    let Some(window) = self.windows_api.get_foreground_window() else {
      return;
    };

    self.windows_api.do_minimise_window(window);
    if self.allow_moving_cursor_after_close_or_minimise {
      self.find_and_select_closest_window(window);
    }
  }

  pub fn restore_all_managed_windows(&mut self) {
    self.workspace_manager.restore_all_managed_windows();
  }

  fn margin(&self) -> i32 {
    let margin = self.configuration_provider.lock().unwrap().get_i32(WINDOW_MARGIN);
    if margin >= MINIMUM_WINDOW_MARGIN { margin } else { 0 }
  }

  fn restore_previous_placement(&self, known_windows: &HashMap<String, WindowPlacement>, handle: WindowHandle) {
    let window_id = format!("{:?}", handle.hwnd);
    if let Some(previous_placement) = known_windows.get(&window_id) {
      info!("Restoring previous placement for {}", window_id);
      self
        .windows_api
        .do_restore_window_placement(handle, previous_placement.clone());
    } else {
      warn!("No previous placement found for {}", window_id);
    }
  }

  fn is_near_maximised(&self, placement: &WindowPlacement, handle: &WindowHandle, monitor_info: &MonitorInfo) -> bool {
    if placement.show_cmd == SW_MAXIMIZE.0 as u32 && self.margin() < MINIMUM_WINDOW_MARGIN {
      debug!("{} is reported as maximised and margins are disabled", handle);
      return true;
    }

    let work_area = monitor_info.work_area;
    let expected_x = work_area.left + self.margin();
    let expected_y = work_area.top + self.margin();
    let expected_width = work_area.right - work_area.left - self.margin() * 2;
    let expected_height = work_area.bottom - work_area.top - self.margin() * 2;
    let sizing = Sizing::new(expected_x, expected_y, expected_width, expected_height);

    if let Some(rect) = self.windows_api.get_window_rect(*handle) {
      let result = (rect.left - expected_x).abs() <= REGULAR_TOLERANCE_IN_PX
        && (rect.top - expected_y).abs() <= REGULAR_TOLERANCE_IN_PX
        && (rect.right - rect.left - expected_width).abs() <= REGULAR_TOLERANCE_IN_PX
        && (rect.bottom - rect.top - expected_height).abs() <= REGULAR_TOLERANCE_IN_PX;

      log_actual_vs_expected(handle, &sizing, rect);
      debug!(
        "{} {} near-maximised (tolerance: {})",
        handle,
        if result { "is currently" } else { "is currently NOT" },
        REGULAR_TOLERANCE_IN_PX
      );

      result
    } else {
      warn!("{} has no window rect, assuming currently NOT near-maximised", handle);
      false
    }
  }

  fn is_three_quarter_near_maximised(
    &self,
    handle: &WindowHandle,
    monitor_info: &MonitorInfo,
    direction: Direction,
  ) -> bool {
    let expected = Sizing::three_quarter_near_maximised(monitor_info.work_area, direction, self.margin());

    if let Some(rect) = self.windows_api.get_window_rect(*handle) {
      let result = (rect.left - expected.x).abs() <= REGULAR_TOLERANCE_IN_PX
        && (rect.top - expected.y).abs() <= REGULAR_TOLERANCE_IN_PX
        && (rect.right - rect.left - expected.width).abs() <= REGULAR_TOLERANCE_IN_PX
        && (rect.bottom - rect.top - expected.height).abs() <= REGULAR_TOLERANCE_IN_PX;
      debug!(
        "{} {} three-quarter near-maximised in [{:?}] direction (tolerance: {})",
        handle,
        if result { "is currently" } else { "is currently NOT" },
        direction,
        REGULAR_TOLERANCE_IN_PX
      );
      result
    } else {
      warn!(
        "{} has no window rect, assuming currently NOT three-quarter near-maximised",
        handle
      );
      false
    }
  }

  fn near_maximise_window(&self, handle: WindowHandle, monitor_info: MonitorInfo, margin: i32) {
    info!("Near-maximising {}", handle);

    // First maximise to get the animation effect
    self.windows_api.do_maximise_window(handle);

    // Then resize the window to the expected size
    if margin >= MINIMUM_WINDOW_MARGIN {
      let work_area = monitor_info.work_area;
      let sizing = Sizing::near_maximised(work_area, margin);
      self.execute_window_resizing(handle, sizing);
    }
  }

  fn find_closest_window_in_direction<'a>(
    &self,
    reference_point: &Point,
    direction: Direction,
    windows: &'a Vec<Window>,
    virtual_desktop_manager: &IVirtualDesktopManager,
    reference_window: Option<&Window>,
  ) -> Option<&'a Window> {
    let mut closest_window = None;
    let mut closest_score = f64::MAX;

    for window in windows {
      // Skip windows that are not on the current desktop
      if !self.windows_api.is_window_on_current_desktop(virtual_desktop_manager, window) {
        continue;
      }

      let target_center_x = window.rect.left + (window.rect.right - window.rect.left) / 2;
      let target_center_y = window.rect.top + (window.rect.bottom - window.rect.top) / 2;
      let dx = target_center_x as i64 - reference_point.x() as i64;
      let dy = target_center_y as i64 - reference_point.y() as i64;

      // Skip windows that are not in the right direction, unless it has the same center as the reference window, if
      // the relevant configuration is enabled (see README for more information)
      let is_selecting_same_center_windows_disabled = !self
        .configuration_provider
        .lock()
        .expect(CONFIGURATION_PROVIDER_LOCK)
        .get_bool(ALLOW_SELECTING_SAME_CENTER_WINDOWS);
      if is_selecting_same_center_windows_disabled
        || (reference_window.is_some() && reference_window?.center != window.center)
        || (reference_window.is_some() && reference_window?.handle == window.handle)
      {
        match direction {
          Direction::Left if dx >= 0 => continue,
          Direction::Right if dx <= 0 => continue,
          Direction::Up if dy >= 0 => continue,
          Direction::Down if dy <= 0 => continue,
          _ => {}
        }
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
      trace!(
        "Score for {} is [{}] (i.e. normalised_angle={}, distance={})",
        window.handle,
        score.trunc(),
        angle,
        distance,
      );
      if score < closest_score {
        closest_score = score;
        closest_window = Some(window);
      }
    }

    closest_window
  }

  fn execute_window_resizing(&self, handle: WindowHandle, sizing: Sizing) {
    let placement = WindowPlacement::new_from_sizing(sizing.clone());
    self.windows_api.set_window_placement_and_force_repaint(handle, placement);

    // If margins are disabled, attempt a Desktop Window Manager-aware correction
    if self.margin() == 0
      && let Some(rect) = self
        .windows_api
        .get_extended_frame_bounds(handle)
        .or_else(|| self.windows_api.get_window_rect(handle))
      && let Some(compensating_rect) = calculate_compensating_rect_if_required(&rect, &sizing)
    {
      self.windows_api.set_window_position(handle, compensating_rect);
    }
  }

  /// Returns the window under the cursor, if any. If there are multiple windows under the cursor, the foreground window
  /// is returned if it's in the list. Otherwise, the window with the closest center point to the cursor is returned.
  fn find_window_at_cursor<'a>(&self, point: &Point, windows: &'a [Window]) -> Option<&'a Window> {
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
      if let Some(foreground_window) = self.windows_api.get_foreground_window()
        && let Some(window_info) = windows_under_cursor
          .iter()
          .find(|window_info| window_info.handle == foreground_window)
      {
        debug!(
          "Cursor is currently over foreground window {} \"{}\" at {point}",
          window_info.handle,
          window_info.title_trunc()
        );
        return Some(window_info);
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
        "Cursor is currently over window {} \"{}\" at {point} with a distance of {}",
        closest_window.handle,
        closest_window.title_trunc(),
        min_distance.trunc()
      );
      return Some(closest_window);
    }

    None
  }

  fn get_window_and_monitor_info(&self) -> Option<(WindowHandle, WindowPlacement, MonitorInfo)> {
    let window = self.windows_api.get_foreground_window()?;
    let placement = self.windows_api.get_window_placement(window)?;
    let monitor_info = self.windows_api.get_monitor_info_for_window(window)?;
    Some((window, placement, monitor_info))
  }

  fn move_focus_to_window(&self, direction: Direction, target_window: &Window, target_point: &Point) {
    self.windows_api.set_cursor_position(target_point);
    self.windows_api.set_foreground_window(WindowHandle::from(target_window));
    info!(
      "Moved cursor in direction [{:?}] to {} \"{}\" at {target_point}",
      direction,
      target_window.handle,
      target_window.title_trunc()
    );
  }

  fn move_focus_to_monitor(&self, direction: Direction, monitor: &Monitor) {
    self.windows_api.set_cursor_position(&monitor.center);
    info!(
      "Moved cursor in direction [{:?}] to {} on [{}]",
      direction, monitor.center, monitor
    );
  }

  fn find_closest_window(&self, cursor_position: Point, ignored_window: Option<WindowHandle>) -> Option<WindowHandle> {
    let mut closest_window = (vec![], f64::MAX);
    self
      .windows_api
      .get_all_visible_windows()
      .iter()
      .filter(|w| ignored_window != Some(w.handle))
      .for_each(|w| {
        let distance = cursor_position.distance_to(&w.center);
        trace!(
          "Distance from cursor position {} to window {} \"{}\" is {}",
          cursor_position,
          w.handle,
          w.title_trunc(),
          distance
        );
        if distance == closest_window.1 {
          closest_window.0.push(w.clone());
        } else if distance < closest_window.1 {
          closest_window.0.clear();
          closest_window.0.push(w.clone());
          closest_window.1 = distance;
        }
      });
    match closest_window.0.len() {
      0 => {
        trace!("No windows found close to {}", cursor_position);

        None
      }
      1 => Some(closest_window.0.first().map(|w| w.handle)?),
      _ => {
        let smallest_window = closest_window
          .0
          .iter()
          .min_by_key(|window| {
            let width = window.rect.right - window.rect.left;
            let height = window.rect.bottom - window.rect.top;
            width * height
          })
          .map(|window| window.handle)
          .expect("Failed to get the smallest window");
        trace!(
          "Found multiple windows closest to cursor position {}, returning {} which is the smallest one",
          cursor_position, smallest_window
        );

        Some(smallest_window)
      }
    }
  }

  fn find_and_select_closest_window(&mut self, window: WindowHandle) {
    let cursor_position = self.windows_api.get_cursor_position();
    if let Some(window) = self.find_closest_window(cursor_position, Some(window)) {
      self.windows_api.set_foreground_window(window);
      let window_info = self
        .windows_api
        .get_window_placement(window)
        .expect("Failed to get window placement");
      let target_point = Point::from_center_of_rect(&window_info.normal_position);
      self.windows_api.set_cursor_position(&target_point);
    } else {
      info!("No window found to move focus to after closing the current window");
    }
  }

  /// Determines whether the given window placement matches the expected sizing. If margins are disabled, allows a
  /// small tolerance when comparing against the DWM extended frame bounds to account for shadows/rounded corners added
  /// by the OS.
  ///
  /// Note: This extra check may be useful in all cases, but I don't understand the Windows API well enough yet, and
  /// I've never had this problem before despite my heavy use of this application.
  fn is_of_expected_size(&self, handle: WindowHandle, placement: &WindowPlacement, sizing: &Sizing) -> bool {
    let rect = placement.normal_position;
    let is_expected_size = rect.left == sizing.x
      && rect.top == sizing.y
      && rect.right - rect.left == sizing.width
      && rect.bottom - rect.top == sizing.height;
    if is_expected_size {
      log_actual_vs_expected(&handle, sizing, rect);
      debug!("{} is currently of expected size (exact placement match)", handle);
      return true;
    }

    if self.margin() == 0
      && let Some(compensating_rect) = self
        .windows_api
        .get_extended_frame_bounds(handle)
        .or_else(|| self.windows_api.get_window_rect(handle))
    {
      let is_compensating_match = (compensating_rect.left - sizing.x).abs() <= DWM_TOLERANCE_IN_PX
        && (compensating_rect.top - sizing.y).abs() <= DWM_TOLERANCE_IN_PX
        && (compensating_rect.right - compensating_rect.left - sizing.width).abs() <= DWM_TOLERANCE_IN_PX
        && (compensating_rect.bottom - compensating_rect.top - sizing.height).abs() <= DWM_TOLERANCE_IN_PX;
      log_actual_vs_expected(&handle, sizing, compensating_rect);
      debug!(
        "{} {} of expected size (dwm_tolerance: {})",
        handle,
        if is_compensating_match {
          "is currently"
        } else {
          "is currently NOT"
        },
        DWM_TOLERANCE_IN_PX
      );

      return is_compensating_match;
    }

    log_actual_vs_expected(&handle, sizing, rect);
    debug!("{} is currently NOT of expected size (strict placement comparison)", handle,);
    false
  }
}

fn add_or_update_previous_placement(
  known_windows: &mut HashMap<String, WindowPlacement>,
  handle: WindowHandle,
  placement: WindowPlacement,
) {
  let window_id = format!("{:?}", handle.hwnd);
  if known_windows.contains_key(&window_id) {
    known_windows.remove(&window_id);
    trace!(
      "Removing previous placement for window {} so that a new value can be added",
      handle
    );
  }

  known_windows.insert(window_id.clone(), placement);
  trace!("Adding/updating previous placement for window {}", handle);
}

fn log_actual_vs_expected(handle: &WindowHandle, sizing: &Sizing, rc: Rect) {
  debug!(
    "Expected size of {}: ({},{})x({},{})",
    handle, sizing.x, sizing.y, sizing.width, sizing.height
  );
  debug!(
    "Actual size of {}: ({},{})x({},{})",
    handle,
    rc.left,
    rc.top,
    rc.right - rc.left,
    rc.bottom - rc.top
  );
}

fn calculate_compensating_rect_if_required(rect: &Rect, sizing: &Sizing) -> Option<Rect> {
  let requested_left = sizing.x;
  let requested_right = sizing.x + sizing.width;
  let left_inset = rect.left - requested_left;
  let right_inset = requested_right - rect.right;

  if left_inset > 0 || right_inset > 0 {
    let compensating_left = requested_left - left_inset.max(0);
    let compensating_right = requested_right + right_inset.max(0);
    return Some(Rect::new(
      compensating_left,
      sizing.y,
      compensating_right,
      sizing.y + sizing.height,
    ));
  }
  None
}

#[cfg(test)]
mod tests {
  use crate::api::{MockWindowsApi, WindowsApi};
  use crate::common::{Direction, MonitorHandle, Point, Rect, Sizing, WindowHandle, WindowPlacement};
  use crate::configuration_provider::ConfigurationProvider;
  use crate::utils::{MINIMUM_WINDOW_DIMENSION, MINIMUM_WINDOW_DIMENSION_DIVISOR, create_temp_directory};
  use crate::window_manager::{DWM_TOLERANCE_IN_PX, WindowManager};
  use crate::workspace_manager::WorkspaceManager;
  use std::path::PathBuf;
  use std::sync::{Arc, Mutex};

  impl WindowManager<MockWindowsApi> {
    pub fn default(api: MockWindowsApi) -> Self {
      WindowManager {
        configuration_provider: Arc::new(Mutex::new(ConfigurationProvider::default())),
        known_windows: Default::default(),
        allow_moving_cursor_after_close_or_minimise: true,
        workspace_manager: WorkspaceManager::default(),
        virtual_desktop_manager: None,
        windows_api: api,
      }
    }

    pub fn new_test(api: MockWindowsApi, config_path: PathBuf) -> Self {
      WindowManager {
        configuration_provider: Arc::new(Mutex::new(ConfigurationProvider::new_test(config_path))),
        known_windows: Default::default(),
        allow_moving_cursor_after_close_or_minimise: true,
        workspace_manager: WorkspaceManager::default(),
        virtual_desktop_manager: None,
        windows_api: api,
      }
    }
  }

  fn with_margin_set_to(margin: i32, manager: &mut WindowManager<MockWindowsApi>) {
    manager
      .configuration_provider
      .lock()
      .unwrap()
      .set_i32(crate::configuration_provider::WINDOW_MARGIN, margin);
  }

  #[test]
  fn is_of_expected_size_test() {
    let handle = WindowHandle::new(1);
    let placement = WindowPlacement::new_from_sizing(Sizing::new(0, 0, 100, 100));
    let sizing = Sizing::new(0, 0, 100, 100);
    let manager = WindowManager::default(MockWindowsApi);
    assert!(manager.is_of_expected_size(handle, &placement, &sizing));

    let placement = WindowPlacement::new_from_sizing(Sizing::new(1, 0, 101, 100));
    assert!(!manager.is_of_expected_size(handle, &placement, &sizing));
  }

  #[test]
  fn near_maximise_window_when_window_is_not_near_maximised() {
    let monitor_handle = MonitorHandle::from(1);
    let window_handle = WindowHandle::new(1);
    let sizing = Sizing::new(0, 0, 100, 100);
    let initial_placement = WindowPlacement::new_from_sizing(sizing.clone());
    MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
    MockWindowsApi::place_window(window_handle, monitor_handle);
    let mut manager = WindowManager::default(MockWindowsApi);

    manager.near_maximise_or_restore();

    let actual_placement = manager.windows_api.get_window_placement(window_handle);
    let expected_placement = WindowPlacement::new_from_sizing(Sizing::new(20, 20, 160, 140));
    assert!(actual_placement.is_some());
    assert_eq!(actual_placement.unwrap(), expected_placement);
    assert!(manager.known_windows.contains_key(&format!("{:?}", window_handle.hwnd)));
    assert_eq!(
      *manager.known_windows.get(&format!("{:?}", window_handle.hwnd)).unwrap(),
      initial_placement
    );
  }

  #[test]
  fn restore_window_when_window_is_near_maximised() {
    let monitor_handle = MonitorHandle::from(1);
    let window_handle = WindowHandle::new(1);
    let sizing = Sizing::new(20, 20, 160, 140);
    MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
    MockWindowsApi::place_window(window_handle, monitor_handle);
    let mut manager = WindowManager::default(MockWindowsApi);
    let previous_placement = WindowPlacement::new_test();
    manager
      .known_windows
      .insert(format!("{:?}", window_handle.hwnd), previous_placement.clone());

    manager.near_maximise_or_restore();

    let actual_placement = manager.windows_api.get_window_placement(window_handle);
    assert!(actual_placement.is_some());
    assert_eq!(actual_placement.unwrap(), previous_placement);
  }

  #[test]
  fn move_window_on_the_same_monitor() {
    let monitor_handle = MonitorHandle::from(1);
    let window_handle = WindowHandle::new(1);
    let sizing = Sizing::new(20, 20, 160, 160);
    MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
    MockWindowsApi::place_window(window_handle, monitor_handle);
    let mock_api = MockWindowsApi;
    let mut manager = WindowManager::default(mock_api);

    manager.move_window(Direction::Right);

    let actual_placement = manager.windows_api.get_window_placement(window_handle);
    let monitor_info = mock_api
      .get_monitor_info_for_monitor(monitor_handle)
      .expect("Failed to get monitor info");
    let expected_sizing = Sizing::right_half_of_screen(monitor_info.work_area, 20);
    let expected_cursor_position = Point::from_center_of_sizing(&expected_sizing);
    let expected_placement = WindowPlacement::new_from_sizing(expected_sizing);
    assert!(actual_placement.is_some());
    assert_eq!(actual_placement.unwrap(), expected_placement);
    assert_eq!(manager.windows_api.get_cursor_position(), expected_cursor_position)
  }

  #[test]
  fn move_window_when_window_is_already_at_target_location() {
    let monitor_handle = MonitorHandle::from(1);
    let window_handle = WindowHandle::new(1);
    let sizing = Sizing::left_half_of_screen(Rect::new(0, 0, 200, 180), 20);
    MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing.clone(), false, false, true);
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
    MockWindowsApi::place_window(window_handle, monitor_handle);
    let mut manager = WindowManager::default(MockWindowsApi);

    manager.move_window(Direction::Left);

    let actual_placement = manager.windows_api.get_window_placement(window_handle);
    let expected_placement = WindowPlacement::new_from_sizing(sizing);
    assert!(actual_placement.is_some());
    assert_eq!(actual_placement.unwrap(), expected_placement);
    assert_eq!(manager.windows_api.get_cursor_position(), Point::default())
  }

  #[test]
  fn move_window_to_another_monitor() {
    let monitor_handle_1 = MonitorHandle::from(1);
    let window_handle = WindowHandle::new(1);
    let sizing = Sizing::right_half_of_screen(Rect::new(0, 0, 200, 180), 20);
    MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing.clone(), false, false, true);
    MockWindowsApi::add_monitor(monitor_handle_1, Rect::new(0, 0, 200, 200), true);
    MockWindowsApi::add_monitor(2.into(), Rect::new(200, 0, 400, 200), false);
    MockWindowsApi::place_window(window_handle, monitor_handle_1);
    let mut manager = WindowManager::default(MockWindowsApi);

    manager.move_window(Direction::Right);

    let actual_placement = manager.windows_api.get_window_placement(window_handle);
    let expected_placement = WindowPlacement::new_from_sizing(Sizing::near_maximised(Rect::new(200, 0, 400, 180), 20));
    assert!(actual_placement.is_some());
    assert_eq!(actual_placement.unwrap(), expected_placement);
    assert_eq!(manager.windows_api.get_cursor_position(), Point::new(300, 100))
  }

  #[test]
  fn move_cursor_moves_cursor_to_center_of_closest_window_on_other_monitor() {
    let current_monitor_handle = MonitorHandle::from(1);
    let target_monitor_handle = MonitorHandle::from(2);
    let target_work_area = Rect::new(200, 0, 400, 200);
    let window_handle = WindowHandle::new(1);
    let sizing = Sizing::right_half_of_screen(target_work_area, 20);
    MockWindowsApi::set_cursor_position(Point::new(0, 0));
    MockWindowsApi::add_or_update_window(window_handle, "Test".to_string(), sizing, false, false, true);
    MockWindowsApi::add_monitor(current_monitor_handle, Rect::new(0, 0, 200, 200), true);
    MockWindowsApi::add_monitor(target_monitor_handle, target_work_area, true);
    MockWindowsApi::place_window(window_handle, target_monitor_handle);
    let mut manager = WindowManager::default(MockWindowsApi);

    manager.move_cursor(Direction::Right);

    assert_eq!(manager.windows_api.get_foreground_window(), Some(window_handle));
    assert_eq!(manager.windows_api.get_cursor_position(), target_work_area.center());
  }

  #[test]
  fn move_cursor_does_nothing_when_there_is_no_window_or_monitor_to_move_to() {
    let monitor_handle = MonitorHandle::from(1);
    let window_handle_1 = WindowHandle::new(1);
    let left_sizing = Sizing::left_half_of_screen(Rect::new(0, 0, 200, 180), 20);
    let initial_cursor_position = Point::new(0, 0);
    MockWindowsApi::set_cursor_position(initial_cursor_position);
    MockWindowsApi::add_or_update_window(window_handle_1, "Test".to_string(), left_sizing, false, false, true);
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
    MockWindowsApi::place_window(window_handle_1, monitor_handle);
    let mut manager = WindowManager::default(MockWindowsApi);

    manager.move_cursor(Direction::Right);

    assert_eq!(manager.windows_api.get_cursor_position(), initial_cursor_position);
  }

  #[test]
  fn close_window_does_close_window() {
    let window_handle = WindowHandle::new(1);
    let monitor_handle = MonitorHandle::from(1);
    MockWindowsApi::add_or_update_window(window_handle, "Test".to_string(), Sizing::default(), false, false, true);
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
    MockWindowsApi::place_window(window_handle, monitor_handle);
    let mut manager = WindowManager::default(MockWindowsApi);

    manager.close_window();

    assert!(
      !manager
        .windows_api
        .get_all_visible_windows()
        .iter()
        .any(|w| w.handle == window_handle)
    );
  }

  #[test]
  fn close_window_moves_cursor_to_closest_window_when_enabled() {
    let window_handle_1 = WindowHandle::new(1);
    let window_handle_2 = WindowHandle::new(2);
    let monitor_handle = MonitorHandle::from(1);
    MockWindowsApi::add_or_update_window(window_handle_1, "Test 1".to_string(), Sizing::default(), false, false, true);
    let sizing = Sizing::new(0, 0, 100, 100);
    MockWindowsApi::add_or_update_window(window_handle_2, "Test 2".to_string(), sizing, false, false, false);
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
    MockWindowsApi::place_window(window_handle_1, monitor_handle);
    MockWindowsApi::place_window(window_handle_2, monitor_handle);
    let mut manager = WindowManager::default(MockWindowsApi);

    manager.close_window();

    assert!(
      !manager
        .windows_api
        .get_all_visible_windows()
        .iter()
        .any(|w| w.handle == window_handle_1)
    );
    assert_eq!(manager.windows_api.get_cursor_position(), Point::new(50, 50));
  }

  #[test]
  fn close_window_does_not_move_cursor_to_closest_window_when_disabled() {
    let window_handle_1 = WindowHandle::new(1);
    let window_handle_2 = WindowHandle::new(2);
    let monitor_handle = MonitorHandle::from(1);
    MockWindowsApi::add_or_update_window(window_handle_1, "Test 1".to_string(), Sizing::default(), false, false, true);
    let sizing = Sizing::new(0, 0, 100, 100);
    MockWindowsApi::add_or_update_window(window_handle_2, "Test 2".to_string(), sizing, false, false, false);
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
    MockWindowsApi::place_window(window_handle_1, monitor_handle);
    MockWindowsApi::place_window(window_handle_2, monitor_handle);
    let mut manager = WindowManager::default(MockWindowsApi);
    manager.allow_moving_cursor_after_close_or_minimise = false;

    manager.close_window();

    assert!(
      !manager
        .windows_api
        .get_all_visible_windows()
        .iter()
        .any(|w| w.handle == window_handle_1)
    );
    assert_eq!(manager.windows_api.get_cursor_position(), Point::default());
  }

  #[test]
  fn close_window_fails_silently() {
    let mut manager = WindowManager::default(MockWindowsApi);
    assert!(manager.windows_api.get_all_visible_windows().is_empty());

    manager.close_window();

    assert!(manager.windows_api.get_all_visible_windows().is_empty());
  }

  #[test]
  fn find_closest_window_returns_none_when_no_windows_are_visible() {
    let cursor_position = Point::new(100, 100);
    let manager = WindowManager::default(MockWindowsApi);

    let result = manager.find_closest_window(cursor_position, None);

    assert!(result.is_none());
  }

  #[test]
  fn find_closest_window_returns_window_under_cursor() {
    let cursor_position = Point::new(50, 50);
    let window_handle = WindowHandle::new(1);
    MockWindowsApi::set_cursor_position(cursor_position);
    MockWindowsApi::add_or_update_window(
      window_handle,
      "Test Window".to_string(),
      Rect::new(0, 0, 100, 100).into(),
      false,
      false,
      true,
    );
    let manager = WindowManager::default(MockWindowsApi);

    let result = manager.find_closest_window(cursor_position, None);

    assert_eq!(result, Some(window_handle));
  }

  #[test]
  fn find_closest_window_returns_smallest_window_when_multiple_windows_overlap() {
    let cursor_position = Point::new(50, 50);
    let expected_window_handle = WindowHandle::new(2);
    MockWindowsApi::set_cursor_position(cursor_position);
    MockWindowsApi::add_or_update_window(
      WindowHandle::new(1),
      "Window 1".to_string(),
      Rect::new(40, 40, 60, 60).into(),
      false,
      false,
      true,
    );
    MockWindowsApi::add_or_update_window(
      expected_window_handle,
      "Window 2".to_string(),
      Rect::new(45, 45, 55, 55).into(),
      false,
      false,
      false,
    );
    let manager = WindowManager::default(MockWindowsApi);

    let result = manager.find_closest_window(cursor_position, None);

    assert_eq!(result, Some(expected_window_handle));
  }

  #[test]
  fn find_closest_window_returns_closest_window_when_cursor_is_outside_all_windows() {
    let cursor_position = Point::new(1000, 1000);
    let window_handle = WindowHandle::new(1);
    MockWindowsApi::set_cursor_position(cursor_position);
    MockWindowsApi::add_or_update_window(
      window_handle,
      "Test Window".to_string(),
      Rect::new(0, 0, 100, 100).into(),
      false,
      false,
      false,
    );
    let manager = WindowManager::default(MockWindowsApi);

    let result = manager.find_closest_window(cursor_position, None);

    assert_eq!(result, Some(window_handle));
  }

  #[test]
  fn find_closest_window_ignores_minimised_or_hidden_windows() {
    let cursor_position = Point::new(50, 50);
    let expected_window_handle = WindowHandle::new(2);
    MockWindowsApi::set_cursor_position(cursor_position);
    MockWindowsApi::add_or_update_window(
      WindowHandle::new(1),
      "Window 1".to_string(),
      Rect::new(40, 40, 60, 60).into(),
      true,
      false,
      true,
    );
    MockWindowsApi::add_or_update_window(
      expected_window_handle,
      "Window 2".to_string(),
      Rect::new(45, 45, 55, 55).into(),
      false,
      true,
      false,
    );
    let manager = WindowManager::default(MockWindowsApi);

    let result = manager.find_closest_window(cursor_position, None);

    assert!(result.is_none());
  }

  #[test]
  fn find_closest_window_ignores_provided_window() {
    let cursor_position = Point::new(50, 50);
    let expected_window_handle = WindowHandle::new(2);
    MockWindowsApi::set_cursor_position(cursor_position);
    MockWindowsApi::add_or_update_window(
      WindowHandle::new(1),
      "Window 1".to_string(),
      Rect::new(0, 0, 60, 60).into(),
      true,
      false,
      true,
    );
    let manager = WindowManager::default(MockWindowsApi);

    let result = manager.find_closest_window(cursor_position, Some(expected_window_handle));

    assert!(result.is_none());
  }

  #[test]
  fn minimise_window_when_no_other_window_present() {
    let window_handle = WindowHandle::new(1);
    MockWindowsApi::add_or_update_window(window_handle, "Test".to_string(), Sizing::default(), false, false, true);
    let mut manager = WindowManager::default(MockWindowsApi);
    assert!(!manager.windows_api.is_window_minimised(window_handle));

    manager.minimise_window();

    assert!(manager.windows_api.is_window_minimised(window_handle));
    assert_eq!(manager.windows_api.get_cursor_position(), Point::default());
    assert_eq!(manager.windows_api.get_foreground_window(), None);
  }

  #[test]
  fn minimise_window_when_another_window_is_present() {
    let window_handle = WindowHandle::new(1);
    let sizing = Sizing::new(50, 50, 100, 100);
    let other_window_handle = WindowHandle::new(2);
    MockWindowsApi::add_or_update_window(window_handle, "Test".to_string(), Sizing::default(), false, false, true);
    MockWindowsApi::add_or_update_window(other_window_handle, "Other".to_string(), sizing.clone(), false, false, false);
    let mut manager = WindowManager::default(MockWindowsApi);
    assert!(!manager.windows_api.is_window_minimised(window_handle));

    manager.minimise_window();

    assert!(manager.windows_api.is_window_minimised(window_handle));
    assert_eq!(
      manager.windows_api.get_cursor_position(),
      Point::from_center_of_sizing(&sizing),
      "Cursor position should be set to the center of the closest, non-minimised window"
    );
    assert_eq!(manager.windows_api.get_foreground_window(), Some(other_window_handle));
  }

  #[test]
  fn near_maximise_window_with_margin_below_threshold_does_not_resize() {
    let monitor_handle = MonitorHandle::from(1);
    let window_handle = WindowHandle::new(1);
    let initial_sizing = Sizing::new(50, 50, 100, 100);
    MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), initial_sizing, false, false, true);
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
    MockWindowsApi::place_window(window_handle, monitor_handle);
    let manager = WindowManager::default(MockWindowsApi);
    let monitor_info = manager
      .windows_api
      .get_monitor_info_for_monitor(monitor_handle)
      .expect("Failed to get monitor info");

    manager.near_maximise_window(window_handle, monitor_info, 3);

    let expected_sizing = Sizing::near_maximised(monitor_info.work_area, 0);
    let expected_placement = WindowPlacement::new_from_sizing(expected_sizing);
    let actual_placement = manager.windows_api.get_window_placement(window_handle);
    assert!(actual_placement.is_some());
    assert_eq!(actual_placement.unwrap(), expected_placement);
  }

  #[test]
  fn near_maximise_window_with_margin_above_threshold_resizes() {
    let monitor_handle = MonitorHandle::from(1);
    let window_handle = WindowHandle::new(1);
    let initial_sizing = Sizing::new(50, 50, 100, 100);
    let initial_placement = WindowPlacement::new_from_sizing(initial_sizing.clone());
    MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), initial_sizing, false, false, true);
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
    MockWindowsApi::place_window(window_handle, monitor_handle);
    let manager = WindowManager::default(MockWindowsApi);
    let monitor_info = manager
      .windows_api
      .get_monitor_info_for_monitor(monitor_handle)
      .expect("Failed to get monitor info");
    let margin = 10;

    manager.near_maximise_window(window_handle, monitor_info, margin);

    let actual_placement = manager.windows_api.get_window_placement(window_handle);
    let expected_sizing = Sizing::near_maximised(monitor_info.work_area, margin);
    let expected_placement = WindowPlacement::new_from_sizing(expected_sizing);
    assert!(actual_placement.is_some());
    assert_eq!(actual_placement.clone().unwrap(), expected_placement);
    assert_ne!(actual_placement.unwrap(), initial_placement);
  }

  #[test]
  fn near_maximise_or_restore_with_zero_margin_can_restore_initial_position() {
    let monitor_handle = MonitorHandle::from(1);
    let window_handle = WindowHandle::new(1);
    let initial_sizing = Sizing::new(50, 50, 100, 100);
    let initial_placement = WindowPlacement::new_from_sizing(initial_sizing.clone());
    MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), initial_sizing, false, false, true);
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
    MockWindowsApi::place_window(window_handle, monitor_handle);
    let mut manager = WindowManager::default(MockWindowsApi);
    let monitor_info = manager
      .windows_api
      .get_monitor_info_for_monitor(monitor_handle)
      .expect("Failed to get monitor info");
    with_margin_set_to(0, &mut manager);

    manager.near_maximise_or_restore();

    let expected_sizing = Sizing::near_maximised(monitor_info.work_area, 0);
    let maximised_placement = WindowPlacement::new_from_sizing(expected_sizing);
    let current_placement = manager
      .windows_api
      .get_window_placement(window_handle)
      .expect("Failed to get placement after maximise");
    assert_eq!(current_placement, maximised_placement, "Window should be maximised");

    manager.near_maximise_or_restore();

    let current_placement = manager
      .windows_api
      .get_window_placement(window_handle)
      .expect("Failed to get placement after restore");
    assert_eq!(
      current_placement, initial_placement,
      "Window should be restored to its initial placement"
    );
  }

  #[test]
  fn is_of_expected_size_returns_true_when_same_size() {
    let directory = create_temp_directory();
    let path = directory.path().join("file.toml");
    let mut manager = WindowManager::new_test(MockWindowsApi, path);
    let window_handle = WindowHandle::new(1);
    let rect = Rect::new(0, 0, 1280, 720);
    let placement = WindowPlacement::new_from_rect(rect);
    let sizing = rect.into();
    with_margin_set_to(10, &mut manager);

    assert!(manager.is_of_expected_size(window_handle, &placement, &sizing));
  }

  #[test]
  fn is_of_expected_size_returns_false_when_different_size() {
    let directory = create_temp_directory();
    let path = directory.path().join("file.toml");
    let mut manager = WindowManager::new_test(MockWindowsApi, path);
    let window_handle = WindowHandle::new(1);
    let rect = Rect::new(0, 0, 1280, 720);
    let placement = WindowPlacement::new_from_rect(rect);
    let mut sizing: Sizing = rect.into();
    sizing.x += 5; // Introducing a discrepancy here
    with_margin_set_to(10, &mut manager);

    assert!(!manager.is_of_expected_size(window_handle, &placement, &sizing));
  }

  #[test]
  fn is_of_expected_size_returns_true_when_difference_is_within_dwm_tolerance() {
    let directory = create_temp_directory();
    let path = directory.path().join("file.toml");
    let window_handle = WindowHandle::new(1);
    let rect = Rect::new(0, 0, 1280, 720);
    let placement = WindowPlacement::new_from_rect(rect);
    let expected_sizing = Sizing::new(0, 0, 640, 720);
    // Windows API returns a different sizing that is within the tolerance for a no-margin configuration
    let api_sizing = Sizing::new(DWM_TOLERANCE_IN_PX - 1, DWM_TOLERANCE_IN_PX - 1, 640, 720);
    MockWindowsApi::add_or_update_window(
      window_handle,
      "Test Window".to_string(),
      api_sizing.clone(),
      false,
      false,
      true,
    );
    let mut manager = WindowManager::new_test(MockWindowsApi, path);
    with_margin_set_to(0, &mut manager);

    assert!(manager.is_of_expected_size(window_handle, &placement, &expected_sizing));
  }

  #[test]
  fn is_of_expected_size_returns_false_when_difference_is_outside_dwm_tolerance() {
    let directory = create_temp_directory();
    let path = directory.path().join("file.toml");
    let window_handle = WindowHandle::new(1);
    let rect = Rect::new(0, 0, 1280, 720);
    let placement = WindowPlacement::new_from_rect(rect);
    let expected_sizing = Sizing::new(0, 0, 640, 720);
    // Windows API returns a different sizing that is outside the tolerance for a no-margin configuration
    let api_sizing = Sizing::new(15, 0, 640, 720);
    MockWindowsApi::add_or_update_window(
      window_handle,
      "Test Window".to_string(),
      api_sizing.clone(),
      false,
      false,
      true,
    );
    let mut manager = WindowManager::new_test(MockWindowsApi, path);
    with_margin_set_to(0, &mut manager);

    assert!(!manager.is_of_expected_size(window_handle, &placement, &expected_sizing));
  }

  #[test]
  fn resize_window_steps_near_maximised_down_to_three_quarter_in_direction() {
    let monitor_handle = MonitorHandle::from(1);
    let window_handle = WindowHandle::new(1);
    // Monitor area bottom is 20px more than work_area bottom (mock subtracts 20 for taskbar)
    let work_area = Rect::new(0, 0, 2000, 1000);
    let sizing = Sizing::near_maximised(work_area, 20);
    MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
    MockWindowsApi::place_window(window_handle, monitor_handle);
    let mut manager = WindowManager::default(MockWindowsApi);

    manager.resize_window(Direction::Left);

    let actual_placement = manager.windows_api.get_window_placement(window_handle);
    let expected_sizing = Sizing::three_quarter_near_maximised(work_area, Direction::Left, 20);
    let expected_placement = WindowPlacement::new_from_sizing(expected_sizing.clone());
    assert!(actual_placement.is_some());
    assert_eq!(actual_placement.unwrap(), expected_placement);
    assert_eq!(
      manager.windows_api.get_cursor_position(),
      Point::from_center_of_sizing(&expected_sizing)
    );
  }

  #[test]
  fn resize_window_halves_non_near_maximised_window_in_direction() {
    let monitor_handle = MonitorHandle::from(1);
    let window_handle = WindowHandle::new(1);
    // Width must be > 2 * dynamic_min (2 * W/4 = 1000) so that halved width (590) > dynamic_min (500)
    let sizing = Sizing::new(20, 20, 1200, 960);
    MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing.clone(), false, false, true);
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
    MockWindowsApi::place_window(window_handle, monitor_handle);
    let mut manager = WindowManager::default(MockWindowsApi);

    manager.resize_window(Direction::Left);

    let actual_placement = manager.windows_api.get_window_placement(window_handle);
    let expected_sizing = sizing.halved(Direction::Left, 20);
    let expected_placement = WindowPlacement::new_from_sizing(expected_sizing.clone());
    assert!(actual_placement.is_some());
    assert_eq!(actual_placement.unwrap(), expected_placement);
    assert_eq!(
      manager.windows_api.get_cursor_position(),
      Point::from_center_of_sizing(&expected_sizing)
    );
  }

  #[test]
  fn resize_window_steps_near_maximised_down_to_three_quarter_down() {
    let monitor_handle = MonitorHandle::from(1);
    let window_handle = WindowHandle::new(1);
    let work_area = Rect::new(0, 0, 2000, 1000);
    let sizing = Sizing::near_maximised(work_area, 20);
    MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
    MockWindowsApi::place_window(window_handle, monitor_handle);
    let mut manager = WindowManager::default(MockWindowsApi);

    manager.resize_window(Direction::Down);

    let actual_placement = manager.windows_api.get_window_placement(window_handle);
    let expected_sizing = Sizing::three_quarter_near_maximised(work_area, Direction::Down, 20);
    let expected_placement = WindowPlacement::new_from_sizing(expected_sizing.clone());
    assert!(actual_placement.is_some());
    assert_eq!(actual_placement.unwrap(), expected_placement);
    assert_eq!(
      manager.windows_api.get_cursor_position(),
      Point::from_center_of_sizing(&expected_sizing)
    );
  }

  #[test]
  fn resize_window_steps_three_quarter_left_down_to_left_half_of_screen() {
    let monitor_handle = MonitorHandle::from(1);
    let window_handle = WindowHandle::new(1);
    let work_area = Rect::new(0, 0, 2000, 1000);
    let sizing = Sizing::three_quarter_near_maximised(work_area, Direction::Left, 20);
    MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
    MockWindowsApi::place_window(window_handle, monitor_handle);
    let mut manager = WindowManager::default(MockWindowsApi);

    manager.resize_window(Direction::Left);

    let actual_placement = manager.windows_api.get_window_placement(window_handle);
    let expected_sizing = Sizing::left_half_of_screen(work_area, 20);
    let expected_placement = WindowPlacement::new_from_sizing(expected_sizing.clone());
    assert!(actual_placement.is_some());
    assert_eq!(actual_placement.unwrap(), expected_placement);
    assert_eq!(
      manager.windows_api.get_cursor_position(),
      Point::from_center_of_sizing(&expected_sizing)
    );
  }

  #[test]
  fn resize_window_steps_three_quarter_down_down_to_bottom_half_of_screen() {
    let monitor_handle = MonitorHandle::from(1);
    let window_handle = WindowHandle::new(1);
    let work_area = Rect::new(0, 0, 2000, 1000);
    let sizing = Sizing::three_quarter_near_maximised(work_area, Direction::Down, 20);
    MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing, false, false, true);
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
    MockWindowsApi::place_window(window_handle, monitor_handle);
    let mut manager = WindowManager::default(MockWindowsApi);

    manager.resize_window(Direction::Down);

    let actual_placement = manager.windows_api.get_window_placement(window_handle);
    let expected_sizing = Sizing::bottom_half_of_screen(work_area, 20);
    let expected_placement = WindowPlacement::new_from_sizing(expected_sizing.clone());
    assert!(actual_placement.is_some());
    assert_eq!(actual_placement.unwrap(), expected_placement);
    assert_eq!(
      manager.windows_api.get_cursor_position(),
      Point::from_center_of_sizing(&expected_sizing)
    );
  }

  #[test]
  fn resize_window_three_quarter_left_halves_normally_in_down_direction() {
    // A 75%-wide window (from Left resize) should NOT trigger the 3/4 rule when pressing Down
    let monitor_handle = MonitorHandle::from(1);
    let window_handle = WindowHandle::new(1);
    let work_area = Rect::new(0, 0, 2000, 1000);
    let sizing = Sizing::three_quarter_near_maximised(work_area, Direction::Left, 20);
    MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing.clone(), false, false, true);
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1020), true);
    MockWindowsApi::place_window(window_handle, monitor_handle);
    let mut manager = WindowManager::default(MockWindowsApi);

    manager.resize_window(Direction::Down);

    let actual_placement = manager.windows_api.get_window_placement(window_handle);
    let expected_sizing = sizing.halved(Direction::Down, 20);
    let expected_placement = WindowPlacement::new_from_sizing(expected_sizing.clone());
    assert!(actual_placement.is_some());
    assert_eq!(actual_placement.unwrap(), expected_placement);
    assert_eq!(
      manager.windows_api.get_cursor_position(),
      Point::from_center_of_sizing(&expected_sizing)
    );
  }

  #[test]
  fn resize_window_does_nothing_when_dynamic_minimum_exceeds_constant_and_result_is_below_it() {
    // Use a screen wide enough that W/DIVISOR > MINIMUM_WINDOW_DIMENSION, so the dynamic minimum
    // takes over. Work area width = MINIMUM * DIVISOR * 2, giving dynamic_min = MINIMUM * 2.
    // The window is sized so its halved width falls between the constant and the dynamic minimum.
    let monitor_handle = MonitorHandle::from(1);
    let window_handle = WindowHandle::new(1);
    let work_area_width = MINIMUM_WINDOW_DIMENSION * MINIMUM_WINDOW_DIMENSION_DIVISOR * 2;
    // dynamic_min = work_area_width / DIVISOR = MINIMUM_WINDOW_DIMENSION * 2
    let sizing = Sizing::new(20, 20, MINIMUM_WINDOW_DIMENSION * 3, 600);
    // halved width = (MINIMUM * 3) / 2 - half_margin (with default margin 20)
    // = 375 - 10 = 365, which is > MINIMUM (250) but < dynamic_min (500) → blocked
    MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing.clone(), false, false, true);
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, work_area_width, 1020), true);
    MockWindowsApi::place_window(window_handle, monitor_handle);
    let initial_cursor = Point::default();
    MockWindowsApi::set_cursor_position(initial_cursor);
    let mut manager = WindowManager::default(MockWindowsApi);

    manager.resize_window(Direction::Left);

    let actual_placement = manager.windows_api.get_window_placement(window_handle);
    assert!(actual_placement.is_some());
    assert_eq!(
      actual_placement.unwrap(),
      WindowPlacement::new_from_sizing(sizing),
      "Window should not have been resized"
    );
    assert_eq!(
      manager.windows_api.get_cursor_position(),
      initial_cursor,
      "Cursor should not have moved"
    );
  }

  #[test]
  fn resize_window_allows_resize_when_constant_is_larger_than_quarter_screen() {
    // Dynamic min = max(MINIMUM_WINDOW_DIMENSION, work_area/4). On a 1000px-wide screen the quarter
    // is 250px, which is smaller than the constant (350px). A window whose halved width (365px)
    // exceeds the constant should be allowed to resize.
    let monitor_handle = MonitorHandle::from(1);
    let window_handle = WindowHandle::new(1);
    // Monitor 1000px wide → work_area 1000px wide → quarter = 250 < MINIMUM_WINDOW_DIMENSION (350)
    let sizing = Sizing::new(20, 20, 750, 560);
    MockWindowsApi::add_or_update_window(window_handle, "Test Window".to_string(), sizing.clone(), false, false, true);
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 1000, 620), true);
    MockWindowsApi::place_window(window_handle, monitor_handle);
    let mut manager = WindowManager::default(MockWindowsApi);

    manager.resize_window(Direction::Left);

    let actual_placement = manager.windows_api.get_window_placement(window_handle);
    let expected_sizing = sizing.halved(Direction::Left, 20);
    let expected_placement = WindowPlacement::new_from_sizing(expected_sizing.clone());
    assert!(actual_placement.is_some());
    assert_eq!(
      actual_placement.unwrap(),
      expected_placement,
      "Window should have been resized"
    );
    assert_eq!(
      manager.windows_api.get_cursor_position(),
      Point::from_center_of_sizing(&expected_sizing)
    );
  }

  #[test]
  fn resize_window_halves_left_half_of_screen_in_left_direction() {
    // With DIVISOR=8, dynamic_min = max(MINIMUM, W/8). On a 2000px screen, dynamic_min = max(250, 250) = 250.
    // Halving left_half produces ~475px > 250 → resize succeeds.
    let monitor_handle = MonitorHandle::from(1);
    let window_handle = WindowHandle::new(1);
    let work_area = Rect::new(0, 0, MINIMUM_WINDOW_DIMENSION * MINIMUM_WINDOW_DIMENSION_DIVISOR, 1000);
    let left_half = Sizing::left_half_of_screen(work_area, 20);
    MockWindowsApi::add_or_update_window(
      window_handle,
      "Test Window".to_string(),
      left_half.clone(),
      false,
      false,
      true,
    );
    MockWindowsApi::add_monitor(
      monitor_handle,
      Rect::new(0, 0, MINIMUM_WINDOW_DIMENSION * MINIMUM_WINDOW_DIMENSION_DIVISOR, 1020),
      true,
    );
    MockWindowsApi::place_window(window_handle, monitor_handle);
    let initial_cursor = Point::default();
    MockWindowsApi::set_cursor_position(initial_cursor);
    let mut manager = WindowManager::default(MockWindowsApi);

    manager.resize_window(Direction::Left);

    let expected = left_half.halved(Direction::Left, 20);
    let actual_placement = manager.windows_api.get_window_placement(window_handle);
    assert!(actual_placement.is_some());
    assert_eq!(
      actual_placement.unwrap(),
      WindowPlacement::new_from_sizing(expected),
      "Window should have been halved"
    );
    assert_ne!(
      manager.windows_api.get_cursor_position(),
      initial_cursor,
      "Cursor should have moved"
    );
  }

  #[test]
  fn resize_window_halves_right_half_of_screen_in_right_direction() {
    // Mirror of the Left case: with DIVISOR=8, dynamic_min = max(250, 250) = 250.
    // Halving right_half produces ~475px > 250 → resize succeeds.
    let monitor_handle = MonitorHandle::from(1);
    let window_handle = WindowHandle::new(1);
    let work_area = Rect::new(0, 0, MINIMUM_WINDOW_DIMENSION * MINIMUM_WINDOW_DIMENSION_DIVISOR, 1000);
    let right_half = Sizing::right_half_of_screen(work_area, 20);
    MockWindowsApi::add_or_update_window(
      window_handle,
      "Test Window".to_string(),
      right_half.clone(),
      false,
      false,
      true,
    );
    MockWindowsApi::add_monitor(
      monitor_handle,
      Rect::new(0, 0, MINIMUM_WINDOW_DIMENSION * MINIMUM_WINDOW_DIMENSION_DIVISOR, 1020),
      true,
    );
    MockWindowsApi::place_window(window_handle, monitor_handle);
    let initial_cursor = Point::default();
    MockWindowsApi::set_cursor_position(initial_cursor);
    let mut manager = WindowManager::default(MockWindowsApi);

    manager.resize_window(Direction::Right);

    let expected = right_half.halved(Direction::Right, 20);
    let actual_placement = manager.windows_api.get_window_placement(window_handle);
    assert!(actual_placement.is_some());
    assert_eq!(
      actual_placement.unwrap(),
      WindowPlacement::new_from_sizing(expected),
      "Window should have been halved"
    );
    assert_ne!(
      manager.windows_api.get_cursor_position(),
      initial_cursor,
      "Cursor should have moved"
    );
  }

  #[test]
  fn resize_window_does_nothing_when_below_minimum_dimension() {
    let monitor_handle = MonitorHandle::from(1);
    let window_handle = WindowHandle::new(1);
    let small_sizing = Sizing::new(20, 20, 200, 200);
    MockWindowsApi::add_or_update_window(
      window_handle,
      "Test Window".to_string(),
      small_sizing.clone(),
      false,
      false,
      true,
    );
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 2000, 1000), true);
    MockWindowsApi::place_window(window_handle, monitor_handle);
    let initial_cursor = Point::default();
    MockWindowsApi::set_cursor_position(initial_cursor);
    let mut manager = WindowManager::default(MockWindowsApi);

    manager.resize_window(Direction::Left);

    let actual_placement = manager.windows_api.get_window_placement(window_handle);
    let expected_placement = WindowPlacement::new_from_sizing(small_sizing);
    assert!(actual_placement.is_some());
    assert_eq!(
      actual_placement.unwrap(),
      expected_placement,
      "Window should not have been resized"
    );
    assert_eq!(
      manager.windows_api.get_cursor_position(),
      initial_cursor,
      "Cursor should not have moved"
    );
  }
}
