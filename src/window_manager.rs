use crate::api::WindowsApi;
use crate::common::*;
use crate::configuration_provider::{
  ADDITIONAL_WORKSPACE_COUNT, ALLOW_SELECTING_SAME_CENTER_WINDOWS, ConfigurationProvider, WINDOW_MARGIN,
};
use crate::utils::CONFIGURATION_PROVIDER_LOCK;
use crate::workspace_manager::WorkspaceManager;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use windows::Win32::UI::Shell::IVirtualDesktopManager;

const TOLERANCE_IN_PX: i32 = 2;

pub struct WindowManager<T: WindowsApi> {
  configuration_provider: Arc<Mutex<ConfigurationProvider>>,
  known_windows: HashMap<String, WindowPlacement>,
  workspace_manager: WorkspaceManager<T>,
  virtual_desktop_manager: Option<IVirtualDesktopManager>,
  windows_api: T,
}

impl<T: WindowsApi + Clone> WindowManager<T> {
  pub fn new(configuration_provider: Arc<Mutex<ConfigurationProvider>>, api: T) -> Self {
    let additional_workspace_count = configuration_provider
      .lock()
      .expect(CONFIGURATION_PROVIDER_LOCK)
      .get_i32(ADDITIONAL_WORKSPACE_COUNT);
    let window_margin = configuration_provider
      .lock()
      .expect(CONFIGURATION_PROVIDER_LOCK)
      .get_i32(WINDOW_MARGIN);
    let workspace_manager = WorkspaceManager::new(additional_workspace_count, window_margin, api.clone());

    Self {
      known_windows: HashMap::new(),
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
    self.find_and_select_closest_window(window);
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

    match is_of_expected_size(handle, &placement, &sizing) {
      true => {
        let all_monitors = self.windows_api.get_all_monitors();
        let this_monitor = self.windows_api.get_monitor_handle_for_window_handle(handle);
        let target_monitor = all_monitors.get(direction, this_monitor);
        if let Some(target_monitor) = target_monitor {
          debug!("Moving window to [{}]", target_monitor);
          self.windows_api.set_window_position(handle, target_monitor.work_area);
          self.near_maximize_window(handle, MonitorInfo::from(target_monitor), self.margin());
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

    match self.is_near_maximized(&placement, &handle, &monitor_info) {
      true => self.restore_previous_placement(&self.known_windows, handle),
      false => {
        add_or_update_previous_placement(&mut self.known_windows, handle, placement);
        self.near_maximize_window(handle, monitor_info, self.margin());
      }
    }
  }

  pub fn minimise_window(&mut self) {
    let Some(window) = self.windows_api.get_foreground_window() else {
      return;
    };

    self.windows_api.do_minimise_window(window);
    self.find_and_select_closest_window(window);
  }

  pub fn restore_all_managed_windows(&mut self) {
    self.workspace_manager.restore_all_managed_windows();
  }

  fn margin(&self) -> i32 {
    self.configuration_provider.lock().unwrap().get_i32(WINDOW_MARGIN)
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

  fn is_near_maximized(&self, placement: &WindowPlacement, handle: &WindowHandle, monitor_info: &MonitorInfo) -> bool {
    let work_area = monitor_info.work_area;
    let expected_x = work_area.left + self.margin();
    let expected_y = work_area.top + self.margin();
    let expected_width = work_area.right - work_area.left - self.margin() * 2;
    let expected_height = work_area.bottom - work_area.top - self.margin() * 2;
    let rect = placement.normal_position;
    let result = (rect.left - expected_x).abs() <= TOLERANCE_IN_PX
      && (rect.top - expected_y).abs() <= TOLERANCE_IN_PX
      && (rect.right - rect.left - expected_width).abs() <= TOLERANCE_IN_PX
      && (rect.bottom - rect.top - expected_height).abs() <= TOLERANCE_IN_PX;

    let sizing = Sizing::new(expected_x, expected_y, expected_width, expected_height);
    log_actual_vs_expected(handle, &sizing, rect);
    debug!(
      "{} {} near-maximized (tolerance: {})",
      handle,
      if result { "is currently" } else { "is currently NOT" },
      TOLERANCE_IN_PX
    );

    result
  }

  fn near_maximize_window(&self, handle: WindowHandle, monitor_info: MonitorInfo, margin: i32) {
    info!("Near-maximizing {}", handle);

    // First maximize to get the animation effect
    self.windows_api.do_maximise_window(handle);

    // Then resize the window to the expected size
    let work_area = monitor_info.work_area;
    let sizing = Sizing::near_maximised(work_area, margin);
    self.execute_window_resizing(handle, sizing);
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
      if !self
        .windows_api
        .is_window_on_current_desktop(virtual_desktop_manager, window)?
      {
        continue;
      }

      let target_center_x = window.rect.left + (window.rect.right - window.rect.left) / 2;
      let target_center_y = window.rect.top + (window.rect.bottom - window.rect.top) / 2;
      let dx = target_center_x as i64 - reference_point.x() as i64;
      let dy = target_center_y as i64 - reference_point.y() as i64;

      // Skip windows that are not in the right direction, unless it has the same center as the reference window, if
      // the relevant configuration is enabled (see README for more information)
      let is_config_enabled = self
        .configuration_provider
        .lock()
        .expect(CONFIGURATION_PROVIDER_LOCK)
        .get_bool(ALLOW_SELECTING_SAME_CENTER_WINDOWS);
      if !is_config_enabled || reference_window?.center != window.center || reference_window?.handle == window.handle {
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
    let placement = WindowPlacement::new_from_sizing(sizing);
    self.windows_api.set_window_placement_and_force_repaint(handle, placement);
  }

  /// Returns the window under the cursor, if any. If there are multiple windows under the cursor, the foreground window
  /// is returned if it's in the list. Otherwise, the closest window is returned.
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
      if let Some(foreground_window) = self.windows_api.get_foreground_window() {
        if let Some(window_info) = windows_under_cursor
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

fn is_of_expected_size(handle: WindowHandle, placement: &WindowPlacement, sizing: &Sizing) -> bool {
  let rect = placement.normal_position;
  let result = rect.left == sizing.x
    && rect.top == sizing.y
    && rect.right - rect.left == sizing.width
    && rect.bottom - rect.top == sizing.height;

  log_actual_vs_expected(&handle, sizing, rect);
  debug!(
    "{} {} of expected size (tolerance: {})",
    handle,
    if result { "is currently" } else { "is currently NOT" },
    TOLERANCE_IN_PX
  );

  result
}

fn log_actual_vs_expected(handle: &WindowHandle, sizing: &Sizing, rc: Rect) {
  trace!(
    "Expected size of {}: ({},{})x({},{})",
    handle, sizing.x, sizing.y, sizing.width, sizing.height
  );
  trace!(
    "Actual size of {}: ({},{})x({},{})",
    handle,
    rc.left,
    rc.top,
    rc.right - rc.left,
    rc.bottom - rc.top
  );
}

#[cfg(test)]
mod tests {
  use crate::api::{MockWindowsApi, WindowsApi};
  use crate::common::{Direction, MonitorHandle, Point, Rect, Sizing, WindowHandle, WindowPlacement};
  use crate::configuration_provider::ConfigurationProvider;
  use crate::window_manager::{WindowManager, is_of_expected_size};
  use crate::workspace_manager::WorkspaceManager;
  use std::sync::{Arc, Mutex};

  impl WindowManager<MockWindowsApi> {
    pub fn default(api: MockWindowsApi) -> Self {
      WindowManager {
        configuration_provider: Arc::new(Mutex::new(ConfigurationProvider::default())),
        known_windows: Default::default(),
        workspace_manager: WorkspaceManager::default(),
        virtual_desktop_manager: None,
        windows_api: api,
      }
    }
  }

  #[test]
  fn is_of_expected_size_test() {
    let handle = WindowHandle::new(1);
    let placement = WindowPlacement::new_from_sizing(Sizing::new(0, 0, 100, 100));
    let sizing = Sizing::new(0, 0, 100, 100);
    assert!(is_of_expected_size(handle, &placement, &sizing));

    let placement = WindowPlacement::new_from_sizing(Sizing::new(1, 0, 101, 100));
    assert!(!is_of_expected_size(handle, &placement, &sizing));
  }

  #[test]
  fn near_maximize_window_when_window_is_not_near_maximised() {
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
  fn move_cursor_does_nothing_when_there_is_no_window_or_monitor_to_move_to() {
    let monitor_handle = MonitorHandle::from(1);
    let window_handle_1 = WindowHandle::new(1);
    let left_sizing = Sizing::left_half_of_screen(Rect::new(0, 0, 200, 180), 20);
    let initial_cursor_position = Point::new(0, 0);
    MockWindowsApi::set_cursor_position(initial_cursor_position);
    MockWindowsApi::add_or_update_window(window_handle_1, "Test".to_string(), left_sizing.clone(), false, false, true);
    MockWindowsApi::add_monitor(monitor_handle, Rect::new(0, 0, 200, 200), true);
    MockWindowsApi::place_window(window_handle_1, monitor_handle);
    let mut manager = WindowManager::default(MockWindowsApi);

    manager.move_cursor(Direction::Right);

    assert_eq!(manager.windows_api.get_cursor_position(), initial_cursor_position);
  }

  #[test]
  fn close_window() {
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
}
