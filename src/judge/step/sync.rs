use std::path::PathBuf;

use async_trait::async_trait;
use thiserror::Error;
use tokio::{
  io,
  sync::oneshot::{self, Receiver},
};
use tokio_tungstenite::tungstenite;

use crate::{
  communicate::message::{SendMessage, SyncRequest},
  judge::dispatch::Dispatch,
};

#[derive(Error, Debug)]
pub enum SyncError {
  #[error("failed to send message: {0}")]
  TungsteniteError(#[from] tungstenite::Error),
  #[error("failed to create file: {0}")]
  IoError(#[from] io::Error),
}

#[async_trait]
pub trait SyncFile {
  /// Request to sync file, resolve when file is ready
  async fn sync_file(&mut self, uuid: &String) -> Result<Receiver<PathBuf>, SyncError>;

  /// Save synced file to cache
  async fn save_sync_file(&mut self, uuid: &String, contents: Vec<u8>) -> Result<(), SyncError>;
}

#[async_trait]
impl SyncFile for Dispatch {
  async fn sync_file(&mut self, uuid: &String) -> Result<Receiver<PathBuf>, SyncError> {
    let (sender, receiver) = oneshot::channel();
    let path = self.cache.filename(uuid);

    if tokio::fs::metadata(&path).await.is_ok() {
      tokio::spawn(async {
        sender.send(path).unwrap();
      });
    } else if self.waiting.contains_key(uuid) {
      // FIXME parallel requests for the same file
      panic!("multiple tasks waiting for the same sync file");
    } else {
      self.waiting.insert(uuid.clone(), sender);
      self
        .send_message(SendMessage::Sync(SyncRequest { uuid: uuid.clone() }))
        .await?;
    }

    Ok(receiver)
  }

  async fn save_sync_file(&mut self, uuid: &String, contents: Vec<u8>) -> Result<(), SyncError> {
    let path = self.cache.filename(uuid);
    tokio::fs::write(&path, contents).await?;

    // trigger any waiting tasks
    if let Some(sender) = self.waiting.remove(uuid) {
      sender.send(path).unwrap();
    }

    Ok(())
  }
}
