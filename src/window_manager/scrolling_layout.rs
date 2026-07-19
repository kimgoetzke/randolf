use crate::api::WindowsApi;
use crate::common::{Direction, PersistentWorkspaceId, Point, Rect, Sizing, WidthPreset, Window, WindowHandle};
use crate::window_manager::scrolling_strips::ScrollingStrips;
use crate::workspace_manager::WorkspaceManager;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use windows::Win32::UI::Shell::IVirtualDesktopManager;

const ANIMATION_FRAMES: u32 = 12;

/// Visible strip members grouped by workspace and ordered spatially from left to right.
type MembersByWorkspace = HashMap<PersistentWorkspaceId, Vec<Window>>;

/// A layout that manages windows and allows scrolling. Keeps scrolling strip membership, focus, and positions across
/// workspaces.
#[derive(Default)]
pub(super) struct ScrollingLayout {
  strips: ScrollingStrips,
  positions: HashMap<PersistentWorkspaceId, Vec<(WindowHandle, Rect)>>,
  initialised: bool,
  previous_foreground_window: Option<WindowHandle>,
  unpositionable: HashSet<WindowHandle>,
}

impl ScrollingLayout {
  /// Returns the scrolling workspace that owns a window.
  pub(super) fn get_workspace_containing(&self, window: WindowHandle) -> Option<PersistentWorkspaceId> {
    self.strips.get_workspace_containing(window)
  }

  /// Lists a workspace's windows in strip order.
  pub(super) fn get_members(&self, workspace: PersistentWorkspaceId) -> Vec<WindowHandle> {
    self.strips.members(workspace)
  }

  /// Allows spatial navigation to use untracked windows and each strip's focused window.
  pub(super) fn is_navigation_eligible(&self, window: WindowHandle) -> bool {
    self
      .strips
      .get_workspace_containing(window)
      .is_none_or(|workspace| self.strips.get_active_handle(workspace) == Some(window))
  }

  /// Removes a window from a workspace's strip and returns its preset.
  pub(super) fn remove(&mut self, workspace: PersistentWorkspaceId, window: WindowHandle) -> Option<WidthPreset> {
    self.positions.remove(&workspace);
    self.strips.remove(workspace, window)
  }

  /// Adds a window to a workspace's strip, retaining a transferred preset when supplied.
  pub(super) fn insert<T: WindowsApi + Clone>(
    &mut self,
    api: &T,
    workspace_manager: &WorkspaceManager<T>,
    workspace: PersistentWorkspaceId,
    window: WindowHandle,
    preset: Option<WidthPreset>,
    margin: i32,
  ) {
    let Some(monitor) = workspace_manager.monitor_for_workspace(workspace) else {
      return;
    };
    let preset = preset.unwrap_or_else(|| {
      let observed_width = api.get_window_rect(window).map_or(0, |rect| rect.width());
      WidthPreset::nearest(observed_width, usable_width(monitor.work_area, margin))
    });
    self.strips.insert_before(workspace, window, preset, None);
    self.positions.remove(&workspace);
  }

  /// Restores and releases active strips that no longer use Scrolling Layout.
  pub(super) fn deactivate<T: WindowsApi + Clone>(
    &mut self,
    api: &T,
    workspace_manager: &WorkspaceManager<T>,
    workspaces: &[PersistentWorkspaceId],
    margin: i32,
  ) {
    let screen_areas = api
      .get_all_monitors()
      .get_all()
      .into_iter()
      .map(|monitor| monitor.monitor_area)
      .collect::<Vec<_>>();
    for workspace in workspaces {
      let members = self.strips.remove_workspace(*workspace);
      self.positions.remove(workspace);
      let Some(monitor) = workspace_manager.monitor_for_workspace(*workspace) else {
        continue;
      };
      for (handle, preset) in members {
        let off_screen = api
          .get_window_rect(handle)
          .is_some_and(|rect| !screen_areas.iter().any(|area| rect.intersects(area)));
        if off_screen {
          api.set_window_position(handle, Rect::from(assigned_sizing(monitor.work_area, margin, preset)));
        }
      }
    }
  }

