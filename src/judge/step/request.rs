use crate::{
  communicate::{message::TaskRequest, result::JudgeResult},
  judge::step::{config::ConfigStep, sync::SyncStep, task::TaskStep},
};
use async_trait::async_trait;
use log::debug;

use super::{Handle, HandleContext};

#[async_trait]
impl Handle<JudgeResult> for TaskRequest {
  async fn handle(self, context: &HandleContext) -> anyhow::Result<JudgeResult> {
    debug!("Working on task {}", self.id);

    debug!("Start sync files...");
    let files = SyncStep::new(self.files).handle(context).await?;

    debug!("Parsing config.json...");
    let config = ConfigStep::new(files.get("config.json").cloned())
      .handle(context)
      .await?;

    debug!("Running task...");
    TaskStep::new(config, files)
      .id(self.id)
      .code(self.code)
      .language(self.language)
      .handle(context)
      .await
  }
}
