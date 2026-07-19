use crate::common::{Direction, PersistentWorkspaceId, Rect, Sizing, WidthPreset, WindowHandle};
use std::collections::HashMap;

/// Stores ordered scrolling strip membership and focus for each workspace. Must only be used by
/// [`crate::window_manager::scrolling_layout::ScrollingLayout`].
#[derive(Debug, Default)]
pub(crate) struct ScrollingStrips {
  by_workspace: HashMap<PersistentWorkspaceId, Strip>,
}

/// A window that belows to a given strip, and it's current size (i.e [`WidthPreset`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Member {
  handle: WindowHandle,
  width_preset: WidthPreset,
}

/// Stores ordered members and current focus for one workspace.
#[derive(Debug, Default)]
struct Strip {
  members: Vec<Member>,
  active_window_handle: Option<WindowHandle>,
}

impl ScrollingStrips {
  /// Replaces a workspace strip with ordered members and presets.
  pub(crate) fn adopt(
    &mut self,
    workspace: PersistentWorkspaceId,
    members: Vec<(WindowHandle, WidthPreset)>,
    focused: Option<WindowHandle>,
  ) {
    let members = members
      .into_iter()
      .map(|(handle, preset)| Member {
        handle,
        width_preset: preset,
      })
      .collect::<Vec<_>>();
    let focused = focused
      .filter(|handle| members.iter().any(|member| member.handle == *handle))
      .or_else(|| members.first().map(|member| member.handle));
    self.by_workspace.insert(
      workspace,
      Strip {
        members,
        active_window_handle: focused,
      },
    );
  }

  /// Inserts a member immediately before the previous active member.
  pub(crate) fn insert_before(
    &mut self,
    workspace: PersistentWorkspaceId,
    handle: WindowHandle,
    preset: WidthPreset,
    previous_active: Option<WindowHandle>,
  ) {
    let strip = self.by_workspace.entry(workspace).or_default();
    strip.members.retain(|member| member.handle != handle);
    let index = previous_active
      .and_then(|active| strip.members.iter().position(|member| member.handle == active))
      .unwrap_or(strip.members.len());
    strip.members.insert(
      index,
      Member {
        handle,
        width_preset: preset,
      },
    );
    strip.active_window_handle = Some(handle);
  }

  /// Returns ordered members for a workspace.
  pub(crate) fn members(&self, workspace: PersistentWorkspaceId) -> Vec<WindowHandle> {
    self
      .by_workspace
      .get(&workspace)
      .map(|strip| strip.members.iter().map(|member| member.handle).collect())
      .unwrap_or_default()
  }

