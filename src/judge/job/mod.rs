pub mod executable;
pub mod limit;
pub mod wait;

use std::process::{Output, Stdio};
use thiserror::Error;
use tokio::process::Command;

use self::{
  executable::Executable,
  limit::{cgroup::CgroupLimit, rlimit::RLimit, Limit, LimitError},
  wait::Wait,
};

/// Run the program with limitations
pub struct Job {
  command: Command,

  stdin: Option<Stdio>,
  stdout: Option<Stdio>,
  stderr: Option<Stdio>,

  rlimit: Option<RLimit>,
  cgroup: Option<CgroupLimit>,
}

#[derive(Debug, Error)]
pub enum JobError {
  #[error("failed to execute command: {0}")]
  IoError(#[from] std::io::Error),
  #[error("wait failed: {0}")]
  WaitError(#[from] self::wait::WaitError),
  // #[error("failed to limit cgroup: {0}")]
  // CgroupError(#[from] cgroups_rs::error::Error),
  #[error("failed to apply limit to job: {0}")]
  LimitError(#[from] LimitError),
}

impl Job {
  /// create a new job without and limitations
  pub fn from_unlimited(command: Command) -> Self {
    Self {
      command,
      stdin: None,
      stdout: None,
      stderr: None,
      rlimit: None,
      cgroup: None,
    }
  }

  /// create a new job with executable file
  pub fn from_executable(executable: &Executable) -> Self {
    let command = match executable {
      Executable::Elf(path) => Command::new(path),
      Executable::Command { name, args } => {
        let mut command = Command::new(name);
        command.args(args);
        command
      }
    };
    Self::from_unlimited(command)
  }
}

// builder
impl Job {
  pub fn stdin(mut self, stdin: impl Into<Stdio>) -> Self {
    self.stdin = Some(stdin.into());
    self
  }
  pub fn stdout(mut self, stdout: impl Into<Stdio>) -> Self {
    self.stdout = Some(stdout.into());
    self
  }
  pub fn stderr(mut self, stderr: impl Into<Stdio>) -> Self {
    self.stderr = Some(stderr.into());
    self
  }
  pub fn rlimit(mut self, rlimit: RLimit) -> Self {
    self.rlimit = Some(rlimit);
    self
  }
  pub fn cgroup(mut self, cgroup: CgroupLimit) -> Self {
    self.cgroup = Some(cgroup);
    self
  }
}

impl Job {
  /// Apply I/O source and limits to Command
  fn command(mut self) -> Result<Command, JobError> {
    // apply limits
    if let Some(rlimit) = self.rlimit {
      rlimit.apply_to(&mut self.command)?;
    }
    if let Some(cgroup) = self.cgroup {
      cgroup.apply_to(&mut self.command)?;
    }

    // apply io
    if let Some(stdin) = self.stdin {
      self.command.stdin(stdin);
    }
    if let Some(stdout) = self.stdout {
      self.command.stdout(stdout);
    }
    if let Some(stderr) = self.stderr {
      self.command.stderr(stderr);
    }

    Ok(self.command)
  }

  /// Execute the job and get resource usage and exit_type
  pub async fn status(self) -> Result<Wait, JobError> {
    Ok(
      Wait::wait(
        self
          .command()?
          .spawn()?
          .id()
          .expect("failed to get child pid") as i32,
      )
      .await?,
    )
  }

  /// Execute the job and get stdout and stderr
  pub async fn output(self) -> Result<Output, JobError> {
    Ok(self.command()?.output().await?)
  }
}
