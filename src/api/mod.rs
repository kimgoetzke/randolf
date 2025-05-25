mod mock_windows_api;
mod real_windows_api;
pub mod window_drag_manager;
mod windows_api;

pub use real_windows_api::{RealWindowsApi, do_process_windows_messages, get_all_monitors};
pub use windows_api::WindowsApi;

#[cfg(test)]
pub use mock_windows_api::test::MockWindowsApi;
