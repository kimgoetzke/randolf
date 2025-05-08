use crate::common::{PersistentWorkspaceId, Window, WindowHandle};
use crate::files::FileManager;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Display;

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct WorkspacesFile {
  pub workspaces: HashMap<PersistentWorkspaceId, HashSet<WindowHandle>>,
}

impl WorkspacesFile {
  /// Creates a new `WorkspacesFile` instance with an empty workspaces map.
  pub fn new() -> Self {
    Self {
      workspaces: HashMap::new(),
    }
  }

  /// Adds a window handle to the specified workspace and saves the changed using the provided file manager.
  pub(crate) fn add(
    &mut self,
    file_manager: &FileManager<WorkspacesFile>,
    workspace_id: &PersistentWorkspaceId,
    window_handle: &WindowHandle,
  ) {
    if let Some(workspace) = self.workspaces.get_mut(workspace_id) {
      workspace.insert(*window_handle);
    } else {
      self.workspaces.insert(*workspace_id, HashSet::from([*window_handle]));
    }
    self.save(file_manager);
  }

  // Adds all window handles to the specified workspace and saves the changes using the provided file manager.
  pub(crate) fn add_all(
    &mut self,
    file_manager: &FileManager<WorkspacesFile>,
    workspace_id: &PersistentWorkspaceId,
    windows: &[Window],
  ) {
    if let Some(workspace) = self.workspaces.get_mut(workspace_id) {
      workspace.extend(windows.iter().map(|w| w.handle));
    } else {
      self
        .workspaces
        .insert(*workspace_id, windows.iter().map(|w| w.handle).collect());
    }
    self.save(file_manager);
  }

  /// Removes a workspace with all its windows and saves the changes using the provided file manager.
  pub(crate) fn remove_workspace(
    &mut self,
    file_manager: &FileManager<WorkspacesFile>,
    workspace_id: &PersistentWorkspaceId,
  ) {
    self.workspaces.remove(workspace_id);
    self.save(file_manager);
  }

  /// Removes all windows from all workspaces, completely ignoring the provided workspace i.e. no workspace will be
  /// removed and the provided workspace will not be modified, even if it contains the windows to be removed. Saves the
  /// changes using the provided file manager.
  pub(crate) fn remove_all_excluding(
    &mut self,
    file_manager: &FileManager<WorkspacesFile>,
    excluded_workspace_id: &PersistentWorkspaceId,
    windows: &[Window],
  ) {
    let handles_to_remove: Vec<WindowHandle> = windows.iter().map(|w| w.handle).collect();
    for (id, windows) in self.workspaces.iter_mut() {
      if id == excluded_workspace_id {
        continue;
      }
      windows.retain(|&handle| !handles_to_remove.contains(&handle));
    }
    self.save(file_manager);
  }

  /// Clears all workspaces and saves the changes using the provided file manager.
  pub(crate) fn clear(&mut self, file_manager: &FileManager<WorkspacesFile>) {
    self.workspaces.clear();
    self.save(file_manager);
  }

  // TODO: Check whether serialisation can be done a little cleaner (no duplicate entries, etc.)
  fn save(&mut self, file_manager: &FileManager<WorkspacesFile>) {
    file_manager.save(self).expect("Failed to save workspace file");
  }
}

impl Display for WorkspacesFile {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    for (i, (workspace_id, windows)) in self.workspaces.iter().enumerate() {
      write!(f, "{}: [", workspace_id)?;
      for (j, window) in windows.iter().enumerate() {
        write!(f, "{}", window)?;
        if j < windows.len() - 1 {
          write!(f, ", ")?;
        }
      }
      write!(f, "]")?;
      if i < self.workspaces.len() - 1 {
        write!(f, ", ")?;
      }
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::common::Rect;
  use crate::utils::create_temp_directory;
  use std::fs;

  #[test]
  fn clear_removes_all_workspaces() {
    let directory = create_temp_directory();
    let file = directory.path().join("test.toml");
    let file_manager = FileManager::new_test(file);
    let mut workspace_file = WorkspacesFile::new();
    let workspace_id = PersistentWorkspaceId::new_test(1);
    let window_handle = WindowHandle::from(1);

    workspace_file.add(&file_manager, &workspace_id, &window_handle);
    workspace_file.clear(&file_manager);

    assert!(workspace_file.workspaces.is_empty());
  }

  #[test]
  fn clear_updates_file_on_disk() {
    let directory = create_temp_directory();
    let file = directory.path().join("test.toml");
    let file_manager = FileManager::new_test(file.clone());
    let mut workspace_file = WorkspacesFile::new();
    let workspace_id = PersistentWorkspaceId::new_test(1);
    let window_handle = WindowHandle::from(1);

    workspace_file.add(&file_manager, &workspace_id, &window_handle);
    workspace_file.clear(&file_manager);

    assert_eq!(
      fs::read_to_string(file).expect("Failed to read config file"),
      "[workspaces]\n"
    );
  }

  #[test]
  fn add_all_adds_multiple_windows_to_workspace() {
    let directory = create_temp_directory();
    let file = directory.path().join("test.toml");
    let file_manager = FileManager::new_test(file.clone());
    let mut workspace_file = WorkspacesFile::new();
    let workspace_id = PersistentWorkspaceId::new_test(1);
    let windows = vec![Window::new_test(1, Rect::default()), Window::new_test(2, Rect::default())];

    workspace_file.add_all(&file_manager, &workspace_id, &windows);

    assert_eq!(workspace_file.workspaces[&workspace_id].len(), 2);
  }