  /// Synchronises active strips with visible, managed windows, then restores layout and focus.
  ///
  /// Reconciliation stops while an unmanaged window owns focus so pop-ups and menus keep their focus and cursor.
  pub(super) fn reconcile<T: WindowsApi + Clone>(
    &mut self,
    api: &T,
    workspace_manager: &WorkspaceManager<T>,
    active_workspaces: &[PersistentWorkspaceId],
    virtual_desktop_manager: Option<&IVirtualDesktopManager>,
    margin: i32,
  ) {
    if active_workspaces.is_empty() {
      return;
    }

    let foreground = api.get_foreground_window();
    if foreground.is_some_and(|handle| api.is_not_a_managed_window(&handle)) {
      return;
    }

    let previous_workspace = self
      .previous_foreground_window
      .and_then(|handle| self.strips.get_workspace_containing(handle));

    // Resolve membership before changing positions: off-screen members must retain their stored workspace.
    let visible_members =
      self.get_visible_members_by_workspace(api, workspace_manager, active_workspaces, virtual_desktop_manager);
    let transferred_presets = self.remove_transferred_windows(&visible_members);
    let newly_focused_workspace = self.reconcile_memberships(
      workspace_manager,
      active_workspaces,
      &visible_members,
      foreground,
      &transferred_presets,
      margin,
    );

    for workspace in active_workspaces {
      self.reflow(api, workspace_manager, *workspace, margin);
    }
    self.reconcile_focus(
      api,
      workspace_manager,
      foreground,
      previous_workspace,
      newly_focused_workspace,
      margin,
    );
    self.previous_foreground_window = api.get_foreground_window();
  }

  /// Groups current-desktop windows by active workspace in deterministic spatial order.
  fn get_visible_members_by_workspace<T: WindowsApi + Clone>(
    &self,
    api: &T,
    workspace_manager: &WorkspaceManager<T>,
    active_workspaces: &[PersistentWorkspaceId],
    virtual_desktop_manager: Option<&IVirtualDesktopManager>,
  ) -> MembersByWorkspace {
    let mut windows = api
      .get_all_visible_windows()
      .into_iter()
      .filter(|window| !self.unpositionable.contains(&window.handle))
      .filter(|window| virtual_desktop_manager.is_none_or(|vdm| api.is_window_on_current_desktop(vdm, window)))
      .collect::<Vec<_>>();
    windows.sort_by_key(|window| (window.rect.left, window.rect.top, window.handle.hwnd));

    let mut members_by_workspace: MembersByWorkspace = HashMap::new();
    for window in windows {
      let workspace = self
        .strips
        .get_workspace_containing(window.handle)
        .filter(|workspace| workspace_manager.is_workspace_active(*workspace))
        .or_else(|| workspace_manager.active_workspace_for_window(window.handle));
      if let Some(workspace) = workspace
        && active_workspaces.contains(&workspace)
      {
        members_by_workspace.entry(workspace).or_default().push(window);
      }
    }
    members_by_workspace
  }

  /// Removes members from their old strip when workspace detection assigns them to another active workspace.
  fn remove_transferred_windows(&mut self, members_by_workspace: &MembersByWorkspace) -> HashMap<WindowHandle, WidthPreset> {
    let mut presets = HashMap::new();
    for (workspace, members) in members_by_workspace {
      for member in members {
        if let Some(previous_workspace) = self.strips.get_workspace_containing(member.handle)
          && previous_workspace != *workspace
          && let Some(preset) = self.strips.remove(previous_workspace, member.handle)
        {
          self.positions.remove(&previous_workspace);
          presets.insert(member.handle, preset);
        }
      }
    }
    presets
  }

  /// Adopts initial strips or incrementally reconciles them, returning the workspace containing the newest focus.
  fn reconcile_memberships<T: WindowsApi + Clone>(
    &mut self,
    workspace_manager: &WorkspaceManager<T>,
    active_workspaces: &[PersistentWorkspaceId],
    visible_members: &MembersByWorkspace,
    foreground: Option<WindowHandle>,
    transferred_presets: &HashMap<WindowHandle, WidthPreset>,
    margin: i32,
  ) -> Option<PersistentWorkspaceId> {
    if !self.initialised {
      for workspace in active_workspaces {
        let usable_width = workspace_manager
          .monitor_for_workspace(*workspace)
          .map_or(1, |monitor| usable_width(monitor.work_area, margin));
        let members = visible_members
          .get(workspace)
          .into_iter()
          .flatten()
          .map(|window| (window.handle, WidthPreset::nearest(window.rect.width(), usable_width)))
          .collect();
        self.strips.adopt(*workspace, members, foreground);
      }
      self.initialised = true;
      return None;
    }

    let mut newly_focused = None;
    for workspace in active_workspaces {
      if self
        .reconcile_workspace(
          workspace_manager,
          *workspace,
          visible_members,
          foreground,
          transferred_presets,
          margin,
        )
        .is_some()
      {
        newly_focused = Some(*workspace);
      }
    }
    newly_focused
  }

