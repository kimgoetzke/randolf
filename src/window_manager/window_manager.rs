use super::navigation;
use super::scrolling_layout::ScrollingLayout;
use super::spatial_layout::SpatialLayout;
use crate::api::WindowsApi;
use crate::common::*;
use crate::configuration_provider::{
  ADDITIONAL_WORKSPACE_COUNT, ALLOW_MOVING_CURSOR_AFTER_OPEN_CLOSE_OR_MINIMISE, ALLOW_SELECTING_SAME_CENTER_WINDOWS,
  ConfigurationProvider, Layout, SCROLLING_ANIMATION_DURATION_IN_MS, WINDOW_MARGIN,
};
use crate::utils::{CONFIGURATION_PROVIDER_LOCK, MINIMUM_WINDOW_MARGIN};
use crate::workspace_manager::WorkspaceManager;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use windows::Win32::UI::Shell::IVirtualDesktopManager;

/// Routes window commands to the configured layout and coordinates workspace changes.
pub struct WindowManager<T: WindowsApi> {
  pub(super) configuration_provider: Arc<Mutex<ConfigurationProvider>>,
  pub(super) placement: Placement,
  pub(super) allow_moving_cursor_after_close_or_minimise: bool,
  pub(super) scrolling: ScrollingLayout,
  pub(super) spatial: SpatialLayout,
  pub(super) workspace_manager: WorkspaceManager<T>,
  pub(super) virtual_desktop_manager: Option<IVirtualDesktopManager>,
  pub(super) windows_api: T,
}

impl<T: WindowsApi + Clone> WindowManager<T> {
  /// Creates a manager backed by the supplied configuration and Windows API.
  ///
  /// Panics if configuration cannot be read or Windows provides no virtual desktop manager.
  pub fn new(configuration_provider: Arc<Mutex<ConfigurationProvider>>, api: T) -> Self {
    let guard = configuration_provider.try_lock().unwrap_or_else(|err| {
      panic!(
        "{} when trying to create window manager: {}",
        CONFIGURATION_PROVIDER_LOCK, err
      )
    });
    let additional_workspace_count = guard.get_i32(ADDITIONAL_WORKSPACE_COUNT);
    let window_margin = guard.get_i32(WINDOW_MARGIN);
    let allow_moving_cursor_after_close_or_minimise = guard.get_bool(ALLOW_MOVING_CURSOR_AFTER_OPEN_CLOSE_OR_MINIMISE);
    drop(guard);
    let workspace_manager = WorkspaceManager::new(additional_workspace_count, window_margin, api.clone());

    Self {
      placement: Placement::default(),
      allow_moving_cursor_after_close_or_minimise,
      scrolling: ScrollingLayout::default(),
      spatial: SpatialLayout,
      virtual_desktop_manager: Some(
        api
          .get_virtual_desktop_manager()
          .expect("Windows must provide the virtual desktop manager"),
      ),
      workspace_manager,
      configuration_provider,
      windows_api: api,
    }
  }

  /// Lists every permanent workspace in monitor and workspace order.
  pub fn get_ordered_permanent_workspace_ids(&mut self) -> Vec<PersistentWorkspaceId> {
    self.workspace_manager.get_ordered_permanent_workspace_ids()
  }

  /// Closes the foreground window and lets its layout choose the next focus.
  pub fn close_window(&mut self) {
    let Some(window) = self.windows_api.get_foreground_window() else {
      return;
    };
    let layout = self.get_layout_for_window(window);
    self.windows_api.do_close_window(window);
    self.execute_post_close_or_minimise_layout_specific_logic(window, layout);
  }

  /// Minimises the foreground window and lets its layout choose the next focus.
  pub fn minimise_window(&mut self) {
    let Some(window) = self.windows_api.get_foreground_window() else {
      return;
    };
    let layout = self.get_layout_for_window(window);
    self.windows_api.do_minimise_window(window);
    self.execute_post_close_or_minimise_layout_specific_logic(window, layout);
  }

  /// Shows a workspace and refreshes its scrolling strip when needed.
  pub fn switch_workspace(&mut self, id: PersistentWorkspaceId) {
    if self.get_layout_for_workspace(id) != Some(Layout::Scrolling) {
      self.workspace_manager.switch_workspace(id);
      return;
    }
    let source = self
      .workspace_manager
      .active_workspace_ids()
      .into_iter()
      .find(|workspace| workspace.monitor_id == id.monitor_id);
    let additional_windows = source.map_or_else(Vec::new, |workspace| self.scrolling.get_members(workspace));
    self
      .workspace_manager
      .switch_workspace_with_additional_windows(id, &additional_windows);
    let margin = self.margin();
    self.scrolling.reflow(&self.windows_api, &self.workspace_manager, id, margin);
    self.scrolling.focus(&self.windows_api, &self.workspace_manager, id, margin);
  }

