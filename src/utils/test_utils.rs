#![cfg(test)]

use simplelog::*;
use std::sync::Once;

static INIT: Once = Once::new();

pub fn with_test_logger() {
  INIT.call_once(|| {
    TermLogger::init(LevelFilter::Trace, Config::default(), TerminalMode::Mixed, ColorChoice::Auto)
      .expect("Failed to init TermLogger");
  });
}
