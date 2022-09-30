use std::path::{Path, PathBuf};

use log::{debug, error};
use tokio::io;
use uuid::Uuid;

pub struct TmpDir {
  pub root: PathBuf,
}

impl TmpDir {
  /// create a new tmpdir under /tmp/judge with random dirname
  pub async fn new() -> io::Result<Self> {
    Self::from(Uuid::new_v4().to_string()).await
  }

  /// create a new tmpdir under /tmp/judge with specific dirname
  pub async fn from(dirname: impl AsRef<Path>) -> io::Result<Self> {
    let mut path = PathBuf::from("/tmp/judge");
    path.push(dirname);
    tokio::fs::create_dir_all(&path).await?;
    Ok(Self { root: path })
  }

  /// get file name under current directory
  pub fn filename(&self, filename: impl AsRef<Path>) -> PathBuf {
    let mut path = self.root.clone();
    path.push(filename);
    path
  }

  /// get a random file under current directory
  pub fn random_file(&self) -> PathBuf {
    self.filename(Uuid::new_v4().to_string())
  }
}

impl Drop for TmpDir {
  fn drop(&mut self) {
    let root = self.root.clone();
    tokio::spawn(async move {
      match tokio::fs::remove_dir_all(&root).await {
        Ok(_) => debug!("removed tmpdir: {root:?}"),
        Err(e) => error!("failed to remove tmpdir: {root:?}: {e}"),
      }
    });
  }
}
