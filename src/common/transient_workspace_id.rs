use crate::common::{MonitorHandle, PersistentWorkspaceId};
use std::fmt::Display;

#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct TransientWorkspaceId {
  pub monitor_id: [u16; 32],
  pub monitor_handle: MonitorHandle,
  pub workspace: usize,
}

impl TransientWorkspaceId {
  pub fn from(persistent_workspace_id: PersistentWorkspaceId, monitor_handle: MonitorHandle) -> Self {
    TransientWorkspaceId {
      monitor_id: persistent_workspace_id.monitor_id,
      monitor_handle,
      workspace: persistent_workspace_id.workspace,
    }
  }
}

impl Display for TransientWorkspaceId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "wst#{}-{}", self.monitor_handle.handle, self.workspace)
  }
}

#[cfg(test)]
mod tests {
  use crate::common::{MonitorHandle, TransientWorkspaceId};

  impl TransientWorkspaceId {
    pub fn new(monitor_id: [u16; 32], monitor_handle: MonitorHandle, workspace: usize) -> Self {
      TransientWorkspaceId {
        monitor_id,
        monitor_handle,
        workspace,
      }
    }
  }

  #[test]
  fn workspace_id_display_handles_negative_values() {
    let handle = MonitorHandle::from(-1);
    let id = TransientWorkspaceId::new([1; 32], handle, 2);

    assert_eq!(id.to_string(), "wst#-1-2");
  }

  #[test]
  fn workspace_id_display_handles_large_values() {
    let handle = MonitorHandle::from(123456789);
    let id = TransientWorkspaceId::new([1; 32], handle, 987654321);

    assert_eq!(id.to_string(), "wst#123456789-987654321");
  }
}
