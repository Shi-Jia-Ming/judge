use async_trait::async_trait;
use log::debug;
use std::{collections::HashMap, path::PathBuf};

use crate::{
  communicate::{
    message::{Progress, SendMessage},
    result::{JudgeResult, JudgeStatus, SubtaskResult, SubtaskStatus},
    spec::JudgeSpec,
  },
  judge::{
    checker::Checker,
    step::{compile::CompileStep, subtask::SubtaskHandler, task::TaskRunnerContextBuilder},
    tmpdir::TmpDir,
    utils::{Memory, Time},
  },
  judge_error,
};

use super::{Handle, HandleContext};

pub struct JudgeHandler {
  task: JudgeSpec,
  files: HashMap<String, PathBuf>,
}

impl JudgeHandler {
  pub fn new(task: JudgeSpec, files: HashMap<String, PathBuf>) -> Self {
    Self { task, files }
  }
}

#[async_trait]
impl Handle<JudgeResult> for JudgeHandler {
  async fn handle(self, context: &HandleContext) -> anyhow::Result<JudgeResult> {
    /// Send progress package to web
    macro_rules! progress {
      ($result:expr) => {
        context
          .sender
          .send(SendMessage::Progress(Progress {
            id: context.request.id,
            result: $result,
          }))
          .await
          .unwrap()
      };
    }

    match self.task {
      // default task
      JudgeSpec::Default { common } => {
        debug!("Start judging default task...");

        let mut result = JudgeResult::from_status(JudgeStatus::Compiling);
        progress!(result.clone());

        // compile step
        let compile = CompileStep::new(
          context.request.code.clone().into_bytes(),
          context.request.language.clone(),
        )
        .compile()
        .await;
        let compile = match compile {
          Ok(compile) => compile,
          Err(error) => judge_error!(JudgeStatus::CompileError, "{error}"),
        };
        result.message += format!("{}\n{}", compile.stdout, compile.stderr).trim();

        // preparing checker
        let checker = Checker::new(&common.checker).await?;

        // start judge
        result.status = JudgeStatus::Running;
        progress!(result.clone());

        // create a tmpdir for stdout
        let tmpdir = TmpDir::new().await?;
        // global default time limit
        let time_limit = Time::from_seconds(1);
        // global default memory limit
        let memory_limit = Memory::from_megabytes(256);

        let runner = TaskRunnerContextBuilder::default()
          .files(&self.files)
          .tmpdir(&tmpdir)
          .checker(&checker)
          .executable(&compile.executable)
          .build();

        // TODO: run subtask parallel
        for subtask in common.subtasks.into_iter() {
          // run the subtask
          let subtask = SubtaskHandler::new(subtask, &runner)
            .time_limit(time_limit)
            .memory_limit(memory_limit)
            .handle(context)
            .await;

          // check if panics
          let subtask = match subtask {
            Ok(subtask) => subtask,
            Err(error) => {
              SubtaskResult::from_status_message(SubtaskStatus::SystemError, format!("{error}"))
            }
          };

          // calculate result score according to subtask status
          result.score += match subtask.status {
            SubtaskStatus::Accepted => subtask.score,
            _ => 0,
          };

          progress!(result.clone());
        }

        // calculate judge status according to subtask status
        result.status = JudgeStatus::Accepted;
        for subtask in result.subtasks.iter() {
          let status = match subtask.status {
            SubtaskStatus::Pending => Some(JudgeStatus::SystemError),
            SubtaskStatus::Running => Some(JudgeStatus::SystemError),
            SubtaskStatus::Accepted => None,
            SubtaskStatus::WrongAnswer => Some(JudgeStatus::WrongAnswer),
            SubtaskStatus::TimeLimitExceeded => Some(JudgeStatus::TimeLimitExceeded),
            SubtaskStatus::MemoryLimitExceeded => Some(JudgeStatus::MemoryLimitExceeded),
            SubtaskStatus::RuntimeError => Some(JudgeStatus::RuntimeError),
            SubtaskStatus::SystemError => Some(JudgeStatus::SystemError),
          };

          if let Some(status) = status {
            result.status = status;
            break;
          }
        }

        Ok(result)
      }

      JudgeSpec::Interactive { .. } => todo!(),
      JudgeSpec::Dynamic { .. } => todo!(),

      // Submit answer problem, we ignore the language
      JudgeSpec::SubmitAnswer { answer } => {
        debug!("Start judging submit_answer task...");
        let mut result = JudgeResult::from_status(JudgeStatus::WrongAnswer);

        // TODO multi-line judge
        if context.request.code.trim() == answer.trim() {
          result.status = JudgeStatus::Accepted;
          result.score = 100;
        }

        Ok(result)
      }
    }
  }
}
