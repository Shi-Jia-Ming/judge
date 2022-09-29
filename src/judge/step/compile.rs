use async_trait::async_trait;
use std::process::ExitStatusError;
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
  #[error("failed to compile: {0}")]
  JobError(#[from] JobError),
  #[error("unknown language")]
  UnknownLanguage,
  #[error("failed to compile: {0}")]
  ExitStatusError(#[from] ExitStatusError),
}

pub struct CompileResult {
  pub success: bool,
  pub stdout: String,
  pub stderr: String,
  pub executable: Executable,
}

#[async_trait]
pub trait Compile {
  /// Compiles the code with provided tmp dir
  async fn compile(self, tmpdir: &TmpDir) -> Result<CompileResult, CompileError>;
}

pub struct Compiler {
  code: Vec<u8>,
  language: String,
}

impl Compiler {
  pub fn new(code: Vec<u8>, language: String) -> Self {
    Self { code, language }
  }
}

#[async_trait]
impl Compile for Compiler {
  async fn compile(self, tmpdir: &TmpDir) -> Result<CompileResult, CompileError> {
    let language =
      Language::from_ext(self.language.as_str()).ok_or(CompileError::UnknownLanguage)?;

    // write source file into disk
    let source = tmpdir.filename(format!("Main.{}", language.extension()));
    tokio::fs::write(&source, self.code).await?;

    // get compile job and executable from language
    let (job, executable) = language.compile(&tmpdir, &source);
    if let Some(job) = job {
      // execute compile job
      let output = job.output().await?;
      Ok(CompileResult {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout[..]).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr[..]).to_string(),
        executable,
      })
    } else {
      Ok(CompileResult {
        success: true,
        stdout: String::new(),
        stderr: String::new(),
        executable,
      })
    }
  }
}
