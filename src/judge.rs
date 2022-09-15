use std::error::Error;
use std::fs::File;
use std::io::{BufReader, Read};
use std::iter;
use std::path::PathBuf;
use tokio::io::AsyncReadExt;

use crate::job::IOSource;
use crate::wire::Task;
pub enum Score {
    AC,
    WA,
    PE,
    PC(i32),
    Fail,
}
pub fn check_answer(a: &str, b: &str) -> bool {
    let a_lines: Vec<&str> = a.split_terminator('\n').map(|x| x.trim()).collect();
    let b_lines: Vec<&str> = b.split_terminator('\n').map(|x| x.trim()).collect();
    if a_lines.len() != b_lines.len() {
        return false;
    }
    if a_lines.len() == 0 {
        return true;
    } else {
        iter::zip(a_lines, b_lines)
            .map(|x| x.0 == x.1)
            .reduce(|x, y| x & y)
            .unwrap_or(false)
    }
}

pub fn check_answer_file(a: &str, b: &str) -> Result<bool, std::io::Error> {
    let str_a = std::fs::read_to_string(a)?;
    let str_b = std::fs::read_to_string(a)?;
    Ok(check_answer(&str_a, &str_b))
}

pub fn check_answer_exact(a: &str, b: &str) -> bool {
    let a_lines: Vec<&str> = a.split_terminator('\n').collect();
    let b_lines: Vec<&str> = b.split_terminator('\n').collect();
    if a_lines.len() != b_lines.len() {
        return false;
    } else {
        iter::zip(a_lines, b_lines)
            .map(|x| x.0 == x.1)
            .reduce(|x, y| x & y)
            .unwrap_or(false)
    }
}

pub async fn check_answer_open(
    a: &mut tokio::fs::File,
    b: &mut tokio::fs::File,
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    //let mut file_a = a.open().await?;
    //let mut file_b = b.open().await?;
    let mut ans_a = String::new();
    let mut ans_b = String::new();
    a.read_to_string(&mut ans_a).await?;
    b.read_to_string(&mut ans_b).await?;
    Ok(check_answer(ans_a.as_ref(), ans_b.as_ref()))
}

#[test]
fn check_answer_test() {
    assert!(check_answer("", ""));
    assert!(check_answer("hello world", "hello world\n"));
    assert!(check_answer("abc", "abc"));
    assert!(!check_answer("abc", "abcd"));
    assert!(check_answer("ab\nc", "ab  \nc     "));
    assert!(check_answer("ab\nc", "ab  \nc     \n"));
    assert!(check_answer("ab\nab\nab\n", "ab\nab\nab\n"));
    assert!(!check_answer("ab\nab\n\nab\n", "ab\nab\nab\n"));
    assert!(!check_answer("ab\nab\nab\n", "ab\nab\nabc\n"));
}

#[tokio::test]
async fn check_answer_file_test() {
    check_answer_open(
        &mut IOSource::Null.open().await.unwrap(),
        &mut IOSource::Null.open().await.unwrap(),
    )
    .await
    .unwrap();
}
