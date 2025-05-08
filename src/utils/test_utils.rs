#![cfg(test)]

use simplelog::*;
use std::sync::Once;
use tempfile::TempDir;

#[allow(dead_code)]
static INIT: Once = Once::new();

/// Initializes the logger for tests. This function can be called in the test setup. However, it must only be used for
/// debugging purposes and must be removed before committing / running all tests as it'll clash with the
/// `testing_logger` crate.
#[allow(dead_code)]
pub fn with_test_logger() {
  INIT.call_once(|| {
    TermLogger::init(LevelFilter::Trace, Config::default(), TerminalMode::Mixed, ColorChoice::Auto)
      .expect("Failed to init TermLogger");
  });
}

pub fn create_temp_directory() -> TempDir {
  TempDir::new().expect("Failed to create temporary directory")
}
