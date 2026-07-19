use crate::common::{Direction, PersistentWorkspaceId, Rect, Sizing, WidthPreset, WindowHandle};
use crate::window_manager::scrolling_strips::ScrollingStrips;

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
  assert_eq!(strips.get_active_handle(id), Some(2.into()));
}

#[test]
fn insert_before_inserts_new_window_before_previous_active_and_focuses_it() {
  let mut strips = ScrollingStrips::default();
  let id = workspace(1);
  strips.adopt(id, members(&[1, 2, 3]), Some(2.into()));

  strips.insert_before(id, 4.into(), WidthPreset::Quarter, Some(2.into()));

  assert_eq!(strips.members(id), vec![1.into(), 4.into(), 2.into(), 3.into()]);
  assert_eq!(strips.get_active_handle(id), Some(4.into()));
  assert_eq!(strips.get_width_preset(id, 4.into()), Some(WidthPreset::Quarter));
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
fn focus_adjacent_focuses_adjacent_members_without_wrapping() {
  let mut strips = ScrollingStrips::default();
  let id = workspace(1);
  strips.adopt(id, members(&[1, 2, 3]), Some(2.into()));

  assert_eq!(strips.set_adjacent_active(id, 2.into(), Direction::Right), Some(3.into()));
  assert_eq!(strips.set_adjacent_active(id, 3.into(), Direction::Right), None);
  assert_eq!(strips.set_adjacent_active(id, 3.into(), Direction::Left), Some(2.into()));
}

#[test]
fn reorders_focused_member_with_adjacent_member() {
  let mut strips = ScrollingStrips::default();
  let id = workspace(1);
  strips.adopt(id, members(&[1, 2, 3]), Some(2.into()));

  assert!(strips.reorder(id, 2.into(), Direction::Right));
  assert_eq!(strips.members(id), vec![1.into(), 3.into(), 2.into()]);
  assert_eq!(strips.get_active_handle(id), Some(2.into()));
  assert!(!strips.reorder(id, 2.into(), Direction::Right));
}

#[test]
fn remove_shifts_focus_on_right_neighbour_then_left_neighbour_and_returns_preset() {
  let mut strips = ScrollingStrips::default();
  let id = workspace(1);
  strips.adopt(id, members(&[1, 2, 3]), Some(2.into()));

  assert_eq!(strips.remove(id, 2.into()), Some(WidthPreset::Half));
  assert_eq!(strips.get_active_handle(id), Some(3.into()));
  strips.remove(id, 3.into());
  assert_eq!(strips.get_active_handle(id), Some(1.into()));
  strips.remove(id, 1.into());
  assert_eq!(strips.get_active_handle(id), None);
}

#[test]
fn reconciliation_removes_stale_members_and_preserves_order() {
  let mut strips = ScrollingStrips::default();
  let id = workspace(1);
  strips.adopt(id, members(&[1, 2, 3]), Some(2.into()));

  strips.retain(id, &[1.into(), 3.into()]);

  assert_eq!(strips.members(id), vec![1.into(), 3.into()]);
  assert_eq!(strips.get_active_handle(id), Some(3.into()));
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
