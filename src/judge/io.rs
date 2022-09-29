use std::path::PathBuf;

use tokio::fs::File;

pub enum IOSource {
  Null,
  File(PathBuf),
}

impl From<&IOSource> for PathBuf {
  fn from(source: &IOSource) -> Self {
    match source {
      IOSource::Null => PathBuf::from("/dev/null"),
      IOSource::File(pathbuf) => pathbuf.clone(),
    }
  }
}

impl IOSource {
  pub async fn open(&self) -> std::io::Result<File> {
    File::open(PathBuf::from(self)).await
  }

  pub async fn create_and_open(&self) -> std::io::Result<File> {
    File::create(PathBuf::from(self)).await
  }
}
