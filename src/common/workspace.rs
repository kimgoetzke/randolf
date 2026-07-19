use crate::api::WindowsApi;
use crate::common::{Monitor, MonitorHandle, PersistentWorkspaceId, Rect, Sizing, Window, WindowHandle, WorkspaceAction};
use std::fmt::Display;

/// Represents a Randolf workspace, which is a collection of zero or more windows that are managed together on a
/// specific monitor's desktop. Will only ever store windows if the workspace is inactive but is also used to position
/// a window on the monitor's desktop which it represents while in an active state.
#[derive(Debug, Clone)]
pub struct Workspace {
  pub id: PersistentWorkspaceId,
  pub monitor_handle: i64,
  pub monitor: Monitor,
  pub(super) windows: Vec<Window>,
  pub(super) minimised_windows: Vec<(WindowHandle, bool)>, // (window_handle, is_minimised)
  pub(super) margin: i32,
  is_active: bool,
}

impl Workspace {
  /// Creates a new, empty workspace with the specified ID and monitor that is marked as active.
  pub fn new_active(id: PersistentWorkspaceId, monitor: &Monitor, margin: i32) -> Self {
    Workspace {
      id,
      monitor_handle: monitor.handle.handle as i64,
      monitor: monitor.clone(),
      windows: vec![],
      minimised_windows: vec![],
      margin,
      is_active: true,
    }
  }

  /// Creates a new, empty workspace with the specified ID and monitor that is marked as inactive.
  pub fn new_inactive(id: PersistentWorkspaceId, monitor: &Monitor, margin: i32) -> Self {
    Workspace {
      id,
      monitor_handle: monitor.handle.handle as i64,
      monitor: monitor.clone(),
      windows: vec![],
      minimised_windows: vec![],
      margin,
      is_active: false,
    }
  }

  /// Returns `true` if the workspace is active.
  pub fn is_active(&self) -> bool {
    self.is_active
  }

  /// Sets the workspace as active (if `true`) or inactive (if `false`).
  pub fn set_active(&mut self, is_active: bool) {
    self.is_active = is_active;
  }

  /// Allows you to update the `MonitorHandle`, which is a non-persistent identifier of a monitor, for this workspace.
  /// Must be called prior to interacting with the workspace.
  pub fn update_handle(&mut self, monitor_handle: MonitorHandle) {
    self.monitor_handle = monitor_handle.handle as i64;
  }

  /// Returns the largest window in the workspace or `None` if none is present. The largest window is defined as the
  /// one covering with the largest area. If multiple windows have the same area, the first one found is returned (not
  /// deterministic).
  pub fn get_largest_window(&self) -> Option<Window> {
    self.windows.iter().max_by_key(|w| w.rect.area()).cloned().to_owned()
  }

  /// Moves the window if the workspace is active, otherwise stores and hides it, so that it can be restored later,
  /// when the workspace is activated, so that an active workspace must never store windows.
  pub fn move_or_store_and_hide_window(
    &mut self,
    window: Window,
    current_monitor: MonitorHandle,
    windows_api: &impl WindowsApi,
  ) -> WorkspaceAction {
    if self.is_active {
      self.move_window(window, current_monitor, windows_api);

      WorkspaceAction::Moved
    } else {
      self.store_and_hide_window(window, current_monitor, windows_api);

      WorkspaceAction::Stored
    }
  }

  /// Returns `true` if the workspace stores the specified window.
  pub fn stores(&self, handle: &WindowHandle) -> bool {
    self.windows.iter().any(|window| window.handle == *handle)
  }

  /// Stores and hides the specified windows. Clears the list of stored windows before storing the new ones.
  pub fn store_and_hide_windows(
    &mut self,
    windows: Vec<Window>,
    current_monitor: MonitorHandle,
    windows_api: &impl WindowsApi,
  ) {
    self.clear_windows();
    for window in windows.iter() {
      self.store_and_hide_window(window.clone(), current_monitor, windows_api);
    }
  }

  /// Removes the specified windows from the workspace. This method should be called after switching workspace and after
  /// a window is moved to a workspace to ensure windows don't exist in multiple workspaces. The reason why this is
  /// currently important is that Randolf does not listen to window events. The application does not know, for example,
  /// when another application has moved a hidden window from a workspace into the foreground. In order to allow the
  /// user to then move/hide this window again without Randolf storing it in multiple workspaces, this method is
  /// required to be called on every action that changes the window state in relation to a workspace.
  pub fn remove_windows_if_present(&mut self, windows: &[Window]) {
    for window in windows.iter() {
      self.windows.retain(|w| w.handle != window.handle);
      self.minimised_windows.retain(|(w, _)| *w != window.handle);
    }
  }

