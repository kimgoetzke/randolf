#[allow(unused_variables)]
#[cfg(test)]
pub(crate) mod test {

  use crate::api::NativeApi;
  use crate::utils::{Monitor, MonitorInfo, Monitors, Point, Rect, Window, WindowHandle, WindowPlacement};
  use std::cell::RefCell;
  use std::collections::HashMap;
  use windows::Win32::UI::Shell::IVirtualDesktopManager;

  thread_local! {
      static MOCK_STATE: RefCell<MockState> = RefCell::new(MockState::default());
  }

  #[derive(Default, Clone)]
  struct MockState {
    foreground_window: Option<WindowHandle>,
    window_placement: Option<WindowPlacement>,
    window_title: String,
    monitors: Vec<Monitor>,
    visible_windows: Vec<Window>,
    cursor_position: Point,
    monitor_for_point: isize,
    hidden_windows: HashMap<WindowHandle, bool>,
  }

  #[derive(Copy, Clone)]
  pub struct MockWindowsApi;

  impl MockWindowsApi {
    pub fn new() -> Self {
      Self {}
    }

    pub fn set_foreground_window(handle: WindowHandle) {
      MOCK_STATE.with(|state| {
        state.borrow_mut().foreground_window = Some(handle);
      });
    }

    pub fn set_window_title(title: String) {
      MOCK_STATE.with(|state| {
        state.borrow_mut().window_title = title;
      });
    }

    pub fn set_visible_windows(windows: Vec<Window>) {
      MOCK_STATE.with(|state| {
        state.borrow_mut().visible_windows = windows;
      });
    }

    pub fn set_is_window_hidden(handle: WindowHandle, is_hidden: bool) {
      MOCK_STATE.with(|state| {
        state.borrow_mut().hidden_windows.insert(handle, is_hidden);
      });
    }

    pub fn set_window_placement(placement: Option<WindowPlacement>) {
      MOCK_STATE.with(|state| {
        state.borrow_mut().window_placement = placement;
      });
    }

    pub fn set_monitors(monitors: Vec<Monitor>) {
      MOCK_STATE.with(|state| {
        state.borrow_mut().monitors = monitors;
      });
    }

    pub fn set_cursor_position(position: Point) {
      MOCK_STATE.with(|state| {
        state.borrow_mut().cursor_position = position;
      });
    }

    pub fn set_monitor_for_point(handle: isize) {
      MOCK_STATE.with(|state| {
        state.borrow_mut().monitor_for_point = handle;
      });
    }

    // Helper method to reset all mock data
    pub fn reset() {
      MOCK_STATE.with(|state| {
        *state.borrow_mut() = MockState::default();
      });
    }
  }

  impl NativeApi for MockWindowsApi {
    fn get_foreground_window(&self) -> Option<WindowHandle> {
      MOCK_STATE.with(|state| state.borrow_mut().foreground_window)
    }

    fn set_foreground_window(&self, handle: WindowHandle) {
      MOCK_STATE.with(|state| {
        state.borrow_mut().foreground_window = Some(handle);
      });
    }

    fn get_all_visible_windows(&self) -> Vec<Window> {
      MOCK_STATE.with(|state| state.borrow().visible_windows.clone())
    }

    fn get_all_visible_windows_within_area(&self, rect: Rect) -> Vec<Window> {
      MOCK_STATE.with(|state| state.borrow().visible_windows.clone())
    }

    fn get_window_title(&self, handle: &WindowHandle) -> String {
      MOCK_STATE.with(|state| state.borrow().window_title.clone())
    }

    fn get_window_class_name(&self, handle: &WindowHandle) -> String {
      unimplemented!()
    }

    fn is_window_minimised(&self, handle: WindowHandle) -> bool {
      false
    }

    fn is_not_a_managed_window(&self, handle: &WindowHandle) -> bool {
      unimplemented!()
    }

    fn is_window_hidden(&self, handle: &WindowHandle) -> bool {
      MOCK_STATE.with(|state| {
        if let Some(window) = state.borrow_mut().hidden_windows.get(handle) {
          return *window;
        }
        false
      })
    }

    fn set_window_position(&self, handle: WindowHandle, rect: Rect) {
      unimplemented!()
    }

    fn do_restore_window(&self, window: &Window, is_minimised: &bool) {
      unimplemented!()
    }

    fn do_maximise_window(&self, handle: WindowHandle) {
      unimplemented!()
    }

    fn do_hide_window(&self, handle: WindowHandle) {
      info!("Hiding window: {handle}");
    }

    fn do_close_window(&self, handle: WindowHandle) {
      unimplemented!()
    }

    fn get_window_placement(&self, handle: WindowHandle) -> Option<WindowPlacement> {
      MOCK_STATE.with(|state| state.borrow().window_placement.clone())
    }

    fn set_window_placement_and_force_repaint(&self, handle: WindowHandle, placement: WindowPlacement) {
      unimplemented!()
    }

    fn do_restore_window_placement(&self, handle: WindowHandle, previous_placement: WindowPlacement) {
      unimplemented!()
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
      unimplemented!()
    }

    fn get_monitor_info_from_window(&self, handle: WindowHandle) -> Option<MonitorInfo> {
      unimplemented!()
    }

    fn get_monitor_for_window_handle(&self, handle: WindowHandle) -> isize {
      unimplemented!()
    }

    fn get_monitor_for_point(&self, point: &Point) -> isize {
      MOCK_STATE.with(|state| state.borrow().monitor_for_point)
    }

    fn get_virtual_desktop_manager(&self) -> Option<IVirtualDesktopManager> {
      unimplemented!()
    }

    fn is_window_on_current_desktop(&self, vdm: &IVirtualDesktopManager, window: &Window) -> Option<bool> {
      unimplemented!()
    }
  }
}
