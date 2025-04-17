use crate::utils::MonitorHandle;
use std::fmt::Display;

#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct WorkspaceId {
  pub monitor_handle: MonitorHandle,
  pub workspace: usize,
}

impl WorkspaceId {
  pub fn new(monitor_handle: MonitorHandle, workspace: usize) -> Self {
    WorkspaceId {
      monitor_handle,
      workspace,
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

impl Display for WorkspaceId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "s#{}-{}", self.monitor_handle.handle, self.workspace)
  }
}

#[cfg(test)]
mod tests {
  use crate::utils::{MonitorHandle, WorkspaceId};

  impl WorkspaceId {
    pub fn from(monitor_handle: isize, workspace: usize) -> Self {
      WorkspaceId {
        monitor_handle: MonitorHandle::from(monitor_handle),
        workspace,
      }
    }
  }

  #[test]
  fn workspace_id_display_handles_negative_values() {
    let id = WorkspaceId::from(-1, 2);

    assert_eq!(id.to_string(), "s#-1-2");
  }

  #[test]
  fn workspace_id_display_handles_large_values() {
    let id = WorkspaceId::from(123456789, 987654321);

    assert_eq!(id.to_string(), "s#123456789-987654321");
  }

  #[test]
  fn workspace_id_same_monitor_returns_true() {
    let id1 = WorkspaceId::from(1, 1);
    let id2 = WorkspaceId::from(1, 2);

    assert!(id1.is_same_monitor(&id2));
  }

  #[test]
  fn workspace_id_different_monitor_returns_false() {
    let id1 = WorkspaceId::from(1, 1);
    let id2 = WorkspaceId::from(2, 1);

    assert!(!id1.is_same_monitor(&id2));
  }

  #[test]
  fn workspace_id_same_workspace_returns_true() {
    let id1 = WorkspaceId::from(1, 1);
    let id2 = WorkspaceId::from(1, 1);

    assert!(id1.is_same_workspace(&id2));
  }

  #[test]
  fn workspace_id_different_workspace_returns_false() {
    let id1 = WorkspaceId::from(1, 1);
    let id2 = WorkspaceId::from(1, 2);

    assert!(!id1.is_same_workspace(&id2));
  }
}
