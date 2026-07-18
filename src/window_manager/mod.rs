mod navigation;
mod placement;
mod scrolling;
mod spatial;
#[cfg(test)]
mod tests;
#[allow(clippy::module_inception)]
mod window_manager;

pub use window_manager::WindowManager;
