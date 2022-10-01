use crate::{
  communicate::result::JudgeResult,
  judge::step::{config::ConfigHandler, sync::SyncHandler, task::TaskStep},
};
use async_trait::async_trait;
use log::debug;

use super::{Handle, HandleContext};

pub struct RequestHandler;

#[async_trait]
impl Handle<JudgeResult> for RequestHandler {
  async fn handle(self, context: &HandleContext) -> anyhow::Result<JudgeResult> {
    debug!("Working on task {}", context.request.id);

    debug!("Start sync files...");
    let files = SyncHandler::new(context.request.files.clone())
      .handle(context)
      .await?;

    debug!("Parsing config.json...");
    let config = ConfigHandler::new(files.get("config.json").cloned())
      .handle(context)
      .await?;

    debug!("Running task...");
    TaskStep::new(config, files).handle(context).await
  }
}
