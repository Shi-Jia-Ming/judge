use async_trait::async_trait;
use log::debug;
use std::path::PathBuf;
use thiserror::Error;

use crate::communicate::spec::JudgeSpec;

use super::{Handle, HandleContext};

pub struct ConfigHandler {
  config: Option<PathBuf>,
}

impl ConfigHandler {
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
impl Handle<JudgeSpec> for ConfigHandler {
  async fn handle(self, _context: &HandleContext) -> anyhow::Result<JudgeSpec> {
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
