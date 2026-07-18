use crate::api::WindowsApi;
use crate::common::{Direction, PersistentWorkspaceId, Point, Rect, Sizing, WindowHandle};
use crate::window_manager::scrolling_strips::ScrollingStrips;
use crate::workspace_manager::WorkspaceManager;
use std::collections::HashMap;
use std::time::Duration;
use windows::Win32::UI::Shell::IVirtualDesktopManager;

const ANIMATION_FRAMES: u32 = 12;

/// Visible strip members grouped by workspace and ordered spatially from left to right.
type MembersByWorkspace = HashMap<PersistentWorkspaceId, Vec<WindowHandle>>;

/// A layout that manages windows and allows scrolling. Keeps scrolling strip membership, focus, and positions across
/// workspaces.
#[derive(Default)]
pub(super) struct ScrollingLayout {
  strips: ScrollingStrips,
  positions: HashMap<PersistentWorkspaceId, Vec<(WindowHandle, Rect)>>,
  initialised: bool,
  previous_foreground_window: Option<WindowHandle>,
}

impl ScrollingLayout {
  /// Returns the scrolling workspace that owns a window.
  pub(super) fn get_workspace_containing(&self, window: WindowHandle) -> Option<PersistentWorkspaceId> {
    self.strips.workspace_containing(window)
  }

  /// Lists a workspace's windows in strip order.
  pub(super) fn get_members(&self, workspace: PersistentWorkspaceId) -> Vec<WindowHandle> {
    self.strips.members(workspace).to_vec()
  }

  /// Allows spatial navigation to use untracked windows and each strip's focused window.
  pub(super) fn is_navigation_eligible(&self, window: WindowHandle) -> bool {
    self
      .strips
      .workspace_containing(window)
      .is_none_or(|workspace| self.strips.focused(workspace) == Some(window))
  }

  /// Removes a window from a workspace's strip.
  pub(super) fn remove(&mut self, workspace: PersistentWorkspaceId, window: WindowHandle) {
    self.strips.remove(workspace, window);
  }