  #[test]
  fn add_all_adds_updates_file_on_disk() {
    let directory = create_temp_directory();
    let file = directory.path().join("test.toml");
    let file_manager = FileManager::new_test(file.clone());
    let mut workspace_file = WorkspacesFile::new();
    let workspace_id = PersistentWorkspaceId::new_test(1);
    let windows = vec![Window::new_test(1, Rect::default()), Window::new_test(2, Rect::default())];

    workspace_file.add_all(&file_manager, &workspace_id, &windows);

    let file = fs::read_to_string(file).expect("Failed to read config file");
    assert!(file.contains("[[workspaces.\"P_DISPLAY|1|true\"]]\nhwnd = 1\n"));
    assert!(file.contains("[[workspaces.\"P_DISPLAY|1|true\"]]\nhwnd = 2\n"));
  }

  #[test]
  fn remove_workspace_removes_specified_workspace() {
    let directory = create_temp_directory();
    let file = directory.path().join("test.toml");
    let file_manager = FileManager::new_test(file);
    let mut workspace_file = WorkspacesFile::new();
    let workspace_id = PersistentWorkspaceId::new_test(1);
    let window_handle = WindowHandle::from(1);
    workspace_file.add(&file_manager, &workspace_id, &window_handle);

    workspace_file.remove_workspace(&file_manager, &workspace_id);

    assert!(!workspace_file.workspaces.contains_key(&workspace_id));
  }

  #[test]
  fn remove_workspace_updates_file_on_disk() {
    let directory = create_temp_directory();
    let file = directory.path().join("test.toml");
    let file_manager = FileManager::new_test(file.clone());
    let mut workspace_file = WorkspacesFile::new();
    let workspace_id = PersistentWorkspaceId::new_test(1);
    let window_handle = WindowHandle::from(1);
    workspace_file.add(&file_manager, &workspace_id, &window_handle);

    workspace_file.remove_workspace(&file_manager, &workspace_id);

    let file = fs::read_to_string(file).expect("Failed to read config file");
    assert!(file.contains("[workspaces]\n"));
    assert!(!file.contains("hwnd = 1"));
  }

  #[test]
  fn remove_all_except_keeps_only_specified_workspace() {
    let directory = create_temp_directory();
    let file = directory.path().join("test.toml");
    let file_manager = FileManager::new_test(file.clone());
    let mut workspace_file = WorkspacesFile::new();
    let workspace_id_1 = PersistentWorkspaceId::new_test(1);
    let workspace_id_2 = PersistentWorkspaceId::new_test(2);
    let window_1 = Window::new_test(1, Rect::default());
    let window_2 = Window::new_test(2, Rect::default());
    workspace_file.add(&file_manager, &workspace_id_1, &window_1.handle);
    workspace_file.add(&file_manager, &workspace_id_2, &window_2.handle);

    workspace_file.remove_all_excluding(&file_manager, &workspace_id_1, &[window_1.clone(), window_2.clone()]);

    assert!(workspace_file.workspaces.contains_key(&workspace_id_1));
    assert_eq!(workspace_file.workspaces[&workspace_id_1].len(), 1);
    assert!(workspace_file.workspaces[&workspace_id_1].contains(&window_1.handle));
    assert!(workspace_file.workspaces.contains_key(&workspace_id_2));
    assert_eq!(workspace_file.workspaces[&workspace_id_2].len(), 0);
  }

  #[test]
  fn remove_all_except_updates_file_on_disk() {
    let directory = create_temp_directory();
    let file = directory.path().join("test.toml");
    let file_manager = FileManager::new_test(file.clone());
    let mut workspace_file = WorkspacesFile::new();
    let workspace_id_1 = PersistentWorkspaceId::new_test(1);
    let workspace_id_2 = PersistentWorkspaceId::new_test(2);
    let window_1 = Window::new_test(1, Rect::default());
    let window_2 = Window::new_test(2, Rect::default());
    let window_3 = Window::new_test(4, Rect::default());
    workspace_file.add(&file_manager, &workspace_id_1, &window_1.handle);
    workspace_file.add(&file_manager, &workspace_id_1, &window_2.handle);
    workspace_file.add(&file_manager, &workspace_id_2, &window_3.handle);

    workspace_file.remove_all_excluding(
      &file_manager,
      &workspace_id_1,
      &[window_1.clone(), window_2.clone(), window_3.clone()],
    );

    let file = fs::read_to_string(file).expect("Failed to read config file");
    assert!(file.contains("[workspaces]\n\"P_DISPLAY|2|true\" = []\n"));
    assert!(file.contains("[[workspaces.\"P_DISPLAY|1|true\"]]\nhwnd = 1\n"));
    assert!(file.contains("[[workspaces.\"P_DISPLAY|1|true\"]]\nhwnd = 2\n"));
  }

  #[test]
  fn display_formats_workspaces_correctly() {
    let mut workspace_file = WorkspacesFile::new();
    let workspace_id = PersistentWorkspaceId::new_test(1);
    let window_handle = WindowHandle::from(1);

    workspace_file.workspaces.insert(workspace_id, HashSet::from([window_handle]));

    let formatted = format!("{}", workspace_file);
    assert!(formatted.contains(&format!("{}: [{}]", workspace_id, window_handle)));
  }
}
