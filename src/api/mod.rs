mod mock_api;
mod native_api;
mod windows_api;

pub use native_api::NativeApi;
pub use windows_api::{WindowsApi, do_process_windows_messages, get_all_monitors};

#[cfg(test)]
pub use mock_api::test::MockWindowsApi;