  /// Adds a window to a workspace's strip.
  pub(super) fn insert(&mut self, workspace: PersistentWorkspaceId, window: WindowHandle) {
    self.strips.insert_before(workspace, window, None);
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
      let target = Rect::from(Sizing::near_maximised(monitor.work_area, margin));
      for handle in members {
        let off_screen = api
          .get_window_rect(handle)
          .is_some_and(|rect| !screen_areas.iter().any(|area| rect.intersects(area)));
        if off_screen {
          api.set_window_position(handle, target);
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
      .and_then(|handle| self.strips.workspace_containing(handle));

    // Resolve membership before changing positions: off-screen members must retain their stored workspace
    let visible_members =
      self.get_visible_members_by_workspace(api, workspace_manager, active_workspaces, virtual_desktop_manager);
    self.remove_transferred_windows(&visible_members);
    let newly_focused_workspace = self.reconcile_memberships(active_workspaces, &visible_members, foreground);

    // Membership changes can alter every strip, while focus changes can require one further reflow
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

  /// Groups current-desktop windows by active workspace in deterministic "spatial" order.
  ///
  /// Existing strip membership wins over monitor detection because off-screen windows may appear to belong to another
  /// monitor.
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
      .filter(|window| virtual_desktop_manager.is_none_or(|vdm| api.is_window_on_current_desktop(vdm, window)))
      .collect::<Vec<_>>();
    windows.sort_by_key(|window| (window.rect.left, window.rect.top, window.handle.hwnd));

    let mut members_by_workspace: MembersByWorkspace = HashMap::new();
    for window in windows {
      let workspace = self
        .strips
        .workspace_containing(window.handle)
        .filter(|workspace| workspace_manager.is_workspace_active(*workspace))
        .or_else(|| workspace_manager.active_workspace_for_window(window.handle));
      if let Some(workspace) = workspace
        && active_workspaces.contains(&workspace)
      {
        members_by_workspace.entry(workspace).or_default().push(window.handle);
      }
    }
    members_by_workspace
  }

  /// Removes members from their old strip when workspace detection assigns them to another active workspace.
  fn remove_transferred_windows(&mut self, members_by_workspace: &MembersByWorkspace) {
    for (workspace, members) in members_by_workspace {
      for member in members {
        if let Some(previous_workspace) = self.strips.workspace_containing(*member)
          && previous_workspace != *workspace
        {
          self.strips.remove(previous_workspace, *member);
        }
      }
    }
  }

  /// Adopts initial strips or incrementally reconciles them, returning the workspace containing the newest focus.
  fn reconcile_memberships(
    &mut self,
    active_workspaces: &[PersistentWorkspaceId],
    visible_members: &MembersByWorkspace,
    foreground: Option<WindowHandle>,
  ) -> Option<PersistentWorkspaceId> {
    // The first snapshot establishes strip order; later snapshots must preserve user reordering.
    if !self.initialised {
      for workspace in active_workspaces {
        let members = visible_members.get(workspace).cloned().unwrap_or_default();
        self.strips.adopt(*workspace, members, foreground);
      }
      self.initialised = true;
      return None;
    }

    let mut newly_focused = None;
    for workspace in active_workspaces {
      if self.reconcile_workspace(*workspace, visible_members, foreground).is_some() {
        newly_focused = Some(*workspace);
      }
    }
    newly_focused
  }

  /// Removes missing members, inserts newly visible members, and returns any new member selected for focus.
  fn reconcile_workspace(
    &mut self,
    workspace: PersistentWorkspaceId,
    visible_members: &MembersByWorkspace,
    foreground: Option<WindowHandle>,
  ) -> Option<WindowHandle> {
    let visible: &[WindowHandle] = visible_members.get(&workspace).map_or(&[], Vec::as_slice);
    self.strips.retain(workspace, visible);
    let new_members = visible
      .iter()
      .copied()
      .filter(|handle| self.strips.workspace_containing(*handle).is_none())
      .collect::<Vec<_>>();
    // Prefer a newly foregrounded window; otherwise focus the last newly detected window.
    let desired_focus = foreground
      .filter(|handle| new_members.contains(handle))
      .or_else(|| new_members.last().copied());
    for member in new_members {
      self.strips.insert_before(workspace, member, self.previous_foreground_window);
    }
    if let Some(member) = desired_focus {
      self.strips.set_focused(workspace, member);
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
      && let Some(workspace) = self.strips.workspace_containing(handle)
    {
      self.strips.set_focused(workspace, handle);
      self.reflow(api, workspace_manager, workspace, margin);
    } else if let Some(workspace) = previous_workspace
      && workspace_manager.is_workspace_active(workspace)
      && self
        .previous_foreground_window
        .is_some_and(|handle| self.strips.workspace_containing(handle).is_none())
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
    let positions = self
      .strips
      .placements(workspace, monitor.work_area, margin)
      .into_iter()
      .map(|(handle, sizing)| (handle, Rect::from(sizing)))
      .collect::<Vec<_>>();
    if self.positions.get(&workspace) == Some(&positions) {
      return;
    }
    if let Some(focused) = self.strips.focused(workspace) {
      api.set_window_positions(&positions, focused);
    }
    self.positions.insert(workspace, positions);
  }

  /// Focuses a workspace's chosen strip window and centres the cursor on it.
  pub(super) fn focus<T: WindowsApi + Clone>(
    &self,
    api: &T,
    workspace_manager: &WorkspaceManager<T>,
    workspace: PersistentWorkspaceId,
    margin: i32,
  ) {
    let Some(handle) = self.strips.focused(workspace) else {
      return;
    };
    let Some(monitor) = workspace_manager.monitor_for_workspace(workspace) else {
      return;
    };
    let sizing = Sizing::near_maximised(monitor.work_area, margin);
    api.set_foreground_window(handle);
    api.set_cursor_position(&Point::from_center_of_sizing(&sizing));
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
    let Some(workspace) = self.strips.workspace_containing(current) else {
      return false;
    };
    if self.strips.focus_adjacent(workspace, current, direction).is_some() {
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
    let Some(workspace) = self.strips.workspace_containing(current) else {
      return;
    };
    if self.strips.reorder(workspace, current, direction) {
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
    let Some(workspace) = self.strips.workspace_containing(member) else {
      return;
    };
    self.strips.remove(workspace, member);
    self.reflow(api, workspace_manager, workspace, margin);
    self.focus(api, workspace_manager, workspace, margin);
  }

  /// Brings wholly off-screen strip windows back onto their workspace monitor.
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
      let target = Rect::from(Sizing::near_maximised(monitor.work_area, margin));
      for handle in self.strips.members(workspace) {
        let off_screen = api
          .get_window_rect(*handle)
          .is_some_and(|rect| !screen_areas.iter().any(|area| rect.intersects(area)));
        if off_screen {
          api.set_window_position(*handle, target);
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
      api.set_window_positions(&positions, outgoing);
      if frame < ANIMATION_FRAMES {
        std::thread::sleep(frame_duration);
      }
    }
    if let Some(focused) = self.strips.focused(workspace) {
      api.set_window_positions(&desired, focused);
    }
    self.positions.insert(workspace, desired);
  }
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