  /// Removes missing members, inserts newly visible members, and returns any new member selected for focus.
  fn reconcile_workspace<T: WindowsApi + Clone>(
    &mut self,
    workspace_manager: &WorkspaceManager<T>,
    workspace: PersistentWorkspaceId,
    visible_members: &MembersByWorkspace,
    foreground: Option<WindowHandle>,
    transferred_presets: &HashMap<WindowHandle, WidthPreset>,
    margin: i32,
  ) -> Option<WindowHandle> {
    let visible = visible_members.get(&workspace).map_or(&[][..], Vec::as_slice);
    let visible_handles = visible.iter().map(|window| window.handle).collect::<Vec<_>>();
    self.strips.retain(workspace, &visible_handles);
    let new_members = visible
      .iter()
      .filter(|window| self.strips.get_workspace_containing(window.handle).is_none())
      .collect::<Vec<_>>();
    let desired_focus = foreground
      .filter(|handle| new_members.iter().any(|window| window.handle == *handle))
      .or_else(|| new_members.last().map(|window| window.handle));
    let usable_width = workspace_manager
      .monitor_for_workspace(workspace)
      .map_or(1, |monitor| usable_width(monitor.work_area, margin));
    for member in new_members {
      let preset = transferred_presets
        .get(&member.handle)
        .copied()
        .unwrap_or_else(|| WidthPreset::nearest(member.rect.width(), usable_width));
      self
        .strips
        .insert_before(workspace, member.handle, preset, self.previous_foreground_window);
    }
    if let Some(member) = desired_focus {
      self.strips.set_active(workspace, member);
    }
    desired_focus
  }

  /// Applies focus priority: new member, current foreground member, then the neighbour of a removed foreground member.
  fn reconcile_focus<T: WindowsApi + Clone>(
    &mut self,
    api: &T,
    workspace_manager: &WorkspaceManager<T>,
    foreground: Option<WindowHandle>,
    previous_workspace: Option<PersistentWorkspaceId>,
    newly_focused_workspace: Option<PersistentWorkspaceId>,
    margin: i32,
  ) {
    if let Some(workspace) = newly_focused_workspace {
      self.reflow(api, workspace_manager, workspace, margin);
      self.focus(api, workspace_manager, workspace, margin);
    } else if let Some(handle) = foreground
      && let Some(workspace) = self.strips.get_workspace_containing(handle)
    {
      self.strips.set_active(workspace, handle);
      self.reflow(api, workspace_manager, workspace, margin);
    } else if let Some(workspace) = previous_workspace
      && workspace_manager.is_workspace_active(workspace)
      && self
        .previous_foreground_window
        .is_some_and(|handle| self.strips.get_workspace_containing(handle).is_none())
    {
      self.reflow(api, workspace_manager, workspace, margin);
      self.focus(api, workspace_manager, workspace, margin);
    }
  }

  /// Places a workspace's strip windows from their current order.
  pub(super) fn reflow<T: WindowsApi + Clone>(
    &mut self,
    api: &T,
    workspace_manager: &WorkspaceManager<T>,
    workspace: PersistentWorkspaceId,
    margin: i32,
  ) {
    let Some(monitor) = workspace_manager.monitor_for_workspace(workspace) else {
      return;
    };
    loop {
      let positions = self
        .strips
        .placements(workspace, monitor.work_area, margin)
        .into_iter()
        .map(|(handle, sizing)| (handle, Rect::from(sizing)))
        .collect::<Vec<_>>();
      if self.positions.get(&workspace) == Some(&positions) {
        return;
      }
      if let Some(focused) = self.strips.get_active_handle(workspace) {
        let failures = api.set_window_positions(&positions, focused);
        if !failures.is_empty() {
          self.reject_unpositionable(workspace, &failures);
          continue;
        }
      }
      self.positions.insert(workspace, positions);
      return;
    }
  }

  fn reject_unpositionable(&mut self, workspace: PersistentWorkspaceId, failures: &[WindowHandle]) {
    for handle in failures {
      self.unpositionable.insert(*handle);
      self.strips.remove(workspace, *handle);
    }
    self.positions.remove(&workspace);
  }

