#[allow(unused_variables)]
#[cfg(test)]
pub(crate) mod test {
  use crate::api::WindowsApi;
  use crate::utils::{Monitor, MonitorInfo, Monitors, Point, Rect, Sizing, Window, WindowPlacement};
  use crate::utils::{MonitorHandle, WindowHandle};
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
    /// less than the `monitor_area`.
    pub fn add_or_update_monitor(monitor_handle: MonitorHandle, monitor_area: Rect, is_primary: bool) {
      MOCK_STATE.with(|state| {
        let mut state = state.borrow_mut();
        let work_area_bottom = monitor_area.bottom - 20;
        let monitor = Monitor {
          handle: monitor_handle,
          size: 0,
          is_primary,
          work_area: Rect::new(monitor_area.left, monitor_area.top, monitor_area.right, work_area_bottom),
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
      MOCK_STATE.with(|state| {
        state.borrow_mut().foreground_window = Some(handle);
      });
    }

    pub fn set_cursor_position(position: Point) {
      MOCK_STATE.with(|state| {
        state.borrow_mut().cursor_position = position;
      });
    }

    #[allow(dead_code)]
    pub fn reset() {
      MOCK_STATE.with(|state| {
        *state.borrow_mut() = MockState::default();
      });
    }
  }

  impl WindowsApi for MockWindowsApi {
    fn get_foreground_window(&self) -> Option<WindowHandle> {
      MOCK_STATE.with(|state| state.borrow_mut().foreground_window)
    }

    fn set_foreground_window(&self, handle: WindowHandle) {
      MOCK_STATE.with(|state| {
        state.borrow_mut().foreground_window = Some(handle);
      });
    }

    fn get_all_visible_windows(&self) -> Vec<Window> {
      MOCK_STATE.with(|state| {
        state
          .borrow()
          .windows
          .values()
          .filter(|ws| !ws.is_hidden && !ws.is_closed)
          .map(|ws| ws.window.clone())
          .collect()
      })
    }

    fn get_all_visible_windows_within_area(&self, rect: Rect) -> Vec<Window> {
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
      unimplemented!()
    }

    fn is_window_minimised(&self, handle: WindowHandle) -> bool {
      MOCK_STATE.with(|state| {
        if let Some(window_state) = state.borrow_mut().windows.get(&handle) {
          return window_state.is_minimised;
        }
        panic!("Window with handle {handle} not found");
      })
    }

    fn is_not_a_managed_window(&self, handle: &WindowHandle) -> bool {
      unimplemented!()
    }

    fn is_window_hidden(&self, handle: &WindowHandle) -> bool {
      MOCK_STATE.with(|state| {
        if let Some(window_state) = state.borrow_mut().windows.get(handle) {
          return window_state.is_hidden;
        }
        false
      })
    }

    fn set_window_position(&self, handle: WindowHandle, rect: Rect) {
      MOCK_STATE.with(|state| {
        if let Some(window_state) = state.borrow_mut().windows.get_mut(&handle) {
          window_state.window_placement = WindowPlacement::new_from_rect(rect);
          window_state.window.rect = rect;
        }
      });
    }

    fn do_restore_window(&self, window: &Window, is_minimised: &bool) {
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

    fn do_hide_window(&self, handle: WindowHandle) {
      MOCK_STATE.with(|state| {
        if let Some(window_state) = state.borrow_mut().windows.get_mut(&handle) {
          window_state.is_hidden = true;
        } else {
          panic!("Window with handle {handle} not found - did you forget to add it?");
        }
      });
    }

    fn do_close_window(&self, handle: WindowHandle) {
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
        let monitor_handle = self.get_monitor_for_window_handle(handle);
        if let Some(windows) = state.borrow_mut().monitor_windows.get_mut(&monitor_handle) {
          windows.retain(|&w| w != handle);
        }
      });
      trace!("Mock windows API closed window {handle}");
    }

    fn get_window_placement(&self, handle: WindowHandle) -> Option<WindowPlacement> {
      MOCK_STATE.with(|state| state.borrow().windows.get(&handle).map(|w| w.window_placement.clone()))
    }

    fn set_window_placement_and_force_repaint(&self, handle: WindowHandle, placement: WindowPlacement) {
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
      MOCK_STATE.with(|state| state.borrow().cursor_position)
    }

    fn set_cursor_position(&self, target_point: &Point) {
      MOCK_STATE.with(|state| {
        state.borrow_mut().cursor_position = *target_point;
      });
    }

    fn get_all_monitors(&self) -> Monitors {
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
      MOCK_STATE.with(|state| {
        let monitor_handle = self.get_monitor_for_window_handle(handle);
        if let Some(monitor_state) = state.borrow_mut().monitors.get(&monitor_handle) {
          return Some(monitor_state.monitor_info);
        }

        None
      })
    }

    fn get_monitor_info_for_monitor(&self, handle: MonitorHandle) -> Option<MonitorInfo> {
      MOCK_STATE.with(|state| {
        if let Some(monitor_info) = state.borrow_mut().monitors.get(&handle) {
          return Some(monitor_info.monitor_info);
        }

        None
      })
    }

    fn get_monitor_for_window_handle(&self, handle: WindowHandle) -> MonitorHandle {
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

    fn get_monitor_for_point(&self, point: &Point) -> MonitorHandle {
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
      unimplemented!()
    }

    fn is_window_on_current_desktop(&self, vdm: &IVirtualDesktopManager, window: &Window) -> Option<bool> {
      unimplemented!()
    }

    fn set_window_position_with_dpi_adjustment(
      &self,
      window_handle: WindowHandle,
      source_monitor_handle: MonitorHandle,
      target_monitor_handle: MonitorHandle,
      rect: Rect,
    ) {
      unimplemented!()
    }
  }
}
