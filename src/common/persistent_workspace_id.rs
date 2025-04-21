use crate::common::TransientWorkspaceId;
use crate::utils::id_to_string_or_panic;
use std::fmt::Display;

#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct PersistentWorkspaceId {
  pub monitor_id: [u16; 32],
  pub workspace: usize,
}

impl PersistentWorkspaceId {
  pub fn new(monitor_id: [u16; 32], workspace: usize) -> Self {
    PersistentWorkspaceId { monitor_id, workspace }
  }

  pub fn id_to_string(&self) -> String {
    id_to_string_or_panic(&self.monitor_id)
  }

  #[allow(unused)]
  pub fn is_same_monitor(&self, other: &Self) -> bool {
    self.monitor_id == other.monitor_id
  }

  pub fn is_same_workspace(&self, other: &Self) -> bool {
    self.workspace == other.workspace
  }
}

impl From<TransientWorkspaceId> for PersistentWorkspaceId {
  fn from(value: TransientWorkspaceId) -> Self {
    PersistentWorkspaceId {
      monitor_id: value.monitor_id,
      workspace: value.workspace,
    }
  }
}

impl Display for PersistentWorkspaceId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "wsp#{}-{}", self.id_to_string(), self.workspace)
  }
}

#[cfg(test)]
mod tests {
  use crate::common::{PersistentWorkspaceId, TransientWorkspaceId};

  impl PersistentWorkspaceId {
    pub fn new_test(workspace: usize) -> Self {
      PersistentWorkspaceId {
        monitor_id: "P_DISPLAY"
          .as_bytes()
          .iter()
          .map(|&b| b as u16)
          .chain(std::iter::repeat(0).take(32 - "P_DISPLAY".len()))
          .collect::<Vec<u16>>()
          .try_into()
          .unwrap(),
        workspace,
      }
    }
  }

  #[test]
  fn permanent_workspace_id_displays_id_as_string() {
    let id = PersistentWorkspaceId::new_test(1);

    assert_eq!(id.workspace, 1);
    assert_eq!(id.id_to_string(), "P_DISPLAY");
  }

  #[test]
  fn from_transient_workspace_id_creates_correct_permanent_workspace_id() {
    let transient_id = TransientWorkspaceId {
      monitor_id: [1; 32],
      monitor_handle: 1.into(),
      workspace: 42,
    };

    let permanent_id: PersistentWorkspaceId = transient_id.into();

    assert_eq!(permanent_id.monitor_id, [1; 32]);
    assert_eq!(permanent_id.workspace, 42);
  }

  #[test]
  fn display_formats_permanent_workspace_id_correctly() {
    let id = PersistentWorkspaceId::new_test(3);

    assert_eq!(id.to_string(), "wsp#P_DISPLAY-3");
  }

  #[test]
  #[should_panic(expected = "Failed to convert id to string")]
  fn id_to_string_panics_on_empty_id() {
    PersistentWorkspaceId::new([0; 32], 1).id_to_string();
  }

  #[test]
  fn workspace_id_same_monitor_returns_true() {
    let id1 = PersistentWorkspaceId::new([1; 32], 1);
    let id2 = PersistentWorkspaceId::new([1; 32], 2);

    assert!(id1.is_same_monitor(&id2));
  }

  #[test]
  fn workspace_id_different_monitor_returns_false() {
    let id1 = PersistentWorkspaceId::new([1; 32], 1);
    let id2 = PersistentWorkspaceId::new([2; 32], 1);

    assert!(!id1.is_same_monitor(&id2));
  }

  #[test]
  fn workspace_id_same_workspace_returns_true() {
    let id1 = PersistentWorkspaceId::new([1; 32], 1);
    let id2 = PersistentWorkspaceId::new([2; 32], 1);

    assert!(id1.is_same_workspace(&id2));
  }

  #[test]
  fn workspace_id_different_workspace_returns_false() {
    let id1 = PersistentWorkspaceId::new([1; 32], 1);
    let id2 = PersistentWorkspaceId::new([1; 32], 2);

    assert!(!id1.is_same_workspace(&id2));
  }
}
