use thiserror::Error;

use crate::judge::{
  job::{executable::Executable, JobError},
  language::Language,
  tmpdir::TmpDir,
};

#[derive(Error, Debug)]
pub enum CompileError {
  #[error("failed to copy source code: {0}")]
  IoError(#[from] tokio::io::Error),
  #[error("failed to execute compile job: {0}")]
  JobError(#[from] JobError),
  #[error("unknown language")]
  UnknownLanguage,
  #[error("{0}")]
  Failed(String),
}

pub struct CompileResult {
  pub stdout: String,
  pub stderr: String,
  pub executable: Executable,
  pub tmpdir: TmpDir,
}

pub struct CompileStep {
  code: Vec<u8>,
  language: String,
}

impl CompileStep {
  pub fn new(code: Vec<u8>, language: String) -> Self {
    Self { code, language }
  }
}

impl CompileStep {
  pub async fn compile(self) -> anyhow::Result<CompileResult> {
    let language =
      Language::from_ext(self.language.as_str()).ok_or(CompileError::UnknownLanguage)?;
    let tmpdir = TmpDir::new().await?;

    // write source file into disk
    let source = tmpdir.filename(format!("Main.{}", language.extension()));
    tokio::fs::write(&source, self.code).await?;

    // get compile job and executable from language
    let (job, executable) = language.compile(&tmpdir, &source);
    if let Some(job) = job {
      // execute compile job
      let output = job.output().await?;
      let stdout = String::from_utf8_lossy(&output.stdout[..]).to_string();
      let stderr = String::from_utf8_lossy(&output.stderr[..]).to_string();
      if output.status.success() {
        Ok(CompileResult {
          stdout,
          stderr,
          executable,
          tmpdir,
        })
      } else {
        Err(CompileError::Failed(
          format!("{stdout}\n{stderr}").trim().to_string(),
        ))?
      }
    } else {
      Ok(CompileResult {
        stdout: String::new(),
        stderr: String::new(),
        executable,
        tmpdir,
      })
    }
  }
}
