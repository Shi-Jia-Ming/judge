use std::{collections::HashMap, path::PathBuf};

use async_trait::async_trait;
use log::debug;

use super::{Handle, HandleContext};

pub struct SyncHandler {
  files: HashMap<String, String>,
}

impl SyncHandler {
  pub fn new(files: HashMap<String, String>) -> Self {
    Self { files }
  }
}

#[async_trait]
impl Handle<HashMap<String, PathBuf>> for SyncHandler {
  async fn handle(self, context: &HandleContext) -> anyhow::Result<HashMap<String, PathBuf>> {
    debug!("Start sync files...");
    let mut map = HashMap::new();

    for (filename, uuid) in self.files.into_iter() {
      // drop cache.lock() before await
      let recv = {
        let mut cache = context.cache.lock().await;
        cache.sync(&uuid).await
      };
      let filepath = recv.await?;
      map.insert(filename, filepath);
    }

    Ok(map)
  }
}
