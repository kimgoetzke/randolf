mod mock_windows_api;
mod native_hooks;
mod native_ui;
mod real_windows_api;
pub mod real_windows_api_for_dragging;
mod windows_api;

pub(crate) use native_hooks::NativeHooks;
pub(crate) use native_ui::NativePickerUi;
pub use real_windows_api::{RealWindowsApi, do_process_windows_messages, get_all_monitors};
pub use windows_api::{WindowLookupError, WindowMetadata, WindowPositioningResult, WindowsApi};

#[cfg(test)]
pub use mock_windows_api::test::MockWindowsApi;
