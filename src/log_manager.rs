#![allow(unused_imports)]

use log::LevelFilter;
use simplelog::{ColorChoice, CombinedLogger, ConfigBuilder, SharedLogger, TermLogger, TerminalMode, WriteLogger};
use std::fs::File;

pub struct LogManager;

impl LogManager {
  pub fn new() -> Self {
    initialise();
    Self
  }
}

#[allow(unused_mut)]
fn initialise() {
  let config = ConfigBuilder::new()
    .set_target_level(LevelFilter::Off)
    .set_thread_level(LevelFilter::Off)
    .build();
  let mut loggers: Vec<Box<dyn SharedLogger>> = vec![TermLogger::new(
    LevelFilter::Debug,
    config.clone(),
    TerminalMode::Mixed,
    ColorChoice::Auto,
  )];

  #[cfg(not(debug_assertions))]
  match File::create("randolf.log") {
    Ok(log_file) => {
      let write_logger = WriteLogger::new(LevelFilter::Trace, config, log_file);
      loggers.push(write_logger);
    }
    Err(err) => {
      eprintln!("Failed to create log file: {}", err);
    }
  }

  let count = loggers.len();
  CombinedLogger::init(loggers).expect("Failed to initialise logger");
  info!("Initialised [{}] logger(s)", count);
}
