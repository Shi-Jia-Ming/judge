use thiserror::Error;

use super::step::compile::CompileError;
use crate::communicate::spec::CheckerSpec;
use std::path::PathBuf;

pub enum Checker {
  DefaultChecker,
  TestlibChecker {},
}

pub struct CheckResult {
  pub r#match: bool,
  pub message: String,
}

#[derive(Debug, Error)]
pub enum CheckError {
  #[error("failed to open file: {0}")]
  IoError(#[from] tokio::io::Error),
}

impl Checker {
  pub async fn new(checker: &Option<CheckerSpec>) -> Result<Self, CompileError> {
    match checker {
      Some(_) => todo!(),
      None => Ok(Checker::DefaultChecker),
    }
  }

  pub fn check(&self, stdout: &str, answer: &str) -> Result<CheckResult, CheckError> {
    match self {
      Checker::DefaultChecker => {
        let stdout = stdout
          .split_terminator('\n')
          .map(|s| s.trim_end())
          .collect::<Vec<_>>();
        let answer = answer
          .split_terminator('\n')
          .map(|s| s.trim_end())
          .collect::<Vec<_>>();

        if stdout.len() != answer.len() {
          return Ok(CheckResult {
            r#match: false,
            message: "Lines mismatch".to_string(),
          });
        }

        let lines = std::iter::zip(stdout, answer).map(|(out, ans)| out == ans);
        let mismatch = std::iter::zip(1.., lines)
          .find(|(_, same)| !same)
          .and_then(|(line, _)| Some(line));

        if let Some(line) = mismatch {
          Ok(CheckResult {
            r#match: false,
            message: format!("Wrong Answer at line {}", line),
          })
        } else {
          Ok(CheckResult {
            r#match: true,
            message: format!("Correct"),
          })
        }
      }

      Checker::TestlibChecker {} => todo!(),
    }
  }

  pub async fn check_file(
    &self,
    stdout: &PathBuf,
    answer: &PathBuf,
  ) -> Result<CheckResult, CheckError> {
    let stdout = tokio::fs::read(stdout).await?;
    let answer = tokio::fs::read(answer).await?;
    let stdout = String::from_utf8_lossy(&stdout);
    let answer = String::from_utf8_lossy(&answer);
    self.check(&stdout, &answer)
  }
}

#[cfg(test)]
mod test {
  use crate::judge::checker::Checker;

  fn check(answer: &str, stdout: &str) -> bool {
    Checker::DefaultChecker
      .check(stdout, answer)
      .unwrap()
      .r#match
  }

  #[test]
  fn check_answer_test() {
    assert!(check("", ""));
    assert!(check("hello world", "hello world\n"));
    assert!(check("abc", "abc"));
    assert!(check("ab\nc", "ab  \nc     "));
    assert!(check("ab\nc", "ab  \nc     "));
    assert!(check("ab\nc", "ab  \nc     \n"));
    assert!(check("ab\nab\nab\n", "ab\nab\nab\n"));
    assert!(!check("hello world", "hello  world"));
    assert!(!check("abc", "abcd"));
    assert!(!check("ab\nc\n", "ab\n c\n"));
    assert!(!check("ab\nab\n\nab\n", "ab\nab\nab\n"));
    assert!(!check("ab\nab\nab\n", "ab\nab\nabc\n"));
  }
}
