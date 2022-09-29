use futures::{SinkExt, StreamExt};
use log::{debug, error};
use std::{collections::HashMap, error::Error, path::PathBuf};
use tokio::{fs::File, net::TcpStream, sync::oneshot::Sender};
use tokio_tungstenite::{
  tungstenite::{self, Message},
  WebSocketStream,
};

use crate::{
  communicate::{
    message::{Hello, Progress, RecvMessage, SendMessage, TaskRequest},
    result::{JudgeResult, JudgeStatus, SubtaskResult, SubtaskStatus, TaskResult, TaskStatus},
    spec::TaskSpec,
  },
  judge::{
    checker::Checker,
    job::{
      limit::{cgroup::CgroupLimit, rlimit::RLimit},
      wait::WaitStatus,
      Job,
    },
    step::{
      compile::{Compile, Compiler},
      sync::SyncFile,
    },
    tmpdir::TmpDir,
  },
};

use super::{tmpdir::CacheDir, JudgeError};

pub struct Dispatch {
  pub stream: WebSocketStream<TcpStream>,
  pub cache: CacheDir,
  pub buzy: bool,
  pub waiting: HashMap<String, Sender<PathBuf>>,
}

impl Dispatch {
  pub async fn from(stream: WebSocketStream<TcpStream>) -> Self {
    Self {
      stream,
      cache: CacheDir::new().await.expect("failed to access sync tmpdir"),
      buzy: false,
      waiting: HashMap::new(),
    }
  }
}

impl Dispatch {
  /// Send message to WebSocket
  pub async fn send_message(
    &mut self,
    send_message: SendMessage,
  ) -> Result<(), tungstenite::Error> {
    debug!("Send: {:?}", send_message);
    let message = serde_json::to_string(&send_message).unwrap();
    let message = Message::Text(message);
    self.stream.send(message).await
  }

  /// Handle received message
  async fn handle_message(&mut self, recv_message: RecvMessage) -> Result<(), Box<dyn Error>> {
    debug!("Received: {:?}", recv_message);

    match recv_message {
      RecvMessage::Ping => {
        self.send_message(SendMessage::Pong).await.unwrap();
        debug!("Ping? Pong!");
      }
      RecvMessage::Task(request) => {
        let id = request.id;
        if self.buzy {
          self.send_message(SendMessage::Reject { id }).await.unwrap();
          debug!("Rejected request {}: buzy", id);
        } else {
          self.buzy = true;
          self.send_message(SendMessage::Accept { id }).await.unwrap();
          debug!("Accepted request {}", id);

          let result = match self.handle_request(request).await {
            Ok(result) => result,
            Err(JudgeError::CompileError(error)) => JudgeResult {
              message: format!("{}", error),
              status: JudgeStatus::CompileError,
              score: 0,
              subtasks: Vec::new(),
            },
            Err(error) => JudgeResult {
              message: format!("[error] {}", error),
              status: JudgeStatus::SystemError,
              score: 0,
              subtasks: Vec::new(),
            },
          };

          debug!("Task finished: {:?}", result);
          self
            .send_message(SendMessage::Finish(Progress { id, result }))
            .await
            .unwrap();
        }
      }
      RecvMessage::Sync(sync) => {
        let contents = base64::decode(sync.data).unwrap();
        let filename = self.save_sync_file(&sync.uuid, contents).await?;
        debug!("Sync file saved to {:?}", filename);
      }
    }

    Ok(())
  }

