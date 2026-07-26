#[allow(unused_variables)]
#[cfg(test)]
pub(crate) mod test {
  use crate::api::{WindowLookupError, WindowMetadata, WindowPositioningResult, WindowsApi};
  use crate::common::{
    Monitor, MonitorHandle, MonitorInfo, Monitors, Point, Rect, Sizing, Window, WindowHandle, WindowPlacement,
  };
  use std::cell::RefCell;
  use std::collections::{HashMap, HashSet};
  use windows::Win32::UI::Shell::IVirtualDesktopManager;

  thread_local! {
      static MOCK_STATE: RefCell<MockState> = RefCell::new(MockState::default());
  }

  #[derive(Default)]
  struct MockState {
    cursor_position: Point,
    windows: HashMap<WindowHandle, WindowState>,
    monitors: HashMap<MonitorHandle, MonitorState>,
    monitor_windows: HashMap<MonitorHandle, Vec<WindowHandle>>,
    foreground_window: Option<WindowHandle>,
    pointer_interaction_active: bool,
    position_batches: Vec<Vec<(WindowHandle, Rect)>>,
    deferred_positioning_failures: HashSet<WindowHandle>,
    deferred_positioning_attempts: HashMap<WindowHandle, usize>,
    window_position_minimum_dimensions: HashMap<WindowHandle, (i32, i32)>,
    point_targets: HashMap<Point, (WindowHandle, WindowHandle)>,
    picker_windows: HashSet<WindowHandle>,
  }

  struct WindowState {
    window: Window,
    class_name: String,
    window_placement: WindowPlacement,
    is_minimised: bool,
    is_hidden: bool,
    is_closed: bool,
    is_manageable: bool,
  }

  #[derive(Clone)]
  struct MonitorState {
    monitor: Monitor,
    monitor_info: MonitorInfo,
  }

  #[derive(Copy, Clone)]
  pub struct MockWindowsApi;

  impl MockWindowsApi {
    pub fn new() -> Self {
      Self {}
    }

    pub fn add_or_update_window(
      handle: WindowHandle,
      title: String,
      sizing: Sizing,
      is_minimised: bool,
      is_hidden: bool,
      is_foreground: bool,
    ) {
      Self::add_or_update_window_with_class(handle, title, String::new(), sizing, is_minimised, is_hidden, is_foreground);
    }

    pub fn add_or_update_window_with_class(
      handle: WindowHandle,
      title: String,
      class_name: String,
      sizing: Sizing,
      is_minimised: bool,
      is_hidden: bool,
      is_foreground: bool,
    ) {
      MOCK_STATE.with(|state| {
        let mut state = state.borrow_mut();
        let window = Window::new(handle.into(), title, sizing.clone().into());
        let window_placement = WindowPlacement::new_from_sizing(sizing);
        state.windows.insert(
          handle,
          WindowState {
            window,
            class_name,
            window_placement,
            is_minimised,
            is_hidden,
            is_closed: false,
            is_manageable: true,
          },
        );
        if is_foreground {
          state.foreground_window = Some(handle);
        }
      });
    }

    pub fn set_point_target(point: Point, hit: WindowHandle, root: WindowHandle) {
      MOCK_STATE.with(|state| {
        state.borrow_mut().point_targets.insert(point, (hit, root));
      });
    }

    pub fn mark_picker_window(handle: WindowHandle) {
      MOCK_STATE.with(|state| {
        state.borrow_mut().picker_windows.insert(handle);
      });
    }

    /// Adds or updates a monitor to the mock state, assuming that the height of the monitor's `work_area` is 20 pixels
    /// less than the `monitor_area` and using the `monitor_handle` as the ID.
    pub fn add_monitor(monitor_handle: MonitorHandle, monitor_area: Rect, is_primary: bool) {
      let work_area_bottom = monitor_area.bottom - 20;
      Self::add_monitor_with_full_details(
        [monitor_handle.handle as u16; 32],
        monitor_handle,
        monitor_area,
        Rect::new(monitor_area.left, monitor_area.top, monitor_area.right, work_area_bottom),
        is_primary,
      );
    }

