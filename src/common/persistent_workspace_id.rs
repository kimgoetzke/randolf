use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// The persistent ID of a Randolf workspace. This ID is unlikely to change at runtime, or even after a restart.
/// However, unlike [`TransientWorkspaceId`][wst], it lacks the [`MonitorHandle`][crate::common::MonitorHandle],
/// which is used by many Windows APIs.
///
/// It is recommended to use [`PersistentWorkspaceId`] by default, and only use [`TransientWorkspaceId`][wst]
/// _within_ a single operation/[`Command`][crate::common::Command] execution. This can be done by resolving the
/// [`PersistentWorkspaceId`] to a [`TransientWorkspaceId`][wst] at the start of the operation.
///
/// [wst]: crate::common::TransientWorkspaceId
#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct PersistentWorkspaceId {
  pub monitor_id: [u16; 32],
  pub workspace: usize,
  is_on_primary_monitor: bool,
}

impl PersistentWorkspaceId {
  pub fn new(monitor_id: [u16; 32], workspace: usize, is_on_primary_monitor: bool) -> Self {
    PersistentWorkspaceId {
      monitor_id,
      workspace,
      is_on_primary_monitor,
    }
  }

  pub fn id_to_string(&self) -> String {
    let device_name = String::from_utf16_lossy(&self.monitor_id).trim_end_matches('\0').to_string();
    if !device_name.is_empty() {
      device_name
    } else {
      panic!("Failed to convert ID to string");
    }
  }

  pub fn is_on_primary_monitor(&self) -> bool {
    self.is_on_primary_monitor
  }

  #[allow(unused)]
  pub fn is_same_monitor(&self, other: &Self) -> bool {
    self.monitor_id == other.monitor_id
  }

  pub fn is_same_workspace(&self, other: &Self) -> bool {
    self.workspace == other.workspace
  }
}

impl Display for PersistentWorkspaceId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "wsp#{}-{}", self.id_to_string(), self.workspace)
  }
}

impl Serialize for PersistentWorkspaceId {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    let monitor_id_str = String::from_utf16_lossy(&self.monitor_id).trim_end_matches('\0').to_string();
    let serialized = format!("{}|{}|{}", monitor_id_str, self.workspace, self.is_on_primary_monitor);
    serializer.serialize_str(&serialized)
  }
}

impl<'de> Deserialize<'de> for PersistentWorkspaceId {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    let s = String::deserialize(deserializer)?;
    let parts: Vec<&str> = s.split('|').collect();
    if parts.len() != 3 {
      return Err(serde::de::Error::custom("Invalid format for PersistentWorkspaceId"));
    }

    let monitor_id = parts[0]
      .encode_utf16()
      .chain(std::iter::repeat(0).take(32 - parts[0].len()))
      .collect::<Vec<u16>>()
      .try_into()
      .map_err(|_| serde::de::Error::custom("Invalid monitor_id length"))?;
    let workspace = parts[1].parse().map_err(serde::de::Error::custom)?;
    let is_on_primary_monitor = parts[2].parse().map_err(serde::de::Error::custom)?;

    Ok(PersistentWorkspaceId {
      monitor_id,
      workspace,
      is_on_primary_monitor,
    })
  }
}

#[cfg(test)]
mod tests {
  use crate::common::{PersistentWorkspaceId, TransientWorkspaceId};

  impl PersistentWorkspaceId {
    /// Creates a new instance of `PersistentWorkspaceId` for testing purposes for a primary monitor with a fixed
    /// monitor ID of `"P_DISPLAY"`
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
        is_on_primary_monitor: true,
      }
    }
  }

  impl From<TransientWorkspaceId> for PersistentWorkspaceId {
    fn from(value: TransientWorkspaceId) -> Self {
      PersistentWorkspaceId {
        monitor_id: value.monitor_id,
        workspace: value.workspace,
        is_on_primary_monitor: false,
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
  #[should_panic(expected = "Failed to convert ID to string")]
  fn id_to_string_panics_on_empty_id() {
    PersistentWorkspaceId::new([0; 32], 1, false).id_to_string();
  }

  #[test]
  fn workspace_id_same_monitor_returns_true() {
    let id1 = PersistentWorkspaceId::new([1; 32], 1, true);
    let id2 = PersistentWorkspaceId::new([1; 32], 2, true);

    assert!(id1.is_same_monitor(&id2));
  }

  #[test]
  fn workspace_id_different_monitor_returns_false() {
    let id1 = PersistentWorkspaceId::new([1; 32], 1, true);
    let id2 = PersistentWorkspaceId::new([2; 32], 1, false);

    assert!(!id1.is_same_monitor(&id2));
  }

  #[test]
  fn workspace_id_same_workspace_returns_true() {
    let id1 = PersistentWorkspaceId::new([1; 32], 1, true);
    let id2 = PersistentWorkspaceId::new([2; 32], 1, false);

    assert!(id1.is_same_workspace(&id2));
  }

  #[test]
  fn workspace_id_different_workspace_returns_false() {
    let id1 = PersistentWorkspaceId::new([1; 32], 1, true);
    let id2 = PersistentWorkspaceId::new([1; 32], 2, true);

    assert!(!id1.is_same_workspace(&id2));
  }
}
