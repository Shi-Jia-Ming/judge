use async_trait::async_trait;

use crate::{
  builder,
  communicate::{
    result::{SubtaskResult, SubtaskStatus, TaskResult, TaskStatus},
    spec::SubtaskSpec,
  },
  judge::utils::{Memory, Time},
};

use super::{
  task::{TaskHandler, TaskRunnerContext},
  Handle, HandleContext,
};

pub struct SubtaskHandler<'a> {
  subtask: SubtaskSpec,
  runner: &'a TaskRunnerContext<'a>,
  time_limit: Option<Time>,
  memory_limit: Option<Memory>,
}

impl<'a> SubtaskHandler<'a> {
  pub fn new(subtask: SubtaskSpec, runner: &'a TaskRunnerContext) -> Self {
    Self {
      subtask,
      runner,
      time_limit: None,
      memory_limit: None,
    }
  }

  builder!(time_limit, Time);
  builder!(memory_limit, Memory);
}

#[async_trait]
impl Handle<SubtaskResult> for SubtaskHandler<'_> {
  async fn handle(self, context: &HandleContext) -> anyhow::Result<SubtaskResult> {
    let mut result = SubtaskResult::from_spec(&self.subtask);
    let mut passed = 0;
    let total = self.subtask.cases.len();

    let time_limit = match self.subtask.time {
      Some(x) => Time::from_microseconds(x),
      None => self.time_limit.unwrap(),
    };
    let memory_limit = match self.subtask.memory {
      Some(x) => Memory::from_bytes(x),
      None => self.memory_limit.unwrap(),
    };

    for task in self.subtask.cases.into_iter() {
      let task = TaskHandler::new(task, self.runner)
        .time_limit(time_limit)
        .memory_limit(memory_limit)
        .handle(context)
        .await;
      let task = match task {
        Ok(task) => task,
        Err(error) => TaskResult::from_status_message(TaskStatus::SystemError, format!("{error}")),
      };

      match task.status {
        // count passed cases
        TaskStatus::Accepted => {
          result.tasks.push(task);
          passed += 1;
        }
        // skip remain test cases if not accepted
        _ => {
          result.tasks.push(task);
          break;
        }
      }
    }

    result.status = result
      .tasks
      .last()
      .and_then(|task| match task.status {
        TaskStatus::Pending => None,
        TaskStatus::Running => None,
        TaskStatus::Skipped => None,
        TaskStatus::Accepted => Some(SubtaskStatus::Accepted),
        TaskStatus::WrongAnswer => Some(SubtaskStatus::WrongAnswer),
        TaskStatus::TimeLimitExceeded => Some(SubtaskStatus::TimeLimitExceeded),
        TaskStatus::MemoryLimitExceeded => Some(SubtaskStatus::MemoryLimitExceeded),
        TaskStatus::RuntimeError => Some(SubtaskStatus::RuntimeError),
        TaskStatus::SystemError => Some(SubtaskStatus::SystemError),
      })
      .unwrap_or(SubtaskStatus::SystemError);
    result.message = format!("{passed}/{total} passed");

    Ok(result)
  }
}
