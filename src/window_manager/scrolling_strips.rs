use crate::common::{Direction, PersistentWorkspaceId, Rect, Sizing, WindowHandle};
use std::collections::HashMap;

const WIDTH_PRESETS: [WidthPreset; 6] = [
  WidthPreset::Quarter,
  WidthPreset::Third,
  WidthPreset::Half,
  WidthPreset::TwoThirds,
  WidthPreset::ThreeQuarters,
  WidthPreset::NearMaximised,
];

/// A monitor-relative scrolling layout width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WidthPreset {
  Quarter,
  Third,
  Half,
  TwoThirds,
  ThreeQuarters,
  NearMaximised,
}

impl WidthPreset {
  /// Selects the preset closest to an observed pixel width.
  pub(crate) fn nearest(observed_width: i32, usable_width: i32) -> Self {
    WIDTH_PRESETS
      .into_iter()
      .min_by_key(|preset| preset.width(usable_width).abs_diff(observed_width))
      .unwrap_or(Self::NearMaximised)
  }

  /// Returns the next narrower preset, clamped at one quarter.
  pub(crate) fn narrower(self) -> Self {
    let index = WIDTH_PRESETS.iter().position(|preset| *preset == self).unwrap_or(0);
    WIDTH_PRESETS[index.saturating_sub(1)]
  }

  /// Returns the next wider preset, clamped at near-maximised.
  pub(crate) fn wider(self) -> Self {
    let index = WIDTH_PRESETS.iter().position(|preset| *preset == self).unwrap_or(0);
    WIDTH_PRESETS[(index + 1).min(WIDTH_PRESETS.len() - 1)]
  }

  /// Calculates this preset's pixel width.
  pub(crate) fn width(self, usable_width: i32) -> i32 {
    match self {
      Self::Quarter => usable_width / 4,
      Self::Third => usable_width / 3,
      Self::Half => usable_width / 2,
      Self::TwoThirds => fraction_width(usable_width, 2, 3),
      Self::ThreeQuarters => fraction_width(usable_width, 3, 4),
      Self::NearMaximised => usable_width,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Member {
  handle: WindowHandle,
  preset: WidthPreset,
}

/// Stores ordered scrolling strip membership and focus for each workspace. Must only be used by
/// [`crate::window_manager::scrolling_layout::ScrollingLayout`].
#[derive(Debug, Default)]
pub(crate) struct ScrollingStrips {
  by_workspace: HashMap<PersistentWorkspaceId, Strip>,
}

/// Stores ordered members and current focus for one workspace.
#[derive(Debug, Default)]
struct Strip {
  members: Vec<Member>,
  focused: Option<WindowHandle>,
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
      .map(|(handle, preset)| Member { handle, preset })
      .collect::<Vec<_>>();
    let focused = focused
      .filter(|handle| members.iter().any(|member| member.handle == *handle))
      .or_else(|| members.first().map(|member| member.handle));
    self.by_workspace.insert(workspace, Strip { members, focused });
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
    strip.members.insert(index, Member { handle, preset });
    strip.focused = Some(handle);
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
        .map(|member| (member.handle, member.preset))
        .collect()
    })
  }

  /// Returns the workspace containing a member.
  pub(crate) fn workspace_containing(&self, handle: WindowHandle) -> Option<PersistentWorkspaceId> {
    self.by_workspace.iter().find_map(|(workspace, strip)| {
      strip
        .members
        .iter()
        .any(|member| member.handle == handle)
        .then_some(*workspace)
    })
  }

  /// Returns the focused member.
  pub(crate) fn focused(&self, workspace: PersistentWorkspaceId) -> Option<WindowHandle> {
    self.by_workspace.get(&workspace).and_then(|strip| strip.focused)
  }

  /// Returns a member's preset.
  pub(crate) fn preset(&self, workspace: PersistentWorkspaceId, handle: WindowHandle) -> Option<WidthPreset> {
    self
      .by_workspace
      .get(&workspace)?
      .members
      .iter()
      .find(|member| member.handle == handle)
      .map(|member| member.preset)
  }

  /// Sets a member's preset.
  pub(crate) fn set_preset(&mut self, workspace: PersistentWorkspaceId, handle: WindowHandle, preset: WidthPreset) -> bool {
    let Some(member) = self
      .by_workspace
      .get_mut(&workspace)
      .and_then(|strip| strip.members.iter_mut().find(|member| member.handle == handle))
    else {
      return false;
    };
    let changed = member.preset != preset;
    member.preset = preset;
    changed
  }

  /// Narrows or widens a member.
  pub(crate) fn resize(&mut self, workspace: PersistentWorkspaceId, handle: WindowHandle, direction: Direction) -> bool {
    let Some(preset) = self.preset(workspace, handle) else {
      return false;
    };
    let target = match direction {
      Direction::Left => preset.narrower(),
      Direction::Right => preset.wider(),
      Direction::Up | Direction::Down => return false,
    };
    self.set_preset(workspace, handle, target)
  }

