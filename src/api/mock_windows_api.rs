#[allow(unused_variables)]
#[cfg(test)]
pub(crate) mod test {
  use crate::api::WindowsApi;
  use crate::common::{
    Monitor, MonitorHandle, MonitorInfo, Monitors, Point, Rect, Sizing, Window, WindowHandle, WindowPlacement,
  };
  use std::cell::RefCell;
  use std::collections::HashMap;
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
  }

  struct WindowState {
    window: Window,
    window_placement: WindowPlacement,
    is_minimised: bool,
    is_hidden: bool,
    is_closed: bool,
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
      MOCK_STATE.with(|state| {
        let mut state = state.borrow_mut();
        let window = Window::new(handle.into(), title, sizing.clone().into());
        let window_placement = WindowPlacement::new_from_sizing(sizing);
        state.windows.insert(
          handle,
          WindowState {
            window,
            window_placement,
            is_minimised,
            is_hidden,
            is_closed: false,
          },
        );
        if is_foreground {
          state.foreground_window = Some(handle);
        }
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

    fn get_foreground_window(&self) -> Option<WindowHandle> {
      trace!("Mock windows API gets foreground window");
      MOCK_STATE.with(|state| state.borrow_mut().foreground_window)
    }

    fn set_foreground_window(&self, handle: WindowHandle) {
      trace!("Mock windows API sets foreground window {handle}");
      MOCK_STATE.with(|state| {
        state.borrow_mut().foreground_window = Some(handle);
      });
    }

    fn get_all_visible_windows(&self) -> Vec<Window> {
      trace!("Mock windows API gets all visible windows");
      MOCK_STATE.with(|state| {
        state
          .borrow()
          .windows
          .values()
          .filter(|ws| !ws.is_hidden && !ws.is_closed && !ws.is_minimised)
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
            if ws.window.rect.intersects(&rect) && !ws.is_hidden {
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
      unimplemented!()
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
      unimplemented!()
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

    fn set_window_position(&self, handle: WindowHandle, rect: Rect) {
      trace!("Mock windows API sets window position for {handle} to {rect}");
      MOCK_STATE.with(|state| {
        if let Some(window_state) = state.borrow_mut().windows.get_mut(&handle) {
          window_state.window_placement = WindowPlacement::new_from_rect(rect);
          window_state.window.rect = rect;
        }
      });
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
      trace!("Mock windows API maximises window {handle} - not implemented yet");
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

    fn set_window_placement_and_force_repaint(&self, handle: WindowHandle, placement: WindowPlacement) {
      trace!("Mock windows API sets window placement for {handle} - {placement:?}");
      MOCK_STATE.with(|state| {
        let Some(window_state) = state.borrow_mut().windows.get_mut(&handle).map(|window_state| {
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

    fn is_window_on_current_desktop(&self, vdm: &IVirtualDesktopManager, window: &Window) -> Option<bool> {
      trace!("Mock windows API checks if window {} is on current desktop", window.handle);
      unimplemented!()
    }
  }
}
