use super::navigation;
use super::placement::Placement;
use super::scrolling::Scrolling;
use super::spatial;
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

pub struct WindowManager<T: WindowsApi> {
  pub(super) configuration_provider: Arc<Mutex<ConfigurationProvider>>,
  pub(super) placement: Placement,
  pub(super) allow_moving_cursor_after_close_or_minimise: bool,
  pub(super) scrolling: Scrolling,
  pub(super) workspace_manager: WorkspaceManager<T>,
  pub(super) virtual_desktop_manager: Option<IVirtualDesktopManager>,
  pub(super) windows_api: T,
}

impl<T: WindowsApi + Clone> WindowManager<T> {
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
      scrolling: Scrolling::default(),
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

  /// Returns the unique IDs for all desktop containers across all monitors in their natural order.
  pub fn get_ordered_permanent_workspace_ids(&mut self) -> Vec<PersistentWorkspaceId> {
    self.workspace_manager.get_ordered_permanent_workspace_ids()
  }

  pub fn close_window(&mut self) {
    let Some(window) = self.windows_api.get_foreground_window() else {
      return;
    };
    let layout = self.layout_for_window(window);
    self.windows_api.do_close_window(window);
    match layout {
      Some(Layout::Scrolling) => {
        let margin = self.margin();
        self
          .scrolling
          .remove_and_refocus(&self.windows_api, &self.workspace_manager, window, margin);
      }
      _ => spatial::after_close_or_minimise(&self.windows_api, window, self.allow_moving_cursor_after_close_or_minimise),
    }
  }

  pub fn minimise_window(&mut self) {
    let Some(window) = self.windows_api.get_foreground_window() else {
      return;
    };
    let layout = self.layout_for_window(window);
    self.windows_api.do_minimise_window(window);
    match layout {
      Some(Layout::Scrolling) => {
        let margin = self.margin();
        self
          .scrolling
          .remove_and_refocus(&self.windows_api, &self.workspace_manager, window, margin);
      }
      _ => spatial::after_close_or_minimise(&self.windows_api, window, self.allow_moving_cursor_after_close_or_minimise),
    }
  }

  pub fn switch_workspace(&mut self, id: PersistentWorkspaceId) {
    if self.layout_for_workspace(id) != Some(Layout::Scrolling) {
      self.workspace_manager.switch_workspace(id);
      return;
    }
    let source = self
      .workspace_manager
      .active_workspace_ids()
      .into_iter()
      .find(|workspace| workspace.monitor_id == id.monitor_id);
    let additional_windows = source.map_or_else(Vec::new, |workspace| self.scrolling.members(workspace));
    self
      .workspace_manager
      .switch_workspace_with_additional_windows(id, &additional_windows);
    let margin = self.margin();
    self.scrolling.reflow(&self.windows_api, &self.workspace_manager, id, margin);
    self.scrolling.focus(&self.windows_api, &self.workspace_manager, id, margin);
  }

  pub fn move_window_to_workspace(&mut self, id: PersistentWorkspaceId) {
    let foreground = self.windows_api.get_foreground_window();
    let source = foreground.and_then(|handle| self.workspace_for_window(handle));
    if source == Some(id) || self.workspace_manager.monitor_for_workspace(id).is_none() {
      return;
    }
    let source_layout = source.and_then(|workspace| self.layout_for_workspace(workspace));
    let target_layout = self.layout_for_workspace(id);
    self.workspace_manager.move_window_to_workspace(id);

    if let (Some(handle), Some(source)) = (foreground, source) {
      if source_layout == Some(Layout::Scrolling) {
        self.scrolling.remove(source, handle);
      }
      if target_layout == Some(Layout::Scrolling) {
        self.scrolling.insert(id, handle);
      }
      let margin = self.margin();
      if source_layout == Some(Layout::Scrolling) && self.workspace_manager.is_workspace_active(source) {
        self
          .scrolling
          .reflow(&self.windows_api, &self.workspace_manager, source, margin);
        self
          .scrolling
          .focus(&self.windows_api, &self.workspace_manager, source, margin);
      }
      if target_layout == Some(Layout::Scrolling) && self.workspace_manager.is_workspace_active(id) {
        self.scrolling.reflow(&self.windows_api, &self.workspace_manager, id, margin);
        self.scrolling.focus(&self.windows_api, &self.workspace_manager, id, margin);
      }
    }
  }

  pub fn move_window(&mut self, direction: Direction) {
    if self.foreground_layout() == Some(Layout::Scrolling) {
      if matches!(direction, Direction::Left | Direction::Right) {
        let margin = self.margin();
        self
          .scrolling
          .reorder(&self.windows_api, &self.workspace_manager, direction, margin);
      }
      return;
    }
    spatial::move_window(&self.windows_api, &self.placement, direction, self.margin());
  }

  pub fn resize_window(&mut self, direction: Direction) {
    if self.foreground_layout() != Some(Layout::Scrolling) {
      spatial::resize_window(&self.windows_api, &self.placement, direction, self.margin());
    }
  }

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
      .filter(|window| self.scrolling.navigation_eligible(window.handle))
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

  pub fn restore_all_managed_windows(&mut self) {
    self.workspace_manager.restore_all_managed_windows();
    self.scrolling.restore_off_screen(&self.windows_api, self.margin());
  }

  /// Reconciles layout runtimes with visible windows.
  pub fn reconcile_layouts(&mut self) {
    let active_workspaces = self
      .workspace_manager
      .active_workspace_ids()
      .into_iter()
      .filter(|workspace| self.layout_for_workspace(*workspace) == Some(Layout::Scrolling))
      .collect::<Vec<_>>();
    let margin = self.margin();
    self.scrolling.reconcile(
      &self.windows_api,
      &self.workspace_manager,
      &active_workspaces,
      self.virtual_desktop_manager.as_ref(),
      margin,
    );
  }

  fn layout_for_monitor(&self, monitor: &Monitor) -> Layout {
    self
      .configuration_provider
      .lock()
      .expect(CONFIGURATION_PROVIDER_LOCK)
      .layout_for_monitor(&monitor.id_to_string(), monitor.is_primary)
  }

  fn layout_for_workspace(&self, workspace: PersistentWorkspaceId) -> Option<Layout> {
    self
      .workspace_manager
      .monitor_for_workspace(workspace)
      .map(|monitor| self.layout_for_monitor(&monitor))
  }

  fn workspace_for_window(&self, window: WindowHandle) -> Option<PersistentWorkspaceId> {
    self
      .scrolling
      .workspace_containing(window)
      .or_else(|| self.workspace_manager.active_workspace_for_window(window))
  }

  fn layout_for_window(&self, window: WindowHandle) -> Option<Layout> {
    self
      .workspace_for_window(window)
      .and_then(|workspace| self.layout_for_workspace(workspace))
  }

  fn foreground_layout(&self) -> Option<Layout> {
    self
      .windows_api
      .get_foreground_window()
      .and_then(|window| self.layout_for_window(window))
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
