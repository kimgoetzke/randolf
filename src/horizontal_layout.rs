use crate::common::{Direction, PersistentWorkspaceId, Rect, Sizing, WindowHandle};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub(crate) struct HorizontalLayout {
  strips: HashMap<PersistentWorkspaceId, Strip>,
}

#[derive(Debug, Default)]
struct Strip {
  members: Vec<WindowHandle>,
  focused: Option<WindowHandle>,
}

impl HorizontalLayout {
  /// Replaces a workspace strip with ordered members.
  pub(crate) fn adopt(
    &mut self,
    workspace: PersistentWorkspaceId,
    members: Vec<WindowHandle>,
    focused: Option<WindowHandle>,
  ) {
    let focused = focused
      .filter(|handle| members.contains(handle))
      .or_else(|| members.first().copied());
    self.strips.insert(workspace, Strip { members, focused });
  }

  /// Inserts a member immediately before the previous active member.
  pub(crate) fn insert_before(
    &mut self,
    workspace: PersistentWorkspaceId,
    member: WindowHandle,
    previous_active: Option<WindowHandle>,
  ) {
    let strip = self.strips.entry(workspace).or_default();
    strip.members.retain(|handle| *handle != member);
    let index = previous_active
      .and_then(|handle| strip.members.iter().position(|candidate| *candidate == handle))
      .unwrap_or(strip.members.len());
    strip.members.insert(index, member);
    strip.focused = Some(member);
  }

  /// Returns ordered members for a workspace.
  pub(crate) fn members(&self, workspace: PersistentWorkspaceId) -> &[WindowHandle] {
    self.strips.get(&workspace).map_or(&[], |strip| strip.members.as_slice())
  }

