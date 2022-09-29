use std::path::PathBuf;

use tokio::process::Command;

use super::{
  job::{executable::Executable, Job},
  tmpdir::TmpDir,
};

/// 编程语言
#[derive(Clone, Debug)]
pub enum Language {
  C11,
  Cpp17,
  Python3,
}

impl Language {
  pub fn name(&self) -> String {
    match self {
      Language::C11 => "C 11".to_string(),
      Language::Cpp17 => "C++ 17".to_string(),
      Language::Python3 => "Python 3".to_string(),
    }
  }

  pub fn extension(&self) -> String {
    match self {
      Language::C11 => "c".to_string(),
      Language::Cpp17 => "cpp".to_string(),
      Language::Python3 => "py".to_string(),
    }
  }

  pub fn from_ext(ext: &str) -> Option<Self> {
    match ext {
      "c" => Some(Language::C11),
      "cpp" => Some(Language::Cpp17),
      "py" => Some(Language::Python3),
      _ => None,
    }
  }

  /// Returns compile job and executable
  ///
  /// TODO Support soft encoding
  pub fn compile(&self, tmpdir: &TmpDir, source: &PathBuf) -> (Option<Job>, Executable) {
    match self {
      Language::C11 => {
        let dest = tmpdir.filename("Main");
        let mut command = Command::new("gcc");
        command
          .arg("-lm")
          .arg("-std=c11")
          .arg("--static")
          .arg("-Wall")
          .arg("-DONLINE_JUDGE")
          .arg("-o")
          .arg(&dest)
          .arg(source);
        (
          // FIXME: limit compile resource
          Some(Job::from_unlimited(command)),
          Executable::Elf(dest),
        )
      }
      Language::Cpp17 => {
        let dest = tmpdir.filename("Main");
        let mut command = Command::new("g++");
        command
          .arg("-lm")
          .arg("-std=c++17")
          .arg("--static")
          .arg("-Wall")
          .arg("-DONLINE_JUDGE")
          .arg("-o")
          .arg(&dest)
          .arg(source);
        (Some(Job::from_unlimited(command)), Executable::Elf(dest))
      }
      Language::Python3 => (
        None,
        Executable::Command {
          name: "python".to_string(),
          args: vec![source.to_string_lossy().to_string()],
        },
      ),
    }
  }
}