  /// Moves the foreground window to a workspace and updates scrolling strip membership.
  pub fn move_window_to_workspace(&mut self, target_id: PersistentWorkspaceId) {
    let foreground = self.windows_api.get_foreground_window();
    let source = foreground.and_then(|handle| self.get_workspace_for_window(handle));
    if source == Some(target_id) || self.workspace_manager.monitor_for_workspace(target_id).is_none() {
      return;
    }
    let source_layout = source.and_then(|workspace| self.get_layout_for_workspace(workspace));
    let target_layout = self.get_layout_for_workspace(target_id);
    self.workspace_manager.move_window_to_workspace(target_id);

    if let (Some(handle), Some(source_id)) = (foreground, source) {
      let transferred_preset = if source_layout == Some(Layout::Scrolling) {
        self.scrolling.remove(source_id, handle)
      } else {
        None
      };
      let margin = self.margin();
      if target_layout == Some(Layout::Scrolling) {
        self.scrolling.insert(
          &self.windows_api,
          &self.workspace_manager,
          target_id,
          handle,
          transferred_preset,
          margin,
        );
      }
      for (workspace, layout) in [(source_id, source_layout), (target_id, target_layout)] {
        if layout == Some(Layout::Scrolling) && self.workspace_manager.is_workspace_active(workspace) {
          self
            .scrolling
            .reflow(&self.windows_api, &self.workspace_manager, workspace, margin);
          self
            .scrolling
            .focus(&self.windows_api, &self.workspace_manager, workspace, margin);
        }
      }
    }
  }

  /// Moves the foreground window according to its layout and the requested direction.
  pub fn move_window(&mut self, direction: Direction) {
    if self.get_foreground_window_layout() == Some(Layout::Scrolling) {
      if matches!(direction, Direction::Left | Direction::Right) {
        let margin = self.margin();
        self
          .scrolling
          .reorder(&self.windows_api, &self.workspace_manager, direction, margin);
      } else {
        self.move_scrolling_window_to_spatial_monitor(direction);
      }
      return;
    }
    self
      .spatial
      .move_window(&self.windows_api, &self.placement, direction, self.margin());
  }

  /// Moves the active window from a scrolling layout monitor to one that has a spatial layout. This method:
  /// - Gets the foreground window and its scrolling layout workspace
  /// - Finds the adjacent monitor in the requested direction
  /// - Confirms that monitor’s active workspace uses spatial layout
  /// - Removes the window from its strip
  /// - Updates the remaining source strip without changing focus
  /// - Moves and near-maximises the window on the target monitor
  /// - Centres the cursor and keeps the moved window foreground
  /// - No-ops when any required window, workspace, monitor, or layout is unavailable
  fn move_scrolling_window_to_spatial_monitor(&mut self, direction: Direction) {
    let Some(handle) = self.windows_api.get_foreground_window() else {
      return;
    };
    let Some(source_workspace_id) = self.scrolling.get_workspace_containing(handle) else {
      return;
    };
    let Some(source_monitor) = self.workspace_manager.monitor_for_workspace(source_workspace_id) else {
      return;
    };
    let monitors = self.windows_api.get_all_monitors();
    let Some(target_monitor) = monitors.get(direction, source_monitor.handle).cloned() else {
      return;
    };
    let Some(target_workspace_id) = self
      .workspace_manager
      .active_workspace_ids()
      .into_iter()
      .find(|workspace| workspace.monitor_id == target_monitor.id)
    else {
      return;
    };
    if self.get_layout_for_workspace(target_workspace_id) != Some(Layout::Spatial) {
      return;
    }
    if self.scrolling.remove(source_workspace_id, handle).is_none() {
      return;
    }

    let margin = self.margin();
    self
      .scrolling
      .reflow(&self.windows_api, &self.workspace_manager, source_workspace_id, margin);
    self
      .spatial
      .move_window_to_monitor(&self.windows_api, &self.placement, handle, &target_monitor, margin);
    self.windows_api.set_foreground_window(handle);
  }

  /// Resizes a window on a monitor using the spatial layout. Scrolling windows remain unchanged.
  pub fn resize_spatial_window(&mut self, direction: Direction) {
    if self.get_foreground_window_layout() != Some(Layout::Scrolling) {
      self
        .spatial
        .resize_window(&self.windows_api, &self.placement, direction, self.margin());
    }
  }

  /// Narrows or widens a scrolling layout window. No-ops in spatial layout.
  pub fn resize_scrolling_window(&mut self, direction: Direction) {
    if self.get_foreground_window_layout() != Some(Layout::Scrolling) {
      return;
    }
    let margin = self.margin();
    self
      .scrolling
      .resize_window(&self.windows_api, &self.workspace_manager, direction, margin);
  }

  /// Snaps a completed mouse resize when the window belongs to scrolling layout. Expected to be called after the user
  /// has resized a window using the mouse-based window resize features.
  pub fn finish_mouse_resize(&mut self, window: WindowHandle) {
    if self.get_layout_for_window(window) != Some(Layout::Scrolling) {
      return;
    }
    let margin = self.margin();
    self
      .scrolling
      .finish_mouse_resize(&self.windows_api, &self.workspace_manager, window, margin);
  }