  /// Returns IDs for all tracked workspaces.
  pub(crate) fn workspace_ids(&self) -> impl Iterator<Item = PersistentWorkspaceId> + '_ {
    self.strips.keys().copied()
  }

  /// Returns the workspace containing a member.
  pub(crate) fn workspace_containing(&self, member: WindowHandle) -> Option<PersistentWorkspaceId> {
    self
      .strips
      .iter()
      .find_map(|(workspace, strip)| strip.members.contains(&member).then_some(*workspace))
  }

  /// Returns the focused member.
  pub(crate) fn focused(&self, workspace: PersistentWorkspaceId) -> Option<WindowHandle> {
    self.strips.get(&workspace).and_then(|strip| strip.focused)
  }

  /// Focuses a member when present.
  pub(crate) fn set_focused(&mut self, workspace: PersistentWorkspaceId, member: WindowHandle) -> bool {
    let Some(strip) = self.strips.get_mut(&workspace) else {
      return false;
    };
    if !strip.members.contains(&member) {
      return false;
    }
    strip.focused = Some(member);
    true
  }

  /// Focuses an adjacent member without wrapping.
  pub(crate) fn focus_adjacent(
    &mut self,
    workspace: PersistentWorkspaceId,
    member: WindowHandle,
    direction: Direction,
  ) -> Option<WindowHandle> {
    let strip = self.strips.get_mut(&workspace)?;
    let index = strip.members.iter().position(|candidate| *candidate == member)?;
    let target = match direction {
      Direction::Left => index.checked_sub(1),
      Direction::Right if index + 1 < strip.members.len() => Some(index + 1),
      Direction::Right | Direction::Up | Direction::Down => None,
    }?;
    let member = strip.members[target];
    strip.focused = Some(member);
    Some(member)
  }

  /// Swaps a member with its horizontal neighbour.
  pub(crate) fn reorder(&mut self, workspace: PersistentWorkspaceId, member: WindowHandle, direction: Direction) -> bool {
    let Some(strip) = self.strips.get_mut(&workspace) else {
      return false;
    };
    let Some(index) = strip.members.iter().position(|candidate| *candidate == member) else {
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
    strip.focused = Some(member);
    true
  }

  /// Removes a member and selects its right neighbour, then left neighbour.
  pub(crate) fn remove(&mut self, workspace: PersistentWorkspaceId, member: WindowHandle) -> Option<WindowHandle> {
    let strip = self.strips.get_mut(&workspace)?;
    let index = strip.members.iter().position(|candidate| *candidate == member)?;
    strip.members.remove(index);
    let focused = strip
      .members
      .get(index)
      .or_else(|| index.checked_sub(1).and_then(|i| strip.members.get(i)))
      .copied();
    if strip.focused == Some(member) {
      strip.focused = focused;
    }
    focused
  }

  /// Removes members absent from the supplied ordered set.
  pub(crate) fn retain(&mut self, workspace: PersistentWorkspaceId, valid: &[WindowHandle]) {
    let Some(strip) = self.strips.get_mut(&workspace) else {
      return;
    };
    let old_focus_index = strip
      .focused
      .and_then(|focused| strip.members.iter().position(|member| *member == focused))
      .unwrap_or(0);
    strip.members.retain(|member| valid.contains(member));
    if strip.focused.is_none_or(|focused| !strip.members.contains(&focused)) {
      strip.focused = strip.members.get(old_focus_index).or_else(|| strip.members.last()).copied();
    }
  }

  /// Calculates translated near-maximised placements.
  pub(crate) fn placements(
    &self,
    workspace: PersistentWorkspaceId,
    work_area: Rect,
    margin: i32,
  ) -> Vec<(WindowHandle, Sizing)> {
    let Some(strip) = self.strips.get(&workspace) else {
      return Vec::new();
    };
    let focused_index = strip
      .focused
      .and_then(|focused| strip.members.iter().position(|member| *member == focused))
      .unwrap_or(0);
    let anchor = Sizing::near_maximised(work_area, margin);
    let stride = work_area.width();
    strip
      .members
      .iter()
      .enumerate()
      .map(|(index, handle)| {
        let delta = i32::try_from(index).unwrap_or(i32::MAX) - i32::try_from(focused_index).unwrap_or(i32::MAX);
        let mut sizing = anchor.clone();
        sizing.x = sizing.x.saturating_add(stride.saturating_mul(delta));
        (*handle, sizing)
      })
      .collect()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn workspace(number: usize) -> PersistentWorkspaceId {
    PersistentWorkspaceId::new([number as u16; 32], number, number == 1)
  }

  #[test]
  fn adopts_windows_in_supplied_order_and_focuses_requested_member() {
    let mut layout = HorizontalLayout::default();
    let id = workspace(1);

    layout.adopt(id, vec![1.into(), 2.into(), 3.into()], Some(2.into()));

    assert_eq!(layout.members(id), &[1.into(), 2.into(), 3.into()]);
    assert_eq!(layout.focused(id), Some(2.into()));
  }

  #[test]
  fn inserts_new_window_before_previous_active_and_focuses_it() {
    let mut layout = HorizontalLayout::default();
    let id = workspace(1);
    layout.adopt(id, vec![1.into(), 2.into(), 3.into()], Some(2.into()));

    layout.insert_before(id, 4.into(), Some(2.into()));

    assert_eq!(layout.members(id), &[1.into(), 4.into(), 2.into(), 3.into()]);
    assert_eq!(layout.focused(id), Some(4.into()));
  }

  #[test]
  fn keeps_strips_independent_between_workspaces() {
    let mut layout = HorizontalLayout::default();
    let first = workspace(1);
    let second = workspace(2);
    layout.adopt(first, vec![1.into()], Some(1.into()));
    layout.adopt(second, vec![2.into()], Some(2.into()));

    layout.insert_before(first, 3.into(), Some(1.into()));

    assert_eq!(layout.members(first), &[3.into(), 1.into()]);
    assert_eq!(layout.members(second), &[2.into()]);
  }

  #[test]
  fn focuses_adjacent_members_without_wrapping() {
    let mut layout = HorizontalLayout::default();
    let id = workspace(1);
    layout.adopt(id, vec![1.into(), 2.into(), 3.into()], Some(2.into()));

    assert_eq!(layout.focus_adjacent(id, 2.into(), Direction::Right), Some(3.into()));
    assert_eq!(layout.focus_adjacent(id, 3.into(), Direction::Right), None);
    assert_eq!(layout.focus_adjacent(id, 3.into(), Direction::Left), Some(2.into()));
  }

  #[test]
  fn reorders_focused_member_with_adjacent_member() {
    let mut layout = HorizontalLayout::default();
    let id = workspace(1);
    layout.adopt(id, vec![1.into(), 2.into(), 3.into()], Some(2.into()));

    assert!(layout.reorder(id, 2.into(), Direction::Right));
    assert_eq!(layout.members(id), &[1.into(), 3.into(), 2.into()]);
    assert_eq!(layout.focused(id), Some(2.into()));
    assert!(!layout.reorder(id, 2.into(), Direction::Right));
  }

  #[test]
  fn removal_focuses_right_neighbour_then_left_neighbour() {
    let mut layout = HorizontalLayout::default();
    let id = workspace(1);
    layout.adopt(id, vec![1.into(), 2.into(), 3.into()], Some(2.into()));

    assert_eq!(layout.remove(id, 2.into()), Some(3.into()));
    assert_eq!(layout.remove(id, 3.into()), Some(1.into()));
    assert_eq!(layout.remove(id, 1.into()), None);
  }

  #[test]
  fn reconciliation_removes_stale_members_and_preserves_order() {
    let mut layout = HorizontalLayout::default();
    let id = workspace(1);
    layout.adopt(id, vec![1.into(), 2.into(), 3.into()], Some(2.into()));

    layout.retain(id, &[1.into(), 3.into()]);

    assert_eq!(layout.members(id), &[1.into(), 3.into()]);
    assert_eq!(layout.focused(id), Some(3.into()));
  }

  #[test]
  fn placements_translate_one_work_area_width_per_member() {
    let mut layout = HorizontalLayout::default();
    let id = workspace(1);
    layout.adopt(id, vec![1.into(), 2.into(), 3.into()], Some(2.into()));

    let placements = layout.placements(id, Rect::new(100, 20, 1100, 720), 10);

    assert_eq!(placements[0], (1.into(), Sizing::new(-890, 30, 980, 680)));
    assert_eq!(placements[1], (2.into(), Sizing::new(110, 30, 980, 680)));
    assert_eq!(placements[2], (3.into(), Sizing::new(1110, 30, 980, 680)));
  }
}
