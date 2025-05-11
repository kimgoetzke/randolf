#![allow(unused_imports)]

use crate::configuration_provider::ConfigurationProvider;
use crate::files::{FileManager, FileType};
use log::LevelFilter;
use simplelog::{ColorChoice, CombinedLogger, ConfigBuilder, SharedLogger, TermLogger, TerminalMode, WriteLogger};
use std::fs::File;
use std::sync::{Arc, Mutex};

#[cfg(not(debug_assertions))]
const LOG_FILE_NAME: &str = "randolf.log";

pub struct LogManager;

impl LogManager {
  fn new() -> Self {
    Self {}
  }

  pub fn new_initialised() -> Self {
    let log_manager = Self::new();
    log_manager.initialise();

    log_manager
  }

  #[allow(unused_mut)]
  fn initialise(&self) {
    let config = ConfigBuilder::new()
      .set_target_level(LevelFilter::Error)
      .set_thread_level(LevelFilter::Off)
      .build();
    let mut loggers: Vec<Box<dyn SharedLogger>> = vec![TermLogger::new(
      LevelFilter::Debug,
      config.clone(),
      TerminalMode::Mixed,
      ColorChoice::Auto,
    )];

    #[cfg(not(debug_assertions))]
    {
      let path = FileManager::<String>::get_path_to_file(LOG_FILE_NAME, FileType::Data)
        .unwrap_or_else(|err| panic!("Failed to get path to log file: {}", err));
      match File::create(path) {
        Ok(log_file) => {
          let write_logger = WriteLogger::new(LevelFilter::Trace, config, log_file);
          loggers.push(write_logger);
        }
        Err(err) => {
          eprintln!("Failed to create log file: {}", err);
        }
      }
    }

    let count = loggers.len();
    CombinedLogger::init(loggers).expect("Failed to initialise logger");
    info!("Initialised [{}] logger(s)", count);
  }
}
