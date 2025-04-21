use crate::utils::{MonitorHandle, PersistentWorkspaceId};
use std::fmt::Display;

#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct TransientWorkspaceId {
  pub monitor_id: [u16; 32],
  pub monitor_handle: MonitorHandle,
  pub workspace: usize,
}

impl TransientWorkspaceId {
  pub fn new(monitor_id: [u16; 32], monitor_handle: MonitorHandle, workspace: usize) -> Self {
    TransientWorkspaceId {
      monitor_id,
      monitor_handle,
      workspace,
    }
  }

  pub fn from(persistent_workspace_id: PersistentWorkspaceId, monitor_handle: MonitorHandle) -> Self {
    TransientWorkspaceId {
      monitor_id: persistent_workspace_id.monitor_id,
      monitor_handle,
      workspace: persistent_workspace_id.workspace,
    }
  }

  #[allow(unused)]
  pub fn is_same_monitor(&self, other: &Self) -> bool {
    self.monitor_handle == other.monitor_handle
  }

  pub fn is_same_workspace(&self, other: &Self) -> bool {
    self.workspace == other.workspace
  }
}

impl Display for TransientWorkspaceId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "s#{}-{}", self.monitor_handle.handle, self.workspace)
  }
}

#[cfg(test)]
mod tests {
  use crate::utils::{MonitorHandle, TransientWorkspaceId};

  impl TransientWorkspaceId {
    pub fn from_test(monitor_handle: isize, workspace: usize) -> Self {
      TransientWorkspaceId {
        monitor_id: [monitor_handle as u16; 32],
        monitor_handle: MonitorHandle::from(monitor_handle),
        workspace,
      }
    }

    pub fn new_test(monitor_handle: MonitorHandle, workspace: usize) -> Self {
      TransientWorkspaceId {
        monitor_id: [monitor_handle.handle as u16; 32],
        monitor_handle,
        workspace,
      }
    }
  }

  #[test]
  fn workspace_id_display_handles_negative_values() {
    let handle = MonitorHandle::from(-1);
    let id = TransientWorkspaceId::new_test(handle, 2);

    assert_eq!(id.to_string(), "s#-1-2");
  }

  #[test]
  fn workspace_id_display_handles_large_values() {
    let id = TransientWorkspaceId::from_test(123456789, 987654321);

    assert_eq!(id.to_string(), "s#123456789-987654321");
  }

  #[test]
  fn workspace_id_same_monitor_returns_true() {
    let id1 = TransientWorkspaceId::from_test(1, 1);
    let id2 = TransientWorkspaceId::from_test(1, 2);

    assert!(id1.is_same_monitor(&id2));
  }

  #[test]
  fn workspace_id_different_monitor_returns_false() {
    let id1 = TransientWorkspaceId::from_test(1, 1);
    let id2 = TransientWorkspaceId::from_test(2, 1);

    assert!(!id1.is_same_monitor(&id2));
  }

  #[test]
  fn workspace_id_same_workspace_returns_true() {
    let id1 = TransientWorkspaceId::from_test(1, 1);
    let id2 = TransientWorkspaceId::from_test(1, 1);

    assert!(id1.is_same_workspace(&id2));
  }

  #[test]
  fn workspace_id_different_workspace_returns_false() {
    let id1 = TransientWorkspaceId::from_test(1, 1);
    let id2 = TransientWorkspaceId::from_test(1, 2);

    assert!(!id1.is_same_workspace(&id2));
  }
}