  /// Returns IDs for all tracked workspaces.
  pub(crate) fn workspace_ids(&self) -> impl Iterator<Item = PersistentWorkspaceId> + '_ {
    self.by_workspace.keys().copied()
  }

  /// Removes a workspace strip and returns its members and presets.
  pub(crate) fn remove_workspace(&mut self, workspace: PersistentWorkspaceId) -> Vec<(WindowHandle, WidthPreset)> {
    self.by_workspace.remove(&workspace).map_or_else(Vec::new, |strip| {
      strip
        .members
        .into_iter()
        .map(|member| (member.handle, member.width_preset))
        .collect()
    })
  }

  /// Returns the workspace containing a member.
  pub(crate) fn get_workspace_containing(&self, handle: WindowHandle) -> Option<PersistentWorkspaceId> {
    self.by_workspace.iter().find_map(|(workspace, strip)| {
      strip
        .members
        .iter()
        .any(|member| member.handle == handle)
        .then_some(*workspace)
    })
  }

  /// Returns the active [`WindowHandle`] for a given strip, if it exists.
  pub(crate) fn get_active_handle(&self, workspace: PersistentWorkspaceId) -> Option<WindowHandle> {
    self.by_workspace.get(&workspace).and_then(|strip| strip.active_window_handle)
  }

  /// Returns a member's [`WidthPreset`], if it exists.
  pub(crate) fn get_width_preset(&self, workspace: PersistentWorkspaceId, handle: WindowHandle) -> Option<WidthPreset> {
    self
      .by_workspace
      .get(&workspace)?
      .members
      .iter()
      .find(|member| member.handle == handle)
      .map(|member| member.width_preset)
  }

  /// Sets a member's [`WidthPreset`] on a given workspace.
  pub(crate) fn set_width_preset(
    &mut self,
    workspace: PersistentWorkspaceId,
    handle: WindowHandle,
    preset: WidthPreset,
  ) -> bool {
    let Some(member) = self
      .by_workspace
      .get_mut(&workspace)
      .and_then(|strip| strip.members.iter_mut().find(|member| member.handle == handle))
    else {
      return false;
    };
    let changed = member.width_preset != preset;
    member.width_preset = preset;
    changed
  }

  /// Narrows or widens a member.
  pub(crate) fn resize(&mut self, workspace: PersistentWorkspaceId, handle: WindowHandle, direction: Direction) -> bool {
    let Some(preset) = self.get_width_preset(workspace, handle) else {
      return false;
    };
    let target = match direction {
      Direction::Left => preset.narrower(),
      Direction::Right => preset.wider(),
      Direction::Up | Direction::Down => return false,
    };
    self.set_width_preset(workspace, handle, target)
  }

  /// Focuses a member when present.
  pub(crate) fn set_active(&mut self, workspace: PersistentWorkspaceId, handle: WindowHandle) -> bool {
    let Some(strip) = self.by_workspace.get_mut(&workspace) else {
      return false;
    };
    if !strip.members.iter().any(|member| member.handle == handle) {
      return false;
    }
    strip.active_window_handle = Some(handle);
    true
  }

  /// Focuses an adjacent member without wrapping.
  pub(crate) fn set_adjacent_active(
    &mut self,
    workspace: PersistentWorkspaceId,
    handle: WindowHandle,
    direction: Direction,
  ) -> Option<WindowHandle> {
    let strip = self.by_workspace.get_mut(&workspace)?;
    let index = strip.members.iter().position(|member| member.handle == handle)?;
    let target = match direction {
      Direction::Left => index.checked_sub(1),
      Direction::Right if index + 1 < strip.members.len() => Some(index + 1),
      Direction::Right | Direction::Up | Direction::Down => None,
    }?;
    let handle = strip.members[target].handle;
    strip.active_window_handle = Some(handle);
    Some(handle)
  }

  /// Swaps a member with its horizontal neighbour.
  pub(crate) fn reorder(&mut self, workspace: PersistentWorkspaceId, handle: WindowHandle, direction: Direction) -> bool {
    let Some(strip) = self.by_workspace.get_mut(&workspace) else {
      return false;
    };
    let Some(index) = strip.members.iter().position(|member| member.handle == handle) else {
      return false;
    };
    let target = match direction {
      Direction::Left => index.checked_sub(1),
      Direction::Right if index + 1 < strip.members.len() => Some(index + 1),
      Direction::Right | Direction::Up | Direction::Down => None,
    };
    let Some(target) = target else {
      return false;
    };
    strip.members.swap(index, target);
    strip.active_window_handle = Some(handle);
    true
  }

  /// Removes a member and selects its right neighbour, then left neighbour.
  pub(crate) fn remove(&mut self, workspace: PersistentWorkspaceId, handle: WindowHandle) -> Option<WidthPreset> {
    let strip = self.by_workspace.get_mut(&workspace)?;
    let index = strip.members.iter().position(|member| member.handle == handle)?;
    let removed = strip.members.remove(index);
    if strip.active_window_handle == Some(handle) {
      strip.active_window_handle = strip
        .members
        .get(index)
        .or_else(|| index.checked_sub(1).and_then(|i| strip.members.get(i)))
        .map(|member| member.handle);
    }
    Some(removed.width_preset)
  }

  /// Removes members absent from the supplied ordered set.
  pub(crate) fn retain(&mut self, workspace: PersistentWorkspaceId, handles_to_retain: &[WindowHandle]) {
    let Some(strip) = self.by_workspace.get_mut(&workspace) else {
      return;
    };
    let old_focus_index = strip
      .active_window_handle
      .and_then(|focused| strip.members.iter().position(|member| member.handle == focused))
      .unwrap_or(0);
    strip.members.retain(|member| handles_to_retain.contains(&member.handle));
    if strip
      .active_window_handle
      .is_none_or(|focused| !strip.members.iter().any(|member| member.handle == focused))
    {
      strip.active_window_handle = strip
        .members
        .get(old_focus_index)
        .or_else(|| strip.members.last())
        .map(|member| member.handle);
    }
  }

  /// Calculates centred, variable-width strip placements. This method:
  ///  1. Finds the workspace’s strip.
  ///  2. Uses the active member as the anchor, falling back to the first member.
  ///  3. Calculates near-maximised vertical position/height.
  ///  4. Converts each member’s width preset into pixels relative to the monitor’s usable width.
  ///  5. Centres the focused member in the monitor work area.
  ///  6. Places left neighbours right-to-left, each separated by one margin.
  ///  7. Places right neighbours left-to-right using the same gap.
  ///  8. Returns [`WindowHandle`]-[`Sizing`] pairs in strip order.
  pub(crate) fn placements(
    &self,
    workspace: PersistentWorkspaceId,
    work_area: Rect,
    margin: i32,
  ) -> Vec<(WindowHandle, Sizing)> {
    let Some(strip) = self.by_workspace.get(&workspace) else {
      return Vec::new();
    };

    // Use the stored focus as the strip's anchor, falling back to its first member
    let Some(focused_index) = strip
      .active_window_handle
      .and_then(|focused| strip.members.iter().position(|member| member.handle == focused))
      .or_else(|| (!strip.members.is_empty()).then_some(0))
    else {
      return Vec::new();
    };

    let near_maximised = Sizing::near_maximised(work_area, margin);
    let usable_width = near_maximised.width.max(1);

    // Resolve every preset against this monitor; horizontal positions are filled below
    let mut placements = strip
      .members
      .iter()
      .map(|member| {
        Sizing::new(
          0,
          near_maximised.y,
          member.width_preset.width(usable_width),
          near_maximised.height,
        )
      })
      .collect::<Vec<_>>();

    // Centre the focused member in the full work area
    let focused_width = placements[focused_index].width;
    placements[focused_index].x = clamp_x(
      work_area
        .left
        .saturating_add(work_area.width().saturating_sub(focused_width) / 2),
      focused_width,
    );

    // Walk outwards from the anchor, placing each neighbour one margin beyond the adjacent edge
    for index in (0..focused_index).rev() {
      let right = placements[index + 1].x.saturating_sub(margin);
      placements[index].x = clamp_x(right.saturating_sub(placements[index].width), placements[index].width);
    }
    for index in focused_index + 1..placements.len() {
      let left = placements[index - 1]
        .x
        .saturating_add(placements[index - 1].width)
        .saturating_add(margin);
      placements[index].x = clamp_x(left, placements[index].width);
    }

    // Restore the member handles while preserving strip order
    strip
      .members
      .iter()
      .zip(placements)
      .map(|(member, sizing)| (member.handle, sizing))
      .collect()
  }
}

fn clamp_x(x: i32, width: i32) -> i32 {
  x.min(i32::MAX.saturating_sub(width.max(0)))
}