  /// after accept request, start to handle it
  ///
  /// sync file and invoke [`TaskSpec::judge`] here
  async fn handle_request(&mut self, request: TaskRequest) -> Result<JudgeResult, JudgeError> {
    debug!("Working on task {}", request.id);

    debug!("Start sync files...");
    let mut filemap = HashMap::new();
    for (filename, uuid) in request.files.into_iter() {
      let filepath = self.sync_file(&uuid).await?.await?;
      filemap.insert(filename, filepath);
    }

    debug!("Parsing config.json...");
    let config = filemap
      .get("config.json")
      .ok_or(JudgeError::ConfigNotFound)?;
    let config = tokio::fs::read(config).await?;
    let config = serde_json::from_str::<TaskSpec>(&String::from_utf8_lossy(config.as_slice()))?;

    // Send progress package to web
    macro_rules! progress {
      ($result:expr) => {
        self
          .send_message(SendMessage::Progress(Progress {
            id: request.id,
            result: $result,
          }))
          .await
          .unwrap()
      };
    }

    match config {
      // default task
      TaskSpec::Default { common } => {
        debug!("Start judging default task...");
        let mut result = JudgeResult::from_status(JudgeStatus::Compiling);
        progress!(result.clone());

        debug!("Start compiling source code...");
        let source_dir = TmpDir::new().await?;
        let compile = Compiler::new(request.code.into_bytes(), request.language)
          .compile(&source_dir)
          .await?;
        result.message += compile.stderr.as_str();

        debug!("Preparing checker...");
        let checker = Checker::new(&common.checker).await?;

        debug!("Check input/output file exists...");
        for subtask in common.subtasks.iter() {
          for case in subtask.cases.iter() {
            filemap
              .get(&case.input)
              .ok_or(JudgeError::FileNotFound(case.input.clone()))?;
            filemap
              .get(&case.output)
              .ok_or(JudgeError::FileNotFound(case.output.clone()))?;
          }
        }

        debug!("Start judging...");
        result.status = JudgeStatus::Running;
        progress!(result.clone());
        let time_limit = common.time.unwrap_or(1000);
        let memory_limit = common.memory.unwrap_or(128 * 1024 * 1024);
        let tmpdir = TmpDir::new().await?;

        // TODO: run subtask parallel
        for subtask in common.subtasks.iter() {
          result.subtasks.push(SubtaskResult::from_spec(&subtask));
          let subtask_result = result.subtasks.last_mut().unwrap();
          let time_limit = subtask.time.unwrap_or(time_limit);
          let memory_limit = subtask.memory.unwrap_or(memory_limit);

          for task in subtask.cases.iter() {
            subtask_result
              .tasks
              .push(TaskResult::from_status(TaskStatus::Running));
            let task_result = subtask_result.tasks.last_mut().unwrap();
            let time_limit = task.time.unwrap_or(time_limit);
            let memory_limit = task.memory.unwrap_or(memory_limit);

            // prepare input/output files
            let input = filemap.get(&task.input).unwrap();
            let answer = filemap.get(&task.output).unwrap();
            let output = &tmpdir.random_file();

            // run executable
            let wait = Job::from_executable(&compile.executable)
              .stdin(File::open(input).await?.into_std().await)
              .stdout(File::create(output).await?.into_std().await)
              .cgroup(CgroupLimit::new(memory_limit))
              .rlimit(RLimit::new(time_limit, memory_limit))
              .status()
              .await?;

            // get task status and message
            let (task_status, message) = match wait.exit_type() {
              WaitStatus::TimeLimitExceed => (TaskStatus::TimeLimitExceeded, String::new()),
              WaitStatus::MemoryLimitExceed => (TaskStatus::MemoryLimitExceeded, String::new()),
              WaitStatus::RuntimeError => (TaskStatus::RuntimeError, String::new()),
              WaitStatus::Ok => {
                // read answer file from output
                let result = checker.check_file(output, answer).await?;
                (
                  match result.r#match {
                    true => TaskStatus::Accepted,
                    false => TaskStatus::WrongAnswer,
                  },
                  result.message,
                )
              }
            };

            task_result.status = task_status;
            task_result.message = message;
            task_result.time = Some(wait.cputime() as u32);
            task_result.memory = Some(wait.memory() as u64);

            // skip remain test cases if not accepted
            match task_result.status {
              TaskStatus::Accepted => continue,
              _ => break,
            }
          }

          subtask_result.status = subtask_result
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
          subtask_result.message = format!(
            "{}/{} passed",
            subtask_result
              .tasks
              .iter()
              .filter(|task| match task.status {
                TaskStatus::Accepted => true,
                _ => false,
              })
              .count(),
            subtask.cases.len()
          );

          progress!(result.clone());
        }

        Ok(result)
      }

      TaskSpec::Interactive { .. } => todo!(),
      TaskSpec::Dynamic { .. } => todo!(),

      // Submit answer problem, we ignore the language
      TaskSpec::SubmitAnswer { answer } => {
        debug!("Start judging submit_answer task...");
        let mut result = JudgeResult::from_status(JudgeStatus::WrongAnswer);

        // TODO multi-line judge
        if request.code.trim() == answer.trim() {
          result.status = JudgeStatus::Accepted;
          result.score = 100;
        }

        Ok(result)
      }
    }
  }

  /// Start the event loop.
  pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
    self
      .send_message(SendMessage::Hello(Hello {
        version: "v0".to_string(),
        cpus: 1,
        langs: vec!["c".to_string(), "cpp".to_string(), "py".to_string()],
        ext_features: vec![],
      }))
      .await?;

    loop {
      match self.stream.next().await {
        Some(msg) => match msg {
          Ok(msg) if msg.is_text() || msg.is_binary() => {
            let message = msg.into_text().unwrap();
            match serde_json::from_str(message.as_str()) {
              Ok(recv) => self.handle_message(recv).await?,
              Err(err) => error!("Unrecognized message: {}", err),
            }
          }
          Ok(msg) if msg.is_close() => break,
          Ok(msg) => debug!("Received Message: {}", msg),
          Err(err) => {
            error!("{}", err);
            break;
          }
        },
        None => break,
      }
    }

    Ok(())
  }
}
