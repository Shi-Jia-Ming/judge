use crate::{
  communicate::result::JudgeResult,
  judge::step::{config::ConfigHandler, judge::JudgeHandler, sync::SyncHandler},
};
use async_trait::async_trait;
use log::debug;

use super::{Handle, HandleContext};

pub struct RequestHandler;

#[async_trait]
impl Handle<JudgeResult> for RequestHandler {
  async fn handle(self, context: &HandleContext) -> anyhow::Result<JudgeResult> {
    debug!("Working on task {}", context.request.id);

    // start sync files
    let files = SyncHandler::new(context.request.files.clone())
      .handle(context)
      .await?;

    // parse config.json
    let config = ConfigHandler::new(files.get("config.json").cloned())
      .handle(context)
      .await?;

    // run task due to config
    JudgeHandler::new(config, files).handle(context).await
  }
}
