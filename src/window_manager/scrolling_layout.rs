use crate::api::WindowsApi;
use crate::common::{Direction, PersistentWorkspaceId, Point, Rect, Sizing, Window, WindowHandle};
use crate::horizontal_layout::HorizontalLayout;
use crate::workspace_manager::WorkspaceManager;
use std::collections::HashMap;
use std::time::Duration;
use windows::Win32::UI::Shell::IVirtualDesktopManager;

const ANIMATION_FRAMES: u32 = 12;

#[derive(Default)]
pub(super) struct ScrollingLayout {
  layout: HorizontalLayout,
  positions: HashMap<PersistentWorkspaceId, Vec<(WindowHandle, Rect)>>,
  initialised: bool,
  previous_foreground_window: Option<WindowHandle>,
}

impl ScrollingLayout {
  pub(super) fn workspace_containing(&self, window: WindowHandle) -> Option<PersistentWorkspaceId> {
    self.layout.workspace_containing(window)
  }

  pub(super) fn members(&self, workspace: PersistentWorkspaceId) -> Vec<WindowHandle> {
    self.layout.members(workspace).to_vec()
  }

  pub(super) fn navigation_eligible(&self, window: WindowHandle) -> bool {
    self
      .layout
      .workspace_containing(window)
      .is_none_or(|workspace| self.layout.focused(workspace) == Some(window))
  }

  pub(super) fn remove(&mut self, workspace: PersistentWorkspaceId, window: WindowHandle) {
    self.layout.remove(workspace, window);
  }

  pub(super) fn insert(&mut self, workspace: PersistentWorkspaceId, window: WindowHandle) {
    self.layout.insert_before(workspace, window, None);
  }

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
      .and_then(|handle| self.layout.workspace_containing(handle));
    let mut windows = api
      .get_all_visible_windows()
      .into_iter()
      .filter(|window| virtual_desktop_manager.is_none_or(|vdm| api.is_window_on_current_desktop(vdm, window)))
      .collect::<Vec<_>>();
    windows.sort_by_key(|window| (window.rect.left, window.rect.top, window.handle.hwnd));

    let mut by_workspace: HashMap<PersistentWorkspaceId, Vec<Window>> = HashMap::new();
    for window in windows {
      let workspace = self
        .layout
        .workspace_containing(window.handle)
        .filter(|workspace| workspace_manager.is_workspace_active(*workspace))
        .or_else(|| workspace_manager.active_workspace_for_window(window.handle));
      if let Some(workspace) = workspace
        && active_workspaces.contains(&workspace)
      {
        by_workspace.entry(workspace).or_default().push(window);
      }
    }

    for (workspace, windows) in &by_workspace {
      for window in windows {
        if let Some(previous_workspace) = self.layout.workspace_containing(window.handle)
          && previous_workspace != *workspace
        {
          self.layout.remove(previous_workspace, window.handle);
        }
      }
    }

    let mut newly_focused = None;
    if !self.initialised {
      for workspace in active_workspaces {
        let members = by_workspace
          .get(workspace)
          .map(|windows| windows.iter().map(|window| window.handle).collect())
          .unwrap_or_default();
        self.layout.adopt(*workspace, members, foreground);
      }
      self.initialised = true;
    } else {
      for workspace in active_workspaces {
        let visible = by_workspace
          .get(workspace)
          .map(|windows| windows.iter().map(|window| window.handle).collect::<Vec<_>>())
          .unwrap_or_default();
        self.layout.retain(*workspace, &visible);
        let new_members = visible
          .iter()
          .copied()
          .filter(|handle| self.layout.workspace_containing(*handle).is_none())
          .collect::<Vec<_>>();
        let desired_focus = foreground
          .filter(|handle| new_members.contains(handle))
          .or_else(|| new_members.last().copied());
        for member in new_members {
          self.layout.insert_before(*workspace, member, self.previous_foreground_window);
        }
        if let Some(member) = desired_focus {
          self.layout.set_focused(*workspace, member);
          newly_focused = Some((*workspace, member));
        }
      }
    }

    for workspace in active_workspaces {
      self.reflow(api, workspace_manager, *workspace, margin);
    }
    if let Some((workspace, _)) = newly_focused {
      self.reflow(api, workspace_manager, workspace, margin);
      self.focus(api, workspace_manager, workspace, margin);
    } else if let Some(handle) = foreground
      && let Some(workspace) = self.layout.workspace_containing(handle)
    {
      self.layout.set_focused(workspace, handle);
      self.reflow(api, workspace_manager, workspace, margin);
    } else if let Some(workspace) = previous_workspace
      && workspace_manager.is_workspace_active(workspace)
      && self
        .previous_foreground_window
        .is_some_and(|handle| self.layout.workspace_containing(handle).is_none())
    {
      self.reflow(api, workspace_manager, workspace, margin);
      self.focus(api, workspace_manager, workspace, margin);
    }
    self.previous_foreground_window = api.get_foreground_window();
  }

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
      .layout
      .placements(workspace, monitor.work_area, margin)
      .into_iter()
      .map(|(handle, sizing)| (handle, Rect::from(sizing)))
      .collect::<Vec<_>>();
    if self.positions.get(&workspace) == Some(&positions) {
      return;
    }
    if let Some(focused) = self.layout.focused(workspace) {
      api.set_window_positions(&positions, focused);
    }
    self.positions.insert(workspace, positions);
  }

  pub(super) fn focus<T: WindowsApi + Clone>(
    &self,
    api: &T,
    workspace_manager: &WorkspaceManager<T>,
    workspace: PersistentWorkspaceId,
    margin: i32,
  ) {
    let Some(handle) = self.layout.focused(workspace) else {
      return;
    };
    let Some(monitor) = workspace_manager.monitor_for_workspace(workspace) else {
      return;
    };
    let sizing = Sizing::near_maximised(monitor.work_area, margin);
    api.set_foreground_window(handle);
    api.set_cursor_position(&Point::from_center_of_sizing(&sizing));
  }

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
    let Some(workspace) = self.layout.workspace_containing(current) else {
      return false;
    };
    if self.layout.focus_adjacent(workspace, current, direction).is_some() {
      self.animate(api, workspace_manager, workspace, current, margin, animation_duration);
      self.focus(api, workspace_manager, workspace, margin);
    }
    true
  }

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
    let Some(workspace) = self.layout.workspace_containing(current) else {
      return;
    };
    if self.layout.reorder(workspace, current, direction) {
      self.reflow(api, workspace_manager, workspace, margin);
      self.focus(api, workspace_manager, workspace, margin);
    }
  }

  pub(super) fn remove_and_refocus<T: WindowsApi + Clone>(
    &mut self,
    api: &T,
    workspace_manager: &WorkspaceManager<T>,
    member: WindowHandle,
    margin: i32,
  ) {
    let Some(workspace) = self.layout.workspace_containing(member) else {
      return;
    };
    self.layout.remove(workspace, member);
    self.reflow(api, workspace_manager, workspace, margin);
    self.focus(api, workspace_manager, workspace, margin);
  }

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
    for workspace in self.layout.workspace_ids() {
      let Some(monitor) = monitors.get_by_id(&workspace.monitor_id).or(fallback) else {
        continue;
      };
      let target = Rect::from(Sizing::near_maximised(monitor.work_area, margin));
      for handle in self.layout.members(workspace) {
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
      .layout
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
    if let Some(focused) = self.layout.focused(workspace) {
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