  /// Focuses a workspace's chosen strip window and centres the cursor on its target.
  pub(super) fn focus<T: WindowsApi + Clone>(
    &self,
    api: &T,
    workspace_manager: &WorkspaceManager<T>,
    workspace: PersistentWorkspaceId,
    margin: i32,
  ) {
    let Some(handle) = self.strips.get_active_handle(workspace) else {
      return;
    };
    let Some(monitor) = workspace_manager.monitor_for_workspace(workspace) else {
      return;
    };
    let Some((_, sizing)) = self
      .strips
      .placements(workspace, monitor.work_area, margin)
      .into_iter()
      .find(|(member, _)| *member == handle)
    else {
      return;
    };
    api.set_foreground_window(handle);
    api.set_cursor_position(&Point::from_center_of_sizing(&sizing));
  }

  /// Narrows or widens the focused member.
  pub(super) fn resize_window<T: WindowsApi + Clone>(
    &mut self,
    api: &T,
    workspace_manager: &WorkspaceManager<T>,
    direction: Direction,
    margin: i32,
  ) {
    let Some(handle) = api.get_foreground_window() else {
      return;
    };
    let Some(workspace) = self.strips.get_workspace_containing(handle) else {
      return;
    };
    if self.strips.resize(workspace, handle, direction) {
      self.positions.remove(&workspace);
      self.reflow(api, workspace_manager, workspace, margin);
      self.focus(api, workspace_manager, workspace, margin);
    }
  }

  /// Snaps a completed mouse resize into the strip and reflows once.
  pub(super) fn finish_mouse_resize<T: WindowsApi + Clone>(
    &mut self,
    api: &T,
    workspace_manager: &WorkspaceManager<T>,
    handle: WindowHandle,
    margin: i32,
  ) {
    let Some(workspace) = self.strips.get_workspace_containing(handle) else {
      return;
    };
    let Some(monitor) = workspace_manager.monitor_for_workspace(workspace) else {
      return;
    };
    let Some(rect) = api.get_window_rect(handle) else {
      return;
    };
    let preset = WidthPreset::nearest(rect.width(), usable_width(monitor.work_area, margin));
    self.strips.set_width_preset(workspace, handle, preset);
    self.strips.set_active(workspace, handle);
    // Low-level dragging bypasses the target cache, even when the preset itself is unchanged
    self.positions.remove(&workspace);
    self.reflow(api, workspace_manager, workspace, margin);
    self.focus(api, workspace_manager, workspace, margin);
  }

  /// Handles adjacent strip focus, returning false when the foreground is not in a strip.
  pub(super) fn move_focus<T: WindowsApi + Clone>(
    &mut self,
    api: &T,
    workspace_manager: &WorkspaceManager<T>,
    direction: Direction,
    margin: i32,
    animation_duration: Duration,
  ) -> bool {
    let Some(current) = api.get_foreground_window() else {
      return false;
    };
    let Some(workspace) = self.strips.get_workspace_containing(current) else {
      return false;
    };
    if self.strips.set_adjacent_active(workspace, current, direction).is_some() {
      self.animate(api, workspace_manager, workspace, current, margin, animation_duration);
      self.focus(api, workspace_manager, workspace, margin);
    }
    true
  }

  /// Moves the foreground window one place through its strip.
  pub(super) fn reorder<T: WindowsApi + Clone>(
    &mut self,
    api: &T,
    workspace_manager: &WorkspaceManager<T>,
    direction: Direction,
    margin: i32,
  ) {
    let Some(current) = api.get_foreground_window() else {
      return;
    };
    let Some(workspace) = self.strips.get_workspace_containing(current) else {
      return;
    };
    if self.strips.reorder(workspace, current, direction) {
      self.positions.remove(&workspace);
      self.reflow(api, workspace_manager, workspace, margin);
      self.focus(api, workspace_manager, workspace, margin);
    }
  }

  /// Removes a strip window, then lays out and focuses the remaining strip.
  pub(super) fn remove_and_refocus<T: WindowsApi + Clone>(
    &mut self,
    api: &T,
    workspace_manager: &WorkspaceManager<T>,
    member: WindowHandle,
    margin: i32,
  ) {
    let Some(workspace) = self.strips.get_workspace_containing(member) else {
      return;
    };
    self.strips.remove(workspace, member);
    self.positions.remove(&workspace);
    self.reflow(api, workspace_manager, workspace, margin);
    self.focus(api, workspace_manager, workspace, margin);
  }

