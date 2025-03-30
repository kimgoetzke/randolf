#![allow(unused_imports)]

use crate::configuration_manager::{ConfigurationProvider, FILE_LOGGING_ENABLED};
use log::LevelFilter;
use simplelog::{ColorChoice, CombinedLogger, ConfigBuilder, SharedLogger, TermLogger, TerminalMode, WriteLogger};
use std::fs::File;
use std::sync::{Arc, Mutex};

pub struct LogManager {
  #[allow(unused)]
  configuration_provider: Arc<Mutex<ConfigurationProvider>>,
}

impl LogManager {
  fn new(configuration_provider: Arc<Mutex<ConfigurationProvider>>) -> Self {
    Self { configuration_provider }
  }

  pub fn new_initialised(configuration_provider: Arc<Mutex<ConfigurationProvider>>) -> Self {
    let log_manager = Self::new(configuration_provider);
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
    if self
      .configuration_provider
      .lock()
      .expect("Log manager failed to read [is_logging_enabled] because configuration provider is locked")
      .get_bool(FILE_LOGGING_ENABLED)
    {
      match File::create("randolf.log") {
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