  /// Moves focus and the cursor using navigation rules for the current layout.
  pub fn move_cursor(&mut self, direction: Direction) {
    if matches!(direction, Direction::Left | Direction::Right) {
      let margin = self.margin();
      let animation_duration = self.scrolling_animation_duration();
      if self.scrolling.move_focus(
        &self.windows_api,
        &self.workspace_manager,
        direction,
        margin,
        animation_duration,
      ) {
        return;
      }
    }

    let windows = self.windows_api.get_all_visible_windows();
    let eligible = windows
      .iter()
      .filter(|window| self.scrolling.is_navigation_eligible(window.handle))
      .collect::<Vec<_>>();
    let allow_same_center = self
      .configuration_provider
      .lock()
      .expect(CONFIGURATION_PROVIDER_LOCK)
      .get_bool(ALLOW_SELECTING_SAME_CENTER_WINDOWS);
    navigation::move_cursor(
      &self.windows_api,
      direction,
      &eligible,
      self.virtual_desktop_manager.as_ref(),
      allow_same_center,
    );
  }

  /// Toggles the foreground window between near-maximised and its previous position.
  pub fn near_maximise_or_restore(&mut self) {
    let Some(window) = self.windows_api.get_foreground_window() else {
      return;
    };
    let Some(window_placement) = self.windows_api.get_window_placement(window) else {
      return;
    };
    let Some(monitor_info) = self.windows_api.get_monitor_info_for_window(window) else {
      return;
    };
    let margin = self.margin();
    self
      .placement
      .near_maximise_or_restore(&self.windows_api, window, window_placement, monitor_info, margin);
  }

  /// Brings back windows hidden or moved off-screen by managed layouts.
  pub fn restore_all_managed_windows(&mut self) {
    self.workspace_manager.restore_all_managed_windows();
    self.scrolling.restore_off_screen(&self.windows_api, self.margin());
  }

  /// Updates active layout state to match the visible managed windows.
  pub fn reconcile_layouts(&mut self) {
    let active_workspaces = self.workspace_manager.active_workspace_ids();
    let scrolling_workspaces = active_workspaces
      .iter()
      .copied()
      .filter(|workspace| self.get_layout_for_workspace(*workspace) == Some(Layout::Scrolling))
      .collect::<Vec<_>>();
    let spatial_workspaces = active_workspaces
      .into_iter()
      .filter(|workspace| self.get_layout_for_workspace(*workspace) == Some(Layout::Spatial))
      .collect::<Vec<_>>();
    let margin = self.margin();
    self
      .scrolling
      .deactivate(&self.windows_api, &self.workspace_manager, &spatial_workspaces, margin);
    self.scrolling.reconcile(
      &self.windows_api,
      &self.workspace_manager,
      &scrolling_workspaces,
      self.virtual_desktop_manager.as_ref(),
      margin,
    );
  }

  fn execute_post_close_or_minimise_layout_specific_logic(&mut self, window: WindowHandle, layout: Option<Layout>) {
    match layout {
      Some(Layout::Scrolling) => {
        let margin = self.margin();
        self
          .scrolling
          .remove_and_refocus(&self.windows_api, &self.workspace_manager, window, margin);
      }
      _ => self
        .spatial
        .after_close_or_minimise(&self.windows_api, window, self.allow_moving_cursor_after_close_or_minimise),
    }
  }

  fn get_layout_for_monitor(&self, monitor: &Monitor) -> Layout {
    self
      .configuration_provider
      .lock()
      .expect(CONFIGURATION_PROVIDER_LOCK)
      .layout_for_monitor(&monitor.id_to_string(), monitor.is_primary)
  }

  fn get_layout_for_workspace(&self, workspace: PersistentWorkspaceId) -> Option<Layout> {
    self
      .workspace_manager
      .monitor_for_workspace(workspace)
      .map(|monitor| self.get_layout_for_monitor(&monitor))
  }

  fn get_workspace_for_window(&self, window: WindowHandle) -> Option<PersistentWorkspaceId> {
    self
      .scrolling
      .get_workspace_containing(window)
      .or_else(|| self.workspace_manager.active_workspace_for_window(window))
  }

  fn get_layout_for_window(&self, window: WindowHandle) -> Option<Layout> {
    self
      .get_workspace_for_window(window)
      .and_then(|workspace| self.get_layout_for_workspace(workspace))
  }

  fn get_foreground_window_layout(&self) -> Option<Layout> {
    self
      .windows_api
      .get_foreground_window()
      .and_then(|window| self.get_layout_for_window(window))
  }

  fn margin(&self) -> i32 {
    let margin = self
      .configuration_provider
      .lock()
      .expect(CONFIGURATION_PROVIDER_LOCK)
      .get_i32(WINDOW_MARGIN);
    if margin >= MINIMUM_WINDOW_MARGIN { margin } else { 0 }
  }

  fn scrolling_animation_duration(&self) -> Duration {
    let duration = self
      .configuration_provider
      .lock()
      .expect(CONFIGURATION_PROVIDER_LOCK)
      .get_i32(SCROLLING_ANIMATION_DURATION_IN_MS);
    Duration::from_millis(u64::try_from(duration).unwrap_or_default())
  }
}