  /// Focuses a member when present.
  pub(crate) fn set_focused(&mut self, workspace: PersistentWorkspaceId, handle: WindowHandle) -> bool {
    let Some(strip) = self.by_workspace.get_mut(&workspace) else {
      return false;
    };
    if !strip.members.iter().any(|member| member.handle == handle) {
      return false;
    }
    strip.focused = Some(handle);
    true
  }

  /// Focuses an adjacent member without wrapping.
  pub(crate) fn focus_adjacent(
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
    strip.focused = Some(handle);
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
    strip.focused = Some(handle);
    true
  }

  /// Removes a member and selects its right neighbour, then left neighbour.
  pub(crate) fn remove(&mut self, workspace: PersistentWorkspaceId, handle: WindowHandle) -> Option<WidthPreset> {
    let strip = self.by_workspace.get_mut(&workspace)?;
    let index = strip.members.iter().position(|member| member.handle == handle)?;
    let removed = strip.members.remove(index);
    if strip.focused == Some(handle) {
      strip.focused = strip
        .members
        .get(index)
        .or_else(|| index.checked_sub(1).and_then(|i| strip.members.get(i)))
        .map(|member| member.handle);
    }
    Some(removed.preset)
  }

  /// Removes members absent from the supplied ordered set.
  pub(crate) fn retain(&mut self, workspace: PersistentWorkspaceId, handles_to_retain: &[WindowHandle]) {
    let Some(strip) = self.by_workspace.get_mut(&workspace) else {
      return;
    };
    let old_focus_index = strip
      .focused
      .and_then(|focused| strip.members.iter().position(|member| member.handle == focused))
      .unwrap_or(0);
    strip.members.retain(|member| handles_to_retain.contains(&member.handle));
    if strip
      .focused
      .is_none_or(|focused| !strip.members.iter().any(|member| member.handle == focused))
    {
      strip.focused = strip
        .members
        .get(old_focus_index)
        .or_else(|| strip.members.last())
        .map(|member| member.handle);
    }
  }

