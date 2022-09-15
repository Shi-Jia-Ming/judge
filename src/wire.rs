// Judger Wire Protocol
// Check hitwhoj:doc/ for spec.

use crate::file::{self, save_opened_file};
use crate::job::{IOSource, Job, JobError, JobResult, JobType, JobUsage};
use log::{debug, warn};
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use thiserror::Error;
use tokio::io::AsyncWriteExt;
#[derive(Error, Debug)]
pub enum TaskError {
    #[error("File `{0}` does not exist")]
    FileNotExist(String),
    #[error("No file specified for `{0}`")]
    NoFile(String),
    #[error("No tasks")]
    NoTask,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum Message {
    Ping,
    Pong,
    Error(WireError),
    Hello(Hello),
    Status(Status),
    Task(Task),
    Sync(SyncRequest), //Currently,only sync needs reply from website.
    Progress(Progress),
    Finish(Progress),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum RecvMessage {
    Ping,
    Task(Task),
    Sync(SyncResponse),
    Error(WireError),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum SendMessage {
    Pong,
    Error(WireError),
    Hello(Hello),
    Status(Status),
    Sync(SyncRequest),
    Progress(Progress),
    Finish(Progress),
    Accept { id: u32 },
    Reject { id: u32 },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum TaskStatus {
    Pending,
    Running,
    Skipped, //Not for Non-ACM type.
    Accepted,
    #[serde(rename = "Wrong Answer")]
    WrongAnswer,
    #[serde(rename = "Time Limit Exceeded")]
    TimeLimitExceeded,
    #[serde(rename = "Memory Limit Exceeded")]
    MemoryLimitExceeded,
    #[serde(rename = "Runtime Error")]
    RuntimeError,
    #[serde(rename = "System Error")]
    SystemError,
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum SubtaskStatus {
    Pending,
    Running,
    Accepted,
    #[serde(rename = "Wrong Answer")]
    WrongAnswer,
    #[serde(rename = "Time Limit Exceeded")]
    TimeLimitExceeded,
    #[serde(rename = "Memory Limit Exceeded")]
    MemoryLimitExceeded,
    #[serde(rename = "Runtime Error")]
    RuntimeError,
    #[serde(rename = "System Error")]
    SystemError,
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum JudgeStatus {
    Pending,
    Judging,
    Compiling,
    Running,
    #[serde(rename = "Compile Error")]
    CompileError,
    Accepted,
    #[serde(rename = "Wrong Answer")]
    WrongAnswer,
    #[serde(rename = "Time Limit Exceeded")]
    TimeLimitExceeded,
    #[serde(rename = "Memory Limit Exceeded")]
    MemoryLimitExceeded,
    #[serde(rename = "Runtime Error")]
    RuntimeError,
    #[serde(rename = "System Error")]
    SystemError,
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    C,
    Cpp,
    Py,
    Rust,
    Go,
    Java,
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Hello {
    pub version: String,
    pub cpus: i32,
    pub langs: Vec<String>,
    pub ext_features: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Status {
    pub cpus: i32,
    pub occupied: i32,
    pub queue: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TaskResult {
    pub id: u32,
    pub message: String,
    pub status: TaskStatus,
    pub time: u32,
    pub memory: u64,
}

impl From<JobResult> for TaskResult {
    fn from(r: JobResult) -> Self {
        TaskResult {
            id: 0,
            message: "judged".to_owned(),
            status: match r.judge_result {
                None => TaskStatus::Unknown,
                Some(crate::job::JudgeResult::AC) => TaskStatus::Accepted,
                Some(crate::job::JudgeResult::WA) => TaskStatus::WrongAnswer,
                Some(crate::job::JudgeResult::CE) => TaskStatus::WrongAnswer,
                Some(crate::job::JudgeResult::MLE) => TaskStatus::MemoryLimitExceeded,
                Some(crate::job::JudgeResult::OLE) => TaskStatus::MemoryLimitExceeded,
                Some(crate::job::JudgeResult::PE) => TaskStatus::WrongAnswer,
                Some(crate::job::JudgeResult::RE) => TaskStatus::RuntimeError,
                Some(crate::job::JudgeResult::TLE) => TaskStatus::TimeLimitExceeded,
                Some(_) => TaskStatus::Unknown,
            },
            time: r.usage.time as u32,
            memory: r.usage.memory as u64,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SubtaskResult {
    pub id: u32,
    pub message: String,
    pub status: SubtaskStatus,
    pub score: i32,
    pub tasks: Vec<TaskResult>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JudgeResult {
    pub message: String,
    pub status: JudgeStatus,
    pub score: i32,
    pub subtasks: Vec<SubtaskResult>,
}

impl JudgeResult {
    pub fn error(msg: String) -> Self {
        JudgeResult {
            message: msg,
            status: JudgeStatus::Unknown,
            score: 0,
            subtasks: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Progress {
    pub id: u32,
    pub result: JudgeResult,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SyncRequest {
    pub uuid: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SyncResponse {
    pub uuid: String,
    pub data: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WireError {
    pub code: String,
    pub msg: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Task {
    pub id: u32,
    pub code: String,
    pub language: String,
    pub files: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "type")]
pub enum TaskSpec {
    Default {
        #[serde(flatten)]
        common: CommonTask,
    },
    Interactive {
        #[serde(flatten)]
        common: CommonTask,
        interactive: String,
    },
    Dynamic {
        #[serde(flatten)]
        common: CommonTask,
        mkdata: String,
        std: String,
    },
    SubmitAnswer {
        user: String,
        answer: String,
    },
}

#[derive(Clone, Debug, Default)]
pub struct JobGroup {
    pub id: i32,
    pub score: i32, //Only scores if all jobs are AC.
    pub jobs: Vec<Job>,
}

#[derive(Clone, Debug, Default)]
pub struct JobGenerated {
    pub id: u32,
    pub before: Vec<Job>,
    //pub jobs : Vec<(i32,Vec<Job>)>,
    pub jobs: Vec<JobGroup>,
    pub after: Vec<Job>,
}

impl TaskSpec {
    /// This function only supports traditional oj now.
    pub async fn task_to_job_spec(
        &self,
        task: &Task,
    ) -> Result<JobGenerated, Box<dyn Error + Send + Sync>> {
        let mut common: Option<&CommonTask> = None;
        let mut interactive: Option<&str> = None;
        let mut mkdata: Option<&str> = None;
        let mut std: Option<&str> = None;
        let mut ret = JobGenerated::default();
        ret.id = task.id;
        let mut program: Option<String> = None;

        match self {
            TaskSpec::Default { common: _common } => {
                common = Some(_common);

                let (source_file, source_id) =
                    IOSource::mktempfile(Some(match task.language.as_ref() {
                        "cpp" => ".cpp",
                        "c" => ".c",
                        _ => "",
                    }));
                warn!("{}", source_file.get_path().unwrap());
                let mut source_handle = source_file.create_and_open().await?;
                //source_handle.write_all(task.code.as_bytes()).await?;
                //let code = base64::decode(&task.code)?;
                let code = task.code.clone().into();
                save_opened_file(code, source_handle).await;
                let compile_log_file =
                    IOSource::opentempfile(source_id.as_str(), Some("_compile.log"));
                let source_file_path = source_file
                    .get_path()
                    .ok_or(TaskError::NoFile("source".to_owned()))?;
                let program_path = format!("/tmp/judger/{}.run", source_id);
                program = Some(program_path.clone());
                //source_file is file backed so get_path() always return Some(_).
                let c_param = vec![
                    "-O2".to_owned(),
                    "-o".to_owned(),
                    program_path.clone(),
                    source_file_path.clone(),
                ];
                let cpp_param = vec![
                    "-O2".to_owned(),
                    "-o".to_owned(),
                    program_path.clone(),
                    source_file_path.clone(),
                ];
                let (compile_command, compile_param, run_command) = match task.language.as_ref() {
                    "c" => ("gcc", c_param, ""),
                    "cpp" => ("g++", cpp_param, ""),
                    _ => unimplemented!(),
                };
                ret.before.push(Job {
                    program_path: compile_command.to_string(),
                    //program_path: format!("{} {} {}{} -o {}","gcc","-O2",source_id,".cpp",source_id).into(),
                    param: compile_param,
                    job_type: JobType::Compile,
                    limit: JobUsage {
                        time: 1000 * 15,           //15s
                        memory: 768 * 1024 * 1024, //768M
                    },
                    stdin: IOSource::Null,
                    stdout: IOSource::Null,
                    answer: IOSource::Null,
                    stderr: compile_log_file,
                    before: vec![],
                    after: vec![],
                })
            }
            TaskSpec::Interactive {
                common: _common,
                interactive: _interactive,
            } => {
                common = Some(_common);
                interactive = Some(_interactive.as_ref());
                Err(JobError::WaitError)?
            }
            TaskSpec::Dynamic {
                common: _common,
                mkdata: _mkdata,
                std: _std,
            } => {
                common = Some(_common);
                mkdata = Some(_mkdata);
                std = Some(_std);
                Err(JobError::WaitError)?;
            }
            _ => {
                Err(JobError::WaitError)?;
            }
        }
        let program_ref = program.as_ref();
        for (i, subtask) in common
            .ok_or(TaskError::NoTask)?
            .subtasks
            .as_slice()
            .into_iter()
            .enumerate()
        {
            let mut items: Vec<Job> = vec![];
            for (j, onetask) in subtask.cases.as_slice().into_iter().enumerate() {
                //A test point in a subtask.
                let job = Job {
                    program_path: program_ref.unwrap().into(),
                    param: vec![],
                    limit: JobUsage {
                        //This unwrap should always succeed.
                        time: onetask
                            .time
                            .unwrap_or(subtask.time.unwrap_or(common.unwrap().time.unwrap_or(1000)))
                            as i64,
                        memory: onetask
                            .memory
                            .unwrap_or(subtask.memory.unwrap_or(common.unwrap().memory.unwrap_or(256*1024*1024)))
                            as i64,
                    },
                    stdin: IOSource::File(
                        format!(
                            "/var/judger/data/{}",
                            task.files.get(onetask.input.as_ref().unwrap()).unwrap()
                        )
                        .into(),
                    ),
                    stdout: IOSource::mktempfile(Some(".out")).0,
                    stderr: IOSource::Null,
                    answer: IOSource::File(
                        format!(
                            "/var/judger/data/{}",
                            task.files
                                .get(onetask.output.as_ref().expect("This "))
                                .unwrap()
                        )
                        .into(),
                    ),
                    job_type: JobType::Oj,
                    before: vec![],
                    after: vec![],
                };
                items.push(job);
            }
            let new_group = JobGroup {
                id: i as i32,
                score: subtask.score,
                jobs: items,
            };
            ret.jobs.push(new_group);
        }
        Ok(ret)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CommonTask {
    time: Option<u32>,
    memory: Option<u64>,
    subtasks: Vec<SubTask>,
    #[serde(rename = "checkerType")]
    checker_type: Option<String>,
    #[serde(rename = "checkerName")]
    checker_name: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TaskInfo {
    time: u32,
    memory: u64,
    subtasks: Vec<i32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SubTask {
    score: i32,
    time: Option<u32>,
    memory: Option<u64>,
    cases: Vec<SingleTask>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SingleTask {
    input: Option<String>,  //Required for traditional + spj
    output: Option<String>, //Required for traditional
    time: Option<u32>,
    memory: Option<u64>,
    args: Option<String>, //For Task::Dynamic only
}

#[test]
fn test_message() {
    let msg = Hello {
        version: "v0".to_owned(),
        cpus: 16,
        ext_features: vec![],
        langs: vec![],
    };
    let b = serde_json::to_string(&Message::Hello(msg)).unwrap();
    println!("{}", b);
    let msg: SendMessage = serde_json::from_str(
        "{\"type\":\"hello\",\"version\":\"v0\",\"cpus\":16,\"langs\":[],\"ext_features\":[]}",
    )
    .unwrap();
    println!("{:?}", msg);
}
