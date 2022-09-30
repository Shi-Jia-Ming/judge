use async_trait::async_trait;
use log::debug;
use std::path::PathBuf;
use thiserror::Error;

use crate::communicate::spec::TaskSpec;

use super::{Handle, HandleContext};

pub struct ConfigStep {
  config: Option<PathBuf>,
}

impl ConfigStep {
  pub fn new(config: Option<PathBuf>) -> Self {
    Self { config }
  }
}

#[derive(Debug, Error)]
pub enum ParseError {
  #[error("config.json not found")]
  ConfigNotFound,
}

#[async_trait]
impl Handle<TaskSpec> for ConfigStep {
  async fn handle(self, _context: &HandleContext) -> anyhow::Result<TaskSpec> {
    debug!("Parsing config.json...");

    // unwrap config
    let config = match self.config {
      Some(config) => config,
      None => Err(ParseError::ConfigNotFound)?,
    };
    // read from cache
    let config = tokio::fs::read(config).await?;
    // deserialize
    let config = serde_json::from_str(&String::from_utf8_lossy(config.as_slice()))?;

    Ok(config)
  }
}