  /// Calculates centred, variable-width strip placements. This method:
  ///  1. Finds the workspace’s strip.
  ///  2. Uses the focused member as the anchor, falling back to the first member.
  ///  3. Calculates near-maximised vertical position/height.
  ///  4. Converts each member’s width preset into pixels relative to the monitor’s usable width.
  ///  5. Centres the focused member in the monitor work area.
  ///  6. Places left neighbours right-to-left, each separated by one margin.
  ///  7. Places right neighbours left-to-right using the same gap.
  ///  8. Returns (WindowHandle, Sizing) pairs in strip order.
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
      .focused
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
      .map(|member| Sizing::new(0, near_maximised.y, member.preset.width(usable_width), near_maximised.height))
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

fn fraction_width(usable_width: i32, numerator: i64, denominator: i64) -> i32 {
  let width = i64::from(usable_width) * numerator / denominator;
  i32::try_from(width).unwrap_or(if width.is_negative() { i32::MIN } else { i32::MAX })
}

fn clamp_x(x: i32, width: i32) -> i32 {
  x.min(i32::MAX.saturating_sub(width.max(0)))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn workspace(number: usize) -> PersistentWorkspaceId {
    PersistentWorkspaceId::new([number as u16; 32], number, number == 1)
  }

  fn members(handles: &[i32]) -> Vec<(WindowHandle, WidthPreset)> {
    handles.iter().map(|handle| ((*handle).into(), WidthPreset::Half)).collect()
  }

  #[test]
  fn adopt_adopts_windows_in_supplied_order_and_focuses_requested_member() {
    let mut strips = ScrollingStrips::default();
    let id = workspace(1);

    strips.adopt(id, members(&[1, 2, 3]), Some(2.into()));

    assert_eq!(strips.members(id), vec![1.into(), 2.into(), 3.into()]);
    assert_eq!(strips.focused(id), Some(2.into()));
  }

  #[test]
  fn insert_before_inserts_new_window_before_previous_active_and_focuses_it() {
    let mut strips = ScrollingStrips::default();
    let id = workspace(1);
    strips.adopt(id, members(&[1, 2, 3]), Some(2.into()));

    strips.insert_before(id, 4.into(), WidthPreset::Quarter, Some(2.into()));

    assert_eq!(strips.members(id), vec![1.into(), 4.into(), 2.into(), 3.into()]);
    assert_eq!(strips.focused(id), Some(4.into()));
    assert_eq!(strips.preset(id, 4.into()), Some(WidthPreset::Quarter));
  }

  #[test]
  fn insert_before_keeps_strips_independent_between_workspaces() {
    let mut strips = ScrollingStrips::default();
    let first = workspace(1);
    let second = workspace(2);
    strips.adopt(first, members(&[1]), Some(1.into()));
    strips.adopt(second, members(&[2]), Some(2.into()));

    strips.insert_before(first, 3.into(), WidthPreset::Half, Some(1.into()));

    assert_eq!(strips.members(first), vec![3.into(), 1.into()]);
    assert_eq!(strips.members(second), vec![2.into()]);
  }

  #[test]
  fn focuses_adjacent_members_without_wrapping() {
    let mut strips = ScrollingStrips::default();
    let id = workspace(1);
    strips.adopt(id, members(&[1, 2, 3]), Some(2.into()));

    assert_eq!(strips.focus_adjacent(id, 2.into(), Direction::Right), Some(3.into()));
    assert_eq!(strips.focus_adjacent(id, 3.into(), Direction::Right), None);
    assert_eq!(strips.focus_adjacent(id, 3.into(), Direction::Left), Some(2.into()));
  }

  #[test]
  fn reorders_focused_member_with_adjacent_member() {
    let mut strips = ScrollingStrips::default();
    let id = workspace(1);
    strips.adopt(id, members(&[1, 2, 3]), Some(2.into()));

    assert!(strips.reorder(id, 2.into(), Direction::Right));
    assert_eq!(strips.members(id), vec![1.into(), 3.into(), 2.into()]);
    assert_eq!(strips.focused(id), Some(2.into()));
    assert!(!strips.reorder(id, 2.into(), Direction::Right));
  }

  #[test]
  fn removal_focuses_right_neighbour_then_left_neighbour_and_returns_preset() {
    let mut strips = ScrollingStrips::default();
    let id = workspace(1);
    strips.adopt(id, members(&[1, 2, 3]), Some(2.into()));

    assert_eq!(strips.remove(id, 2.into()), Some(WidthPreset::Half));
    assert_eq!(strips.focused(id), Some(3.into()));
    strips.remove(id, 3.into());
    assert_eq!(strips.focused(id), Some(1.into()));
    strips.remove(id, 1.into());
    assert_eq!(strips.focused(id), None);
  }

  #[test]
  fn reconciliation_removes_stale_members_and_preserves_order() {
    let mut strips = ScrollingStrips::default();
    let id = workspace(1);
    strips.adopt(id, members(&[1, 2, 3]), Some(2.into()));

    strips.retain(id, &[1.into(), 3.into()]);

    assert_eq!(strips.members(id), vec![1.into(), 3.into()]);
    assert_eq!(strips.focused(id), Some(3.into()));
  }

  #[test]
  fn width_preset_nearest_selects_nearest_monitor_relative_width_preset() {
    assert_eq!(WidthPreset::nearest(245, 980), WidthPreset::Quarter);
    assert_eq!(WidthPreset::nearest(500, 980), WidthPreset::Half);
    assert_eq!(WidthPreset::nearest(970, 980), WidthPreset::NearMaximised);
  }

  #[test]
  fn width_preset_narrower_traverses_width_presets_without_wrapping() {
    assert_eq!(WidthPreset::Quarter.narrower(), WidthPreset::Quarter);
    assert_eq!(WidthPreset::Half.narrower(), WidthPreset::Third);
    assert_eq!(WidthPreset::Half.wider(), WidthPreset::TwoThirds);
    assert_eq!(WidthPreset::NearMaximised.wider(), WidthPreset::NearMaximised);
  }

  #[test]
  fn width_preset_width_calculates_fractional_widths_without_intermediate_overflow() {
    assert_eq!(WidthPreset::TwoThirds.width(i32::MAX), 1_431_655_764);
    assert_eq!(WidthPreset::ThreeQuarters.width(i32::MAX), 1_610_612_735);
  }

  #[test]
  fn placements_centre_focus_and_accumulate_variable_width_neighbours_with_one_margin() {
    let mut strips = ScrollingStrips::default();
    let id = workspace(1);
    strips.adopt(
      id,
      vec![
        (1.into(), WidthPreset::Quarter),
        (2.into(), WidthPreset::Half),
        (3.into(), WidthPreset::ThreeQuarters),
      ],
      Some(2.into()),
    );

    let placements = strips.placements(id, Rect::new(100, 20, 1100, 720), 10);

    assert_eq!(placements[0], (1.into(), Sizing::new(100, 30, 245, 680)));
    assert_eq!(placements[1], (2.into(), Sizing::new(355, 30, 490, 680)));
    assert_eq!(placements[2], (3.into(), Sizing::new(855, 30, 735, 680)));
  }

  #[test]
  fn placements_use_overflow_safe_coordinates() {
    let mut strips = ScrollingStrips::default();
    let id = workspace(1);
    strips.adopt(
      id,
      vec![(1.into(), WidthPreset::NearMaximised), (2.into(), WidthPreset::NearMaximised)],
      Some(1.into()),
    );

    let placements = strips.placements(id, Rect::new(i32::MAX - 100, 0, i32::MAX, 100), 0);

    assert!(placements.iter().all(|(_, sizing)| sizing.x <= i32::MAX - sizing.width));
  }
}
