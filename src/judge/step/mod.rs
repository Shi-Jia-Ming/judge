use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{mpsc, Mutex};

use crate::communicate::message::{SendMessage, TaskRequest};

use super::cache::CacheDir;

pub mod compile;
pub mod config;
pub mod judge;
pub mod request;
pub mod subtask;
pub mod sync;
pub mod task;

/// Containing mpsc sender and a mutex cache in order to be shared among threads
#[derive(Clone)]
pub struct HandleContext {
  /// send message to websocket
  pub sender: mpsc::Sender<SendMessage>,
  /// get cache
  pub cache: Arc<Mutex<CacheDir>>,
  /// task request
  pub request: TaskRequest,
}

#[async_trait]
pub trait Handle<T> {
  async fn handle(self, context: &HandleContext) -> anyhow::Result<T>;
}

/// returns as system error
#[macro_export]
macro_rules! judge_error {
  ($status:expr, $($arg:tt)*) => {
    return Ok(JudgeResult::from_status_message(
      $status,
      format!($($arg)*),
    ))
  }
}

/// returns as system error
#[macro_export]
macro_rules! system_error {
  ($($arg:tt)*) => {
    return Ok(JudgeResult::from_status_message(
      JudgeStatus::SystemError,
      format!($($arg)*),
    ))
  }
}
