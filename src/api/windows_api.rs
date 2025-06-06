use crate::common::{MonitorHandle, MonitorInfo, Monitors, Point, Rect, Window, WindowHandle, WindowPlacement};
use windows::Win32::UI::Shell::IVirtualDesktopManager;

pub trait WindowsApi {
  fn is_running_as_admin(&self) -> bool;
  fn get_foreground_window(&self) -> Option<WindowHandle>;
  fn set_foreground_window(&self, handle: WindowHandle);
  fn get_all_windows(&self) -> Vec<Window>;
  fn get_all_visible_windows(&self) -> Vec<Window>;
  fn get_all_visible_windows_within_area(&self, rect: Rect) -> Vec<Window>;
  fn get_window_title(&self, handle: &WindowHandle) -> String;
  fn get_window_class_name(&self, handle: &WindowHandle) -> String;
  fn is_window_minimised(&self, handle: WindowHandle) -> bool;
  fn is_not_a_managed_window(&self, handle: &WindowHandle) -> bool;
  fn is_window_hidden(&self, handle: &WindowHandle) -> bool;
  fn set_window_position(&self, handle: WindowHandle, rect: Rect);
  /// Sets the window position on the same monitor as the given rectangle. WARNING: Does not adjust for DPI scaling.
  #[allow(dead_code)]
  fn set_window_position_with_dpi_adjustment(
    &self,
    window_handle: WindowHandle,
    source_monitor_handle: MonitorHandle,
    target_monitor_handle: MonitorHandle,
    rect: Rect,
  );
  fn do_restore_window(&self, window: &Window, is_minimised: &bool);
  fn do_maximise_window(&self, handle: WindowHandle);
  fn do_minimise_window(&self, handle: WindowHandle);
  fn do_hide_window(&self, handle: WindowHandle);
  fn do_unhide_window(&self, handle: WindowHandle);
  fn do_close_window(&self, handle: WindowHandle);
  fn get_window_placement(&self, handle: WindowHandle) -> Option<WindowPlacement>;
  fn set_window_placement_and_force_repaint(&self, handle: WindowHandle, placement: WindowPlacement);
  fn do_restore_window_placement(&self, handle: WindowHandle, previous_placement: WindowPlacement);
  fn get_cursor_position(&self) -> Point;
  fn set_cursor_position(&self, target_point: &Point);
  fn get_all_monitors(&self) -> Monitors;
  fn get_monitor_info_for_window(&self, handle: WindowHandle) -> Option<MonitorInfo>;
  fn get_monitor_info_for_monitor(&self, handle: MonitorHandle) -> Option<MonitorInfo>;
  fn get_monitor_id_for_handle(&self, handle: MonitorHandle) -> Option<[u16; 32]>;
  fn get_monitor_handle_for_window_handle(&self, handle: WindowHandle) -> MonitorHandle;
  fn get_monitor_handle_for_point(&self, point: &Point) -> MonitorHandle;
  fn get_virtual_desktop_manager(&self) -> Option<IVirtualDesktopManager>;
  fn is_window_on_current_desktop(&self, vdm: &IVirtualDesktopManager, window: &Window) -> bool;
}