  /// Restores all windows that were stored in this workspace by unhiding them. Clears the list of stored windows
  /// after restoring.
  pub fn restore_windows(&mut self, api: &impl WindowsApi) {
    if self.windows.is_empty() && self.minimised_windows.is_empty() {
      debug!("No windows to restore for workspace [{}]", self.id);
      return;
    }
    if self.windows.len() != self.minimised_windows.len() {
      error!(
        "Data inconsistency detected: {} stores [{}] window(s) but [{}] window state(s)",
        self.id,
        self.windows.len(),
        self.minimised_windows.len()
      );
      return;
    }
    let mut i = 0;
    for (window_handle, is_minimised) in self.minimised_windows.iter() {
      i += 1;
      if *is_minimised {
        continue;
      }
      match self.windows.iter().find(|w| w.handle == *window_handle) {
        Some(window) => {
          if api.is_window_hidden(&window.handle) {
            debug!(
              "Restoring {} \"{}\" on workspace [{}]",
              window.handle,
              window.title_trunc(),
              self.id
            );
            api.do_restore_window(window, is_minimised);
          } else {
            debug!("Attempted to restore window {} but it is already visible", window_handle);
          }
        }
        None => {
          warn!("Attempted to restore window {window_handle} but workspace manager doesn't recognise it");
        }
      }
    }
    debug!("Restored [{}] window(s) on workspace [{}]", i, self.id);
    self.clear_windows();
  }

  fn move_window(&mut self, mut window: Window, current_monitor_handle: MonitorHandle, windows_api: &impl WindowsApi) {
    window = self.update_window_rect_if_required(window, current_monitor_handle, windows_api);
    if current_monitor_handle != self.monitor.handle {
      windows_api.set_window_position(window.handle, window.rect);
      std::thread::sleep(std::time::Duration::from_millis(10));
    }
    windows_api.set_window_position(window.handle, window.rect);
    windows_api.set_cursor_position(&window.rect.center());
    trace!(
      "Moved {} \"{}\" to active workspace [{}]",
      window.handle,
      window.title_trunc(),
      self.id
    );
  }

  pub(super) fn store_and_hide_window(
    &mut self,
    mut window: Window,
    current_monitor: MonitorHandle,
    windows_api: &impl WindowsApi,
  ) {
    if !self.windows.iter().any(|w| w.handle == window.handle) {
      if windows_api.is_window_minimised(window.handle) {
        debug!("{} is minimised, ignoring it for workspace [{}]", window.handle, self.id);
        return;
      }
      window = self.update_window_rect_if_required(window, current_monitor, windows_api);
      windows_api.do_hide_window(window.handle);
      self.minimised_windows.push((window.handle, false));
      self.windows.push(window.clone());
      trace!(
        "Stored and hid {} \"{}\" in workspace [{}]",
        window.handle,
        window.title_trunc(),
        self.id
      );
    } else {
      warn!(
        "{} already exists in workspace [{}], only hiding it now",
        window.handle, self.id
      );
      windows_api.do_hide_window(window.handle);
    }
  }

  pub(super) fn update_window_rect_if_required(
    &mut self,
    mut window: Window,
    current_monitor: MonitorHandle,
    windows_api: &impl WindowsApi,
  ) -> Window {
    if self.monitor_handle == current_monitor.as_i64() {
      return window;
    }

    // Check if window was near maximised or near-snapped on current monitor
    let new_sizing = if let Some(monitor_info) = windows_api.get_monitor_info_for_monitor(current_monitor) {
      let current_monitor_work_area = monitor_info.work_area;
      let current_sizing = Sizing::from(window.rect);
      match current_sizing {
        sizing if sizing == Sizing::near_maximised(current_monitor_work_area, self.margin) => {
          Some(Sizing::near_maximised(self.monitor.work_area, self.margin))
        }
        sizing if sizing == Sizing::left_half_of_screen(current_monitor_work_area, self.margin) => {
          Some(Sizing::left_half_of_screen(self.monitor.work_area, self.margin))
        }
        sizing if sizing == Sizing::right_half_of_screen(current_monitor_work_area, self.margin) => {
          Some(Sizing::right_half_of_screen(self.monitor.work_area, self.margin))
        }
        sizing if sizing == Sizing::top_half_of_screen(current_monitor_work_area, self.margin) => {
          Some(Sizing::top_half_of_screen(self.monitor.work_area, self.margin))
        }
        sizing if sizing == Sizing::bottom_half_of_screen(current_monitor_work_area, self.margin) => {
          Some(Sizing::bottom_half_of_screen(self.monitor.work_area, self.margin))
        }
        _ => None,
      }
    } else {
      error!(
        "Unable to get monitor info for current monitor {}, cannot detect if window was near-maximised or -snapped",
        current_monitor
      );

      None
    };

    let old_rect = window.rect;
    if let Some(new_sizing) = new_sizing {
      debug!("{} is currently near-maximised or -snapped", window.handle);
      window.rect = new_sizing.into();
    } else {
      debug!("{} is currently NOT near-maximised or -snapped", window.handle);
      let width = window.rect.width();
      let height = window.rect.height();
      let target_monitor_work_area_center = self.monitor.work_area.center();
      let left = target_monitor_work_area_center.x() - (width / 2);
      let top = target_monitor_work_area_center.y() - (height / 2);
      window.rect = Rect::new(left, top, left + width, top + height).clamp(&self.monitor.work_area, 10);
    }

    window.center = window.rect.center();
    trace!(
      "{} is being moved to a different monitor, its location was updated from {} to {}",
      window.handle, old_rect, window.rect
    );

    window
  }

  fn clear_windows(&mut self) {
    self.windows.clear();
    self.minimised_windows.clear();
  }
}

impl Display for Workspace {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "Workspace {{ id: {}, monitor_handle: {}, is_primary_monitor: {} }}",
      self.id, self.monitor_handle, self.monitor.is_primary
    )
  }
}