    pub fn add_monitor_with_full_details(
      monitor_id: [u16; 32],
      monitor_handle: MonitorHandle,
      monitor_area: Rect,
      work_area: Rect,
      is_primary: bool,
    ) {
      trace!(
        "Mock windows API adds monitor {monitor_handle} - monitor_area: {monitor_area}, work_area: {work_area}, is_primary: {is_primary}"
      );
      MOCK_STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.monitors.contains_key(&monitor_handle) {
          panic!("Monitor with handle {monitor_handle} already exists");
        }
        let work_area_bottom = monitor_area.bottom - 20;
        let monitor = Monitor {
          id: monitor_id,
          handle: monitor_handle,
          size: 0,
          is_primary,
          work_area,
          monitor_area,
          center: Point::from_center_of_rect(&monitor_area),
        };
        let monitor_info = (&monitor).into();
        state.monitors.insert(monitor_handle, MonitorState { monitor, monitor_info });
      });
    }

    /// Adds a link between a window and a monitor, simulating the placement of the window on that monitor.
    /// This does not mean that the window is on the active workspace of the monitor or that it is active.
    pub fn place_window(window_handle: WindowHandle, monitor_handle: MonitorHandle) {
      trace!("Mock windows API places window {window_handle} on monitor {monitor_handle}");
      MOCK_STATE.with(|state| {
        state
          .borrow_mut()
          .monitor_windows
          .entry(monitor_handle)
          .or_default()
          .push(window_handle);
      });
    }

    pub fn assign_window_to_monitor(window_handle: WindowHandle, monitor_handle: MonitorHandle) {
      MOCK_STATE.with(|state| {
        let mut state = state.borrow_mut();
        for windows in state.monitor_windows.values_mut() {
          windows.retain(|handle| *handle != window_handle);
        }
        state.monitor_windows.entry(monitor_handle).or_default().push(window_handle);
      });
    }

    pub fn set_foreground_window(handle: WindowHandle) {
      trace!("Mock windows API sets foreground window {handle}");
      MOCK_STATE.with(|state| {
        state.borrow_mut().foreground_window = Some(handle);
      });
    }

    pub fn set_cursor_position(position: Point) {
      trace!("Mock windows API sets cursor position to {position}");
      MOCK_STATE.with(|state| {
        state.borrow_mut().cursor_position = position;
      });
    }

    pub fn set_pointer_interaction_active(active: bool) {
      MOCK_STATE.with(|state| state.borrow_mut().pointer_interaction_active = active);
    }

    /// Configures the minimum dimensions enforced during window positioning.
    pub fn set_window_position_minimum_dimensions(handle: WindowHandle, width: i32, height: i32) {
      MOCK_STATE.with(|state| {
        state
          .borrow_mut()
          .window_position_minimum_dimensions
          .insert(handle, (width, height));
      });
    }

    pub fn mark_window_unmanageable(handle: WindowHandle) {
      MOCK_STATE.with(|state| {
        if let Some(window) = state.borrow_mut().windows.get_mut(&handle) {
          window.is_manageable = false;
        }
      });
    }

    pub fn clear_position_batches() {
      MOCK_STATE.with(|state| state.borrow_mut().position_batches.clear());
    }

    pub fn position_batches() -> Vec<Vec<(WindowHandle, Rect)>> {
      MOCK_STATE.with(|state| state.borrow().position_batches.clone())
    }

    pub fn fail_deferred_positioning(handle: WindowHandle) {
      MOCK_STATE.with(|state| {
        state.borrow_mut().deferred_positioning_failures.insert(handle);
      });
    }

    pub fn deferred_positioning_attempts(handle: WindowHandle) -> usize {
      MOCK_STATE.with(|state| {
        state
          .borrow()
          .deferred_positioning_attempts
          .get(&handle)
          .copied()
          .unwrap_or_default()
      })
    }

    #[allow(dead_code)]
    pub fn reset() {
      trace!("Mock windows API resets state");
      MOCK_STATE.with(|state| {
        *state.borrow_mut() = MockState::default();
      });
    }
  }

  impl WindowsApi for MockWindowsApi {
    fn is_running_as_admin(&self) -> bool {
      trace!("Mock windows API checks if running as admin");
      true
    }

    fn get_raw_foreground_window(&self) -> Option<WindowHandle> {
      trace!("Mock windows API gets raw foreground window");
      MOCK_STATE.with(|state| state.borrow().foreground_window)
    }

    fn get_foreground_window(&self) -> Option<WindowHandle> {
      trace!("Mock windows API gets managed foreground window");
      self
        .get_raw_foreground_window()
        .filter(|handle| !self.is_not_a_managed_window(handle))
    }

    fn is_pointer_interaction_active(&self) -> bool {
      MOCK_STATE.with(|state| state.borrow().pointer_interaction_active)
    }

    fn set_foreground_window(&self, handle: WindowHandle) {
      trace!("Mock windows API sets foreground window {handle}");
      MOCK_STATE.with(|state| {
        state.borrow_mut().foreground_window = Some(handle);
      });
    }

    fn get_all_windows(&self) -> Vec<Window> {
      trace!("Mock windows API gets all windows");
      MOCK_STATE.with(|state| {
        state
          .borrow()
          .windows
          .values()
          .filter(|ws| !ws.is_closed && ws.is_manageable)
          .map(|ws| ws.window.clone())
          .collect()
      })
    }

    fn get_all_visible_windows(&self) -> Vec<Window> {
      trace!("Mock windows API gets all visible windows");
      MOCK_STATE.with(|state| {
        state
          .borrow()
          .windows
          .values()
          .filter(|ws| !ws.is_hidden && !ws.is_closed && !ws.is_minimised && ws.is_manageable)
          .map(|ws| ws.window.clone())
          .collect()
      })
    }

    fn get_all_visible_windows_within_area(&self, rect: Rect) -> Vec<Window> {
      trace!("Mock windows API gets all visible windows within {rect}");
      MOCK_STATE.with(|state| {
        state
          .borrow()
          .windows
          .iter()
          .filter_map(|(_, ws)| {
            if ws.window.rect.intersects(&rect) && !ws.is_hidden && ws.is_manageable {
              Some(ws.window.clone())
            } else {
              None
            }
          })
          .collect()
      })
    }

    fn get_window_title(&self, handle: &WindowHandle) -> String {
      trace!("Mock windows API gets window title for {handle}");
      MOCK_STATE.with(|state| {
        state.borrow().windows.get(handle).map_or_else(
          || {
            panic!("Window with handle {handle} not found");
          },
          |window_state| window_state.window.title.clone(),
        )
      })
    }

    fn get_window_class_name(&self, handle: &WindowHandle) -> String {
      trace!("Mock windows API gets window class name for {handle}");
      MOCK_STATE.with(|state| {
        state
          .borrow()
          .windows
          .get(handle)
          .map(|window| window.class_name.clone())
          .unwrap_or_default()
      })
    }

    fn get_window_at_point(&self, point: Point) -> Result<WindowMetadata, WindowLookupError> {
      MOCK_STATE.with(|state| {
        let state = state.borrow();
        let (_, root) = state.point_targets.get(&point).ok_or(WindowLookupError::NoTarget)?;
        if state.picker_windows.contains(root) {
          return Err(WindowLookupError::OwnWindow);
        }
        let window = state.windows.get(root).ok_or(WindowLookupError::Vanished)?;
        if window.is_closed {
          return Err(WindowLookupError::Vanished);
        }
        Ok(WindowMetadata {
          handle: *root,
          title: window.window.title.clone(),
          class_name: window.class_name.clone(),
          rect: window.window.rect,
        })
      })
    }

    fn get_window_rect(&self, handle: WindowHandle) -> Option<Rect> {
      trace!("Mock windows API gets window rect for {handle}");
      MOCK_STATE.with(|state| state.borrow().windows.get(&handle).map(|ws| ws.window.rect))
    }

    fn get_extended_frame_bounds(&self, handle: WindowHandle) -> Option<Rect> {
      trace!("Mock windows API gets extended frame bounds for {handle}");
      MOCK_STATE.with(|state| state.borrow().windows.get(&handle).map(|ws| ws.window.rect))
    }

    fn is_window_minimised(&self, handle: WindowHandle) -> bool {
      trace!("Mock windows API checks if window {handle} is minimised");
      MOCK_STATE.with(|state| {
        if let Some(window_state) = state.borrow_mut().windows.get(&handle) {
          return window_state.is_minimised;
        }
        panic!("Window with handle {handle} not found");
      })
    }

    fn is_not_a_managed_window(&self, handle: &WindowHandle) -> bool {
      trace!("Mock windows API checks if window {handle} is not a managed window");
      MOCK_STATE.with(|state| state.borrow().windows.get(handle).is_none_or(|window| !window.is_manageable))
    }

    fn is_window_hidden(&self, handle: &WindowHandle) -> bool {
      trace!("Mock windows API checks if window {handle} is hidden");
      MOCK_STATE.with(|state| {
        if let Some(window_state) = state.borrow_mut().windows.get(handle) {
          return window_state.is_hidden;
        }
        false
      })
    }

    fn set_window_position(&self, handle: WindowHandle, mut rect: Rect) {
      trace!("Mock windows API sets window position for {handle} to {rect}");
      MOCK_STATE.with(|state| {
        let mut state = state.borrow_mut();
        if let Some((minimum_width, minimum_height)) = state.window_position_minimum_dimensions.get(&handle).copied() {
          rect.right = rect.left + rect.width().max(minimum_width);
          rect.bottom = rect.top + rect.height().max(minimum_height);
        }
        if let Some(window_state) = state.windows.get_mut(&handle) {
          window_state.window_placement = WindowPlacement::new_from_rect(rect);
          window_state.window.rect = rect;
        }
      });
    }

    fn set_window_positions(&self, positions: &[(WindowHandle, Rect)], focused: WindowHandle) -> WindowPositioningResult {
      trace!("Mock windows API atomically positions [{}] windows", positions.len());
      MOCK_STATE.with(|state| {
        let mut state = state.borrow_mut();
        for (handle, _) in positions {
          *state.deferred_positioning_attempts.entry(*handle).or_default() += 1;
        }
        let failures = positions
          .iter()
          .filter_map(|(handle, _)| state.deferred_positioning_failures.contains(handle).then_some(*handle))
          .collect::<Vec<_>>();
        if !failures.is_empty() {
          return WindowPositioningResult::Rejected(failures);
        }
        let mut ordered = Vec::with_capacity(positions.len());
        if let Some(position) = positions.iter().find(|(handle, _)| *handle == focused) {
          ordered.push(*position);
        }
        ordered.extend(positions.iter().copied().filter(|(handle, _)| *handle != focused));
        for (handle, rect) in &ordered {
          if let Some(window_state) = state.windows.get_mut(handle) {
            window_state.window_placement = WindowPlacement::new_from_rect(*rect);
            window_state.window.rect = *rect;
            window_state.window.center = Point::from_center_of_rect(rect);
          }
        }
        state.position_batches.push(ordered);
        WindowPositioningResult::Applied
      })
    }

    fn set_window_position_with_dpi_adjustment(
      &self,
      window_handle: WindowHandle,
      source_monitor_handle: MonitorHandle,
      target_monitor_handle: MonitorHandle,
      rect: Rect,
    ) {
      trace!(
        "Mock windows API sets window position for {window_handle} to {rect} with DPI adjustment from {source_monitor_handle} to {target_monitor_handle}"
      );
      unimplemented!()
    }

    fn do_restore_window(&self, window: &Window, is_minimised: &bool) {
      trace!("Mock windows API restores window {}", window.handle);
      MOCK_STATE.with(|state| {
        if let Some(window_state) = state.borrow_mut().windows.get_mut(&window.handle) {
          window_state.is_minimised = *is_minimised;
          window_state.is_hidden = false;
          window_state.window_placement.normal_position = window.rect;
          window_state.window.rect = window.rect;
        } else {
          panic!("Window with handle {} not found", window.handle);
        }
      });
    }

    fn do_maximise_window(&self, handle: WindowHandle) {
      trace!("Mock windows API maximises window {handle}");
      let monitor_handle = self.get_monitor_handle_for_window_handle(handle);
      let monitor_info = self
        .get_monitor_info_for_monitor(monitor_handle)
        .unwrap_or_else(|| panic!("Monitor info for monitor {monitor_handle} not found"));

      MOCK_STATE.with(|state| {
        let mut ref_mut = state.borrow_mut();
        if let Some(window_state) = ref_mut.windows.get_mut(&handle) {
          let placement = WindowPlacement::new_from_rect(monitor_info.work_area);
          window_state.is_minimised = false;
          window_state.is_hidden = false;
          window_state.is_closed = false;
          window_state.window.rect = placement.normal_position;
          window_state.window_placement = placement;
          window_state.window.center = Point::from_center_of_rect(&window_state.window.rect);
          ref_mut.foreground_window = Some(handle);
        } else {
          panic!("Window with handle {handle} not found - did you forget to add it?");
        }
      });
    }

    fn do_minimise_window(&self, handle: WindowHandle) {
      trace!("Mock windows API minimises window {handle}");
      MOCK_STATE.with(|state| {
        if let Some(window_state) = state.borrow_mut().windows.get_mut(&handle) {
          if window_state.is_hidden {
            panic!("Window with handle {handle} is hidden and cannot be minimised");
          }
          window_state.is_minimised = true;
        } else {
          panic!("Window with handle {handle} not found - did you forget to add it?");
        }
        state.borrow_mut().foreground_window = None;
      });
    }

    fn do_hide_window(&self, handle: WindowHandle) {
      trace!("Mock windows API hides window {handle}");
      MOCK_STATE.with(|state| {
        if let Some(window_state) = state.borrow_mut().windows.get_mut(&handle) {
          window_state.is_hidden = true;
        } else {
          panic!("Window with handle {handle} not found - did you forget to add it?");
        }
        if state.borrow().foreground_window == Some(handle) {
          state.borrow_mut().foreground_window = None;
        }
      });
    }

    fn do_unhide_window(&self, handle: WindowHandle) {
      trace!("Mock windows API unhides window {handle}");
      MOCK_STATE.with(|state| {
        if let Some(window_state) = state.borrow_mut().windows.get_mut(&handle) {
          window_state.is_hidden = false;
        } else {
          panic!("Window with handle {handle} not found - did you forget to add it?");
        }
        state.borrow_mut().foreground_window = Some(handle);
      });
    }

    fn do_close_window(&self, handle: WindowHandle) {
      trace!("Mock windows API closes window {handle}");
      MOCK_STATE.with(|state| {
        if let Some(window_state) = state.borrow_mut().windows.get_mut(&handle) {
          window_state.is_closed = true;
          window_state.is_hidden = true;
        } else {
          panic!("Window with handle {handle} not found - did you forget to add it?");
        }
        let is_foreground = state.borrow().foreground_window == Some(handle);
        if is_foreground {
          state.borrow_mut().foreground_window = None;
        }
        let monitor_handle = self.get_monitor_handle_for_window_handle(handle);
        if let Some(windows) = state.borrow_mut().monitor_windows.get_mut(&monitor_handle) {
          windows.retain(|&w| w != handle);
        }
      });
      trace!("Mock windows API closed window {handle}");
    }

    fn get_window_placement(&self, handle: WindowHandle) -> Option<WindowPlacement> {
      trace!("Mock windows API gets window placement for {handle}");
      MOCK_STATE.with(|state| state.borrow().windows.get(&handle).map(|w| w.window_placement.clone()))
    }

    fn get_minimum_window_dimensions(&self, handle: WindowHandle) -> Option<(i32, i32)> {
      trace!("Mock windows API gets minimum window dimensions for {handle}");
      MOCK_STATE.with(|state| state.borrow().window_position_minimum_dimensions.get(&handle).copied())
    }

    fn set_window_placement_and_force_repaint(&self, handle: WindowHandle, mut placement: WindowPlacement) {
      trace!("Mock windows API sets window placement for {handle} - {placement:?}");
      MOCK_STATE.with(|state| {
        let mut state = state.borrow_mut();
        if let Some((minimum_width, minimum_height)) = state.window_position_minimum_dimensions.get(&handle).copied() {
          let rect = &mut placement.normal_position;
          rect.right = rect.left + rect.width().max(minimum_width);
          rect.bottom = rect.top + rect.height().max(minimum_height);
        }
        let Some(window_state) = state.windows.get_mut(&handle).map(|window_state| {
          window_state.window.rect = placement.normal_position;
          window_state.window.center = Point::from_center_of_rect(&placement.normal_position);
          window_state.window_placement = placement;
        }) else {
          panic!("Window with handle {handle} not found");
        };
      });
    }

    fn do_restore_window_placement(&self, handle: WindowHandle, previous_placement: WindowPlacement) {
      trace!("Mock windows API restores window placement for {handle}");
      MOCK_STATE.with(|state| {
        if let Some(window_state) = state.borrow_mut().windows.get_mut(&handle).or_else(|| {
          panic!("Window with handle {handle} not found");
        }) {
          window_state.window_placement = previous_placement.clone();
          window_state.window.rect = previous_placement.normal_position;
        }
      })
    }

    fn get_cursor_position(&self) -> Point {
      trace!("Mock windows API gets cursor position");
      MOCK_STATE.with(|state| state.borrow().cursor_position)
    }

    fn set_cursor_position(&self, target_point: &Point) {
      trace!("Mock windows API sets cursor position to {target_point}");
      MOCK_STATE.with(|state| {
        state.borrow_mut().cursor_position = *target_point;
      });
    }

    fn get_all_monitors(&self) -> Monitors {
      trace!("Mock windows API gets all monitors");
      MOCK_STATE.with(|state| {
        let monitors = state
          .borrow()
          .monitors
          .values()
          .cloned()
          .map(|monitor_state| monitor_state.monitor)
          .collect::<Vec<Monitor>>();

        Monitors::from(monitors)
      })
    }

    fn get_monitor_info_for_window(&self, handle: WindowHandle) -> Option<MonitorInfo> {
      trace!("Mock windows API gets monitor info for window {handle}");
      MOCK_STATE.with(|state| {
        let monitor_handle = self.get_monitor_handle_for_window_handle(handle);
        if let Some(monitor_state) = state.borrow_mut().monitors.get(&monitor_handle) {
          return Some(monitor_state.monitor_info);
        }

        None
      })
    }

    fn get_monitor_info_for_monitor(&self, handle: MonitorHandle) -> Option<MonitorInfo> {
      trace!("Mock windows API gets monitor info for monitor {handle}");
      MOCK_STATE.with(|state| {
        if let Some(monitor_info) = state.borrow_mut().monitors.get(&handle) {
          return Some(monitor_info.monitor_info);
        }

        None
      })
    }

    fn get_monitor_id_for_handle(&self, handle: MonitorHandle) -> Option<[u16; 32]> {
      trace!("Mock windows API gets monitor id for handle {handle}");
      MOCK_STATE.with(|state| {
        state
          .borrow()
          .monitors
          .get(&handle)
          .map(|monitor_state| monitor_state.monitor.id)
      })
    }

    fn get_monitor_handle_for_window_handle(&self, handle: WindowHandle) -> MonitorHandle {
      trace!("Mock windows API gets monitor for window {handle}");
      MOCK_STATE.with(|state| {
        if let Some((monitor_handle, _)) = state
          .borrow_mut()
          .monitor_windows
          .iter()
          .find(|(_, windows)| windows.contains(&handle))
        {
          return *monitor_handle;
        }
        panic!("You forgot to set a monitor for for window {}", handle);
      })
    }

    fn get_monitor_handle_for_point(&self, point: &Point) -> MonitorHandle {
      trace!("Mock windows API gets monitor for point {point:?}");
      MOCK_STATE.with(|state| {
        state
          .borrow()
          .monitors
          .iter()
          .find(|(_, ms)| ms.monitor_info.monitor_area.contains(point))
          .map(|(handle, _)| *handle)
          .expect("Unable to find monitor for point")
      })
    }

    fn get_virtual_desktop_manager(&self) -> Option<IVirtualDesktopManager> {
      trace!("Mock windows API gets virtual desktop manager");
      unimplemented!()
    }

    fn is_window_on_current_desktop(&self, vdm: &IVirtualDesktopManager, window: &Window) -> bool {
      trace!("Mock windows API checks if window {} is on current desktop", window.handle);
      unimplemented!()
    }
  }

  #[test]
  fn point_hit_returns_frozen_top_level_window_metadata() {
    MockWindowsApi::reset();
    let point = Point::new(40, 60);
    let child = WindowHandle::new(41);
    let root = WindowHandle::new(42);
    MockWindowsApi::add_or_update_window_with_class(
      root,
      "Résumé — 東京".to_string(),
      "EditorWindow".to_string(),
      Sizing::new(10, 20, 300, 200),
      false,
      false,
      false,
    );
    MockWindowsApi::set_point_target(point, child, root);

    let metadata = MockWindowsApi::new().get_window_at_point(point).unwrap();

    assert_eq!(metadata.handle, root);
    assert_eq!(metadata.title, "Résumé — 東京");
    assert_eq!(metadata.class_name, "EditorWindow");
    assert_eq!(metadata.rect, Rect::new(10, 20, 310, 220));
  }

  #[test]
  fn point_hit_rejects_the_window_picker_ui() {
    MockWindowsApi::reset();
    let point = Point::new(10, 10);
    let picker = WindowHandle::new(7);
    MockWindowsApi::add_or_update_window(
      picker,
      "Window Picker".to_string(),
      Sizing::new(0, 0, 100, 100),
      false,
      false,
      false,
    );
    MockWindowsApi::set_point_target(point, picker, picker);
    MockWindowsApi::mark_picker_window(picker);

    assert_eq!(
      MockWindowsApi::new().get_window_at_point(point),
      Err(WindowLookupError::OwnWindow)
    );
  }

  #[test]
  fn point_hit_bypasses_manageability_filter() {
    MockWindowsApi::reset();
    let point = Point::new(20, 20);
    let handle = WindowHandle::new(8);
    MockWindowsApi::add_or_update_window(
      handle,
      "Excluded".to_string(),
      Sizing::new(0, 0, 100, 100),
      false,
      false,
      false,
    );
    MockWindowsApi::mark_window_unmanageable(handle);
    MockWindowsApi::set_point_target(point, handle, handle);

    assert_eq!(MockWindowsApi::new().get_window_at_point(point).unwrap().handle, handle);
  }

  #[test]
  fn point_without_target_returns_typed_absence() {
    MockWindowsApi::reset();

    assert_eq!(
      MockWindowsApi::new().get_window_at_point(Point::new(-1, -1)),
      Err(WindowLookupError::NoTarget)
    );
  }

  #[test]
  fn point_hit_to_missing_root_returns_vanished_error() {
    MockWindowsApi::reset();
    let point = Point::new(30, 30);
    let missing = WindowHandle::new(9);
    MockWindowsApi::set_point_target(point, missing, missing);

    assert_eq!(
      MockWindowsApi::new().get_window_at_point(point),
      Err(WindowLookupError::Vanished)
    );
  }
}
