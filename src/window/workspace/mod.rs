mod transient_workspace_id;
#[allow(clippy::module_inception)]
mod workspace;
mod workspace_action;
mod workspace_guard;
mod workspace_manager;

pub(crate) use transient_workspace_id::TransientWorkspaceId;
pub(crate) use workspace::Workspace;
pub(crate) use workspace_manager::WorkspaceManager;

#[cfg(test)]
pub(crate) mod tests;