  /// Brings wholly off-screen strip windows back onto their workspace monitor at their assigned widths.
  pub(super) fn restore_off_screen<T: WindowsApi>(&self, api: &T, margin: i32) {
    let monitors = api.get_all_monitors();
    let screen_areas = monitors
      .get_all()
      .into_iter()
      .map(|monitor| monitor.monitor_area)
      .collect::<Vec<_>>();
    let fallback = monitors
      .get_all()
      .into_iter()
      .find(|monitor| monitor.is_primary)
      .or_else(|| monitors.get_all().into_iter().next());
    for workspace in self.strips.workspace_ids() {
      let Some(monitor) = monitors.get_by_id(&workspace.monitor_id).or(fallback) else {
        continue;
      };
      for handle in self.strips.members(workspace) {
        let off_screen = api
          .get_window_rect(handle)
          .is_some_and(|rect| !screen_areas.iter().any(|area| rect.intersects(area)));
        if off_screen && let Some(preset) = self.strips.get_width_preset(workspace, handle) {
          api.set_window_position(handle, Rect::from(assigned_sizing(monitor.work_area, margin, preset)));
        }
      }
    }
  }

  fn animate<T: WindowsApi + Clone>(
    &mut self,
    api: &T,
    workspace_manager: &WorkspaceManager<T>,
    workspace: PersistentWorkspaceId,
    outgoing: WindowHandle,
    margin: i32,
    animation_duration: Duration,
  ) {
    let Some(monitor) = workspace_manager.monitor_for_workspace(workspace) else {
      return;
    };
    let desired = self
      .strips
      .placements(workspace, monitor.work_area, margin)
      .into_iter()
      .map(|(handle, sizing)| (handle, Rect::from(sizing)))
      .collect::<Vec<_>>();
    let starts = desired
      .iter()
      .map(|(handle, target)| (*handle, api.get_window_rect(*handle).unwrap_or(*target)))
      .collect::<HashMap<_, _>>();
    let frame_duration = animation_duration / ANIMATION_FRAMES;
    for frame in 1..=ANIMATION_FRAMES {
      let progress = f64::from(frame) / f64::from(ANIMATION_FRAMES);
      let eased_progress = 1.0 - (1.0 - progress).powi(3);
      let positions = desired
        .iter()
        .map(|(handle, target)| {
          let start = starts.get(handle).copied().unwrap_or(*target);
          (*handle, interpolate_rect(start, *target, eased_progress))
        })
        .collect::<Vec<_>>();
      let failures = api.set_window_positions(&positions, outgoing);
      if !failures.is_empty() {
        self.reject_unpositionable(workspace, &failures);
        self.reflow(api, workspace_manager, workspace, margin);
        return;
      }
      if frame < ANIMATION_FRAMES {
        std::thread::sleep(frame_duration);
      }
    }
    if let Some(focused) = self.strips.get_active_handle(workspace) {
      let failures = api.set_window_positions(&desired, focused);
      if !failures.is_empty() {
        self.reject_unpositionable(workspace, &failures);
        self.reflow(api, workspace_manager, workspace, margin);
        return;
      }
    }
    self.positions.insert(workspace, desired);
  }
}

fn usable_width(work_area: Rect, margin: i32) -> i32 {
  Sizing::near_maximised(work_area, margin).width.max(1)
}

fn assigned_sizing(work_area: Rect, margin: i32, preset: WidthPreset) -> Sizing {
  let near_maximised = Sizing::near_maximised(work_area, margin);
  let width = preset.width(near_maximised.width.max(1));
  Sizing::new(
    work_area.left.saturating_add(work_area.width().saturating_sub(width) / 2),
    near_maximised.y,
    width,
    near_maximised.height,
  )
}

fn interpolate_rect(start: Rect, target: Rect, progress: f64) -> Rect {
  fn coordinate(start: i32, target: i32, progress: f64) -> i32 {
    (f64::from(start) + f64::from(target - start) * progress.clamp(0.0, 1.0)).round() as i32
  }
  Rect::new(
    coordinate(start.left, target.left, progress),
    coordinate(start.top, target.top, progress),
    coordinate(start.right, target.right, progress),
    coordinate(start.bottom, target.bottom, progress),
  )
}
