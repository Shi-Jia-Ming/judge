use std::{collections::HashMap, path::PathBuf};

use log::debug;
use tokio::{
  io,
  sync::{mpsc, oneshot},
};

use crate::communicate::message::{SendMessage, SyncRequest};

pub struct CacheDir {
  pub root: PathBuf,
  pub map: HashMap<String, Vec<oneshot::Sender<PathBuf>>>,
  pub cache: HashMap<String, PathBuf>,
  pub stream: mpsc::Sender<SendMessage>,
}

impl CacheDir {
  pub async fn new(stream: mpsc::Sender<SendMessage>) -> io::Result<Self> {
    let root = PathBuf::from("/var/judge/data");
    tokio::fs::create_dir_all(&root).await?;
    Ok(Self {
      root,
      map: HashMap::new(),
      cache: HashMap::new(),
      stream,
    })
  }

  /// get filename in cache dir
  pub fn filename(&self, filename: &String) -> PathBuf {
    let mut path = self.root.clone();
    path.push(filename);
    path
  }

  pub async fn save(&mut self, filename: &String, contents: &[u8]) -> io::Result<()> {
    let path = self.filename(filename);
    tokio::fs::write(&path, contents).await?;
    debug!("sync file {filename} saved to {path:?}");
    if let Some(senders) = self.map.remove(filename) {
      for sender in senders {
        sender.send(path.clone()).unwrap();
      }
    }
    self.cache.insert(filename.clone(), path.clone());
    Ok(())
  }

  pub async fn sync(&mut self, filename: &String) -> oneshot::Receiver<PathBuf> {
    let (send, recv) = oneshot::channel();

    if self.cache.contains_key(filename) {
      let path = self.cache.get(filename).unwrap().clone();
      debug!("found cache in {path:?}");
      tokio::spawn(async { send.send(path).unwrap() });
    }
    // sync now
    else {
      debug!("cache not found, send message");
      self
        .stream
        .send(SendMessage::Sync(SyncRequest {
          uuid: filename.clone(),
        }))
        .await
        .expect("failed to send sync request");

      match self.map.get_mut(filename) {
        Some(vec) => vec.push(send),
        None => {
          self.map.insert(filename.clone(), vec![send]);
        }
      }
    }

    recv
  }
}
