use thiserror::Error;
use tokio::sync::oneshot::error::RecvError;

use self::{
  checker::CheckError,
  job::JobError,
  step::{compile::CompileError, sync::SyncError},
};

pub mod checker;
pub mod dispatch;
pub mod io;
pub mod job;
pub mod language;
pub mod step;
pub mod tmpdir;

#[derive(Debug, Error)]
pub enum JudgeError {
  #[error("failed to sync file: {0}")]
  SyncError(#[from] SyncError),
  #[error("failed to wait file: {0}")]
  RecvError(#[from] RecvError),
  #[error("config.json not found")]
  ConfigNotFound,
  #[error("failed to parse config.json: {0}")]
  ConfigError(#[from] serde_json::Error),
  #[error("failed to run io command: {0}")]
  IoError(#[from] tokio::io::Error),
  #[error("failed to compile: {0}")]
  CompileError(#[from] CompileError),
  #[error("file not found: {0}")]
  FileNotFound(String),
  #[error("failed to execute job: {0}")]
  JobError(#[from] JobError),
  #[error("failed to run checker: {0}")]
  CheckError(#[from] CheckError),
}