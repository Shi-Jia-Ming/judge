use async_trait::async_trait;
//use std::fs::File;
use cgroups_rs::cgroup_builder::{CgroupBuilder, CpuResourceBuilder};
use cgroups_rs::CgroupPid;
use log::{debug, error, info, log, warn};
use rlimit::{setrlimit, Resource, INFINITY};
use serde_derive::{self, Deserialize, Serialize};
use serde_json;
use std::borrow::Cow;
use std::env;
use std::error::Error;
use std::os::unix::prelude::RawFd;
use std::os::unix::process::CommandExt;
use std::path::*;
use std::process::{id, Command};
use std::sync::Arc;
use tokio::fs;
use tokio::fs::File;
use tokio::sync::{mpsc, Semaphore};
use uuid::Uuid;

use crate::cgroup::V2Path;
use crate::seccomp::{Seccomp, SeccompType};
use crate::util::Logable;
use crate::wait::{Wait, WaitStatus};
use crate::wire::{JobGenerated, JudgeStatus, Progress, SendMessage, TaskResult, TaskStatus};
use crate::{judge, wire};
use std::os::unix::io::FromRawFd;
use thiserror::Error;
#[derive(Error, Debug)]
pub enum JobError {
    #[error("Failed to wait.")]
    WaitError,
    #[error("Not supported.")]
    NotSupportedError,
    #[error("Unknown failure in execution.")]
    UnknownExecError,
    #[error("Failed to execute pre job.")]
    PreJobError,
}

#[derive(Error, Debug)]
pub enum FileError {
    #[error("IO Source type does not support creation.")]
    NotSupportedError,
    #[error("Is a directory")]
    IsDirectory,
}

//Job presets for basic constraints.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum JobType {
    Compile,    //No stdin,with stdout to buffer,can write files to directory.
    Oj,         //Classic OJ workstream,stdin and stdout from file.
    Checker,    //stdout goes through another program.
    Interactor, //TODO: Interactor and submission program talk through pipe.
    Validator,  //Check constraints for user inputs,stdin only
    Generator,  //TODO: Generate data for input.
    Design,     //Reserved,for projects.
    Unlimited,  //Dangerous,should not use!
}

impl JobType {
    fn to_scmp_type(&self) -> SeccompType {
        match self {
            JobType::Compile | JobType::Design => SeccompType::Compile,
            JobType::Checker | JobType::Interactor | JobType::Validator | JobType::Generator => {
                SeccompType::Relaxed
            }
            JobType::Oj => SeccompType::Strict,
            JobType::Unlimited => SeccompType::Unlimited,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum JudgeResult {
    AC,
    RE,
    MLE,
    OLE,
    CE,
    PE, //No usage.
    WA,
    TLE,
    UKE,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ExecResult {
    Ok,
    Err(i32),
    Signal(i32),
    Timeout,
    Memory,
    Unknown,
}

/// IOSource for read-write based file descriptors.
/// Note: Pipe only have limited size of buffer!
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum IOSource {
    File(PathBuf),
    NamedPipe(PathBuf),
    OpenedFile(i32),
    Pipe(i32),
    Dir(PathBuf),
    Null, //Opens /dev/null on *nix systems.
    None, //no-op
}

impl IOSource {
    pub fn mktempdir() -> Self {
        let mut path = env::temp_dir();
        let id = uuid::Uuid::new_v4().to_string();
        path.push(id);
        //let dir_id = Uuid
        IOSource::Dir(path)
    }

    pub fn mktempfile(postfix: Option<&str>) -> (Self, String) {
        let mut path = env::temp_dir();
        path.push("judger");
        let id = uuid::Uuid::new_v4().to_string();
        path.push(format!("{}{}", id, postfix.unwrap_or("")));
        //let dir_id = Uuid
        (IOSource::File(path), id)
    }

    pub fn opentempfile(name: &str, postfix: Option<&str>) -> Self {
        let mut path = env::temp_dir();
        path.push("judger");
        path.push(format!("{}{}", name, postfix.unwrap_or("")));
        //let dir_id = Uuid
        IOSource::File(path)
    }

    pub async fn open(&self) -> Result<File, Box<dyn Error + Send + Sync>> {
        match self {
            IOSource::File(k) | IOSource::NamedPipe(k) => {
                let file = fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(k)
                    .await?;
                Ok(file)
            }
            IOSource::OpenedFile(k) | IOSource::Pipe(k) => {
                let file: File = unsafe { FromRawFd::from_raw_fd(*k) };
                Ok(file)
            }
            IOSource::Dir(_) => Err(FileError::IsDirectory)?,
            IOSource::Null | IOSource::None => {
                #[cfg(target_family = "unix")]
                return Ok(File::open("/dev/null").await?);
            }
            _ => Err(FileError::IsDirectory)?,
        }
    }

    pub async fn create_and_open(&self) -> Result<File, Box<dyn Error + Send + Sync>> {
        match self {
            IOSource::File(k) | IOSource::NamedPipe(k) => {
                let file = fs::OpenOptions::new()
                    .create(true)
                    .read(true)
                    .write(true)
                    .truncate(true)
                    .open(k)
                    .await?;
                Ok(file)
            }
            IOSource::OpenedFile(k) | IOSource::Pipe(k) => {
                let file: File = unsafe { FromRawFd::from_raw_fd(*k) };
                Ok(file)
            }
            IOSource::Dir(_) => Err(FileError::IsDirectory)?,
            IOSource::Null | IOSource::None => {
                #[cfg(target_family = "unix")]
                return Ok(File::open("/dev/null").await?);
            }
            _ => Err(FileError::IsDirectory)?,
        }
    }

    /// This does not open file.
    /// Use open calls to open.
    pub async fn create_new(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        match self {
            IOSource::File(k) => {
                let _file = tokio::fs::OpenOptions::new()
                    .create_new(true)
                    .open(k)
                    .await?;
                return Ok(());
            }
            IOSource::NamedPipe(k) => {
                interprocess::os::unix::fifo_file::create_fifo(k, 0o777)?;
                return Ok(());
            }
            IOSource::Dir(k) => {
                fs::create_dir_all(k).await?;
                return Ok(());
            }
            _ => {
                Err(FileError::NotSupportedError)?;
            }
        }
        Err(JobError::WaitError)?
    }

    pub fn get_path(&self) -> Option<String> {
        match self {
            IOSource::Dir(k) | IOSource::File(k) | IOSource::NamedPipe(k) => {
                Some(k.display().to_string())
            }
            IOSource::None | IOSource::Null => Some("/dev/null".to_owned()),
            _ => None,
        }
    }

    pub async fn delete(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        match self {
            IOSource::File(k) | IOSource::NamedPipe(k) => {
                fs::remove_file(k).await?;
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Job {
    pub program_path: String,
    pub param: Vec<String>,
    pub job_type: JobType,
    pub limit: JobUsage,
    pub stdin: IOSource,
    pub stdout: IOSource,
    pub answer: IOSource,
    pub stderr: IOSource,
    pub before: Vec<Job>,
    pub after: Vec<Job>,
}

impl Job {
    fn new(program: String, job_type: JobType, input: PathBuf, output: PathBuf) -> Job {
        Self {
            program_path: program,
            param: vec![],
            job_type: job_type,
            limit: JobUsage {
                time: 1000,
                memory: 1024,
            },
            stdin: IOSource::None,
            stdout: IOSource::None,
            answer: IOSource::None,
            stderr: IOSource::None,
            before: vec![],
            after: vec![],
        }
    }
}

#[async_trait]
pub trait JobRunner {
    async fn run(&mut self, job: &Job) -> Result<JobResult, Box<dyn Error + Send + Sync>>;
    async fn run_loop(
        &mut self,
        jobs: mpsc::Receiver<JobGenerated>,
        report: mpsc::Sender<wire::SendMessage>,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}

#[derive(Debug, Clone)]
pub struct LocalRunner;

#[derive(Debug)]
pub struct LocalRunnerClient {
    jobs: mpsc::Receiver<JobGenerated>,
    send: mpsc::Sender<wire::SendMessage>,
    runner: LocalRunner,
}
impl LocalRunnerClient {
    pub fn new(
        jobs: mpsc::Receiver<JobGenerated>,
        send: mpsc::Sender<wire::SendMessage>,
        runner: LocalRunner,
    ) -> Self {
        Self { jobs, send, runner }
    }

    pub async fn run_loop(mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.runner.run_loop(self.jobs, self.send).await
    }
}

#[async_trait]
impl JobRunner for LocalRunner {
    /// Job sync runner
    async fn run(&mut self, job: &Job) -> Result<JobResult, Box<dyn Error + Send + Sync>> {
        //Read Job spec
        //Run pre-job
        debug!("job {:?}", job);
        for i in job.before.as_slice() {
            let ret = self.run(i).await?;
            //If data generator?
            /*
            match &i.job_type {
                JobType::Compile => {
                }
                _ => {

                }
            }
            */
            match ret.exec_result {
                ExecResult::Ok => {
                    //Nothing to do. Propably use `if let` instead.
                }
                e => {
                    error!("job error {:?}", e);
                    Err(JobError::PreJobError)?;
                }
            }
        }
        //Split work process
        //let hier = Box::new(cgroups_rs::hierarchies::V2::new());
        //let hier = Box::new(V2Path::from_path("/sys/fs/cgroup/jugder"));
        let hier = Box::new(V2Path::from_path("/sys/fs/cgroup/judger/"));
        //eprintln!("cgroup root : {}",hier.root().to_str().unwrap());
        let cg_group = format!("judger_{}", Uuid::new_v4());
        let cg = CgroupBuilder::new(cg_group.as_ref())
            .memory()
            .kernel_memory_limit(job.limit.memory / 4)
            .memory_hard_limit((job.limit.memory).try_into()?)
            .done()
            .cpu()
            .cpus("1".to_owned()) //Currently,no multi-threaded/processed program is allowed for common OJ use.
            .done()
            .pid()
            .maximum_number_of_processes(cgroups_rs::MaxValue::Value(1000))
            .done()
            .build(hier);
        //Open input file (if exists)
        let input_file = job.stdin.open().await?;
        let output_file = job.stdout.create_and_open().await?;
        let stderr_file = job.stderr.create_and_open().await?;
        let cg_clone = cg.clone();

        //Create process
        let mut command = Command::new(&job.program_path);
        command.args(job.param.as_slice());
        let job_type = job.job_type.clone();
        let job_inner = job.clone();
        let mem_limit = job.limit.memory;
        let time_limit = job.limit.time;
        unsafe {
            command
                .pre_exec(move || {
                    //Install cgroups
                    //TODO: Fix cgroups init.
                    match cg_clone.add_task(CgroupPid::from(id() as u64)) {
                        Ok(_) => (),
                        Err(k) => eprintln!("{}", k),
                    };
                    //Put resource limits
                    setrlimit(Resource::NOFILE, 5000, 5000)?; //TODO:Apply setting for different presets.
                    setrlimit(
                        Resource::CPU,
                        (time_limit / 1000) as u64,
                        (time_limit / 1000) as u64,
                    )?;
                    setrlimit(Resource::STACK, INFINITY, INFINITY)?;
                    setrlimit(Resource::CORE, 0, 0)?; //No core dump needed
                                                      //TODO: Use pids.max in cgroups to achieve process limits.
                                                      //NPROC is per-user limit,not per-process limit.
                                                      //setrlimit(Resource::NPROC,INFINITY,INFINITY)?;
                    setrlimit(Resource::DATA, mem_limit as u64, mem_limit as u64)?;
                    //Apply seccomp filter
                    //When filter failed to apply,the errno is EINVAL.
                    //Seccomp::apply_rule(&job_type.to_scmp_type()).or(Err(std::io::Error::from_raw_os_error(22)))?;
                    Ok(())
                })
                .stdin(input_file.into_std().await)
                .stdout(output_file.into_std().await)
                .stderr(stderr_file.into_std().await);
        }
        let child = command.spawn()?;
        //File does not Copy so we open again.
        //This open file is used to check answer.
        let input_file = job.stdin.open().await?;
        let mut output_file = job.stdout.open().await?;
        let stderr_file = job.stderr.open().await?;
        //Collect child process manually!
        //TODO: using pid directly from process struct requires further investigation.
        let wait = Wait::wait_for(child.id() as i32).ok_or(JobError::WaitError)?;
        //Run post job

        for i in job.after.as_slice() {
            self.run(i).await?;
        }

        //judge::check_answer_file(&args.userout, &args.userout).unwrap();
        let correct = match &job.answer {
            IOSource::None | IOSource::Null => Ok(true),
            k => judge::check_answer_open(&mut output_file, &mut k.open().await?).await,
        };
        /*
        let ret = JudgeResultPrint {
            memory : wait.memory(),
            time : wait.cputime(),
            result : match wait.exit_type() {
                WaitStatus::Ok => match correct {
                    Ok(true) => JudgeResult::AC,
                    Ok(false) => JudgeResult::WA,
                    Err(_) => JudgeResult::UKE,
                },
                WaitStatus::MLE => JudgeResult::MLE,
                WaitStatus::TLE => JudgeResult::TLE,
                WaitStatus::RE => JudgeResult::RE,
                WaitStatus::Kill => if wait.cputime() + 50 > job.limit.time {JudgeResult::TLE} else {JudgeResult::RE}
            }
        };
        */
        //TODO:Delete temporary file.
        //Delete cg group.
        cg.delete().log();
        let (judge_result, exec_result) = match wait.exit_type() {
            WaitStatus::Ok => (
                match correct {
                    Ok(true) => JudgeResult::AC,
                    Ok(false) => JudgeResult::WA,
                    Err(_) => JudgeResult::UKE,
                },
                ExecResult::Ok,
            ),
            WaitStatus::MLE => (JudgeResult::MLE, ExecResult::Memory),
            WaitStatus::TLE => (JudgeResult::TLE, ExecResult::Timeout),
            WaitStatus::RE => (
                JudgeResult::RE,
                if wait.is_exited() {
                    ExecResult::Err(wait.exit_code().unwrap_or(-1))
                } else {
                    ExecResult::Signal(wait.signal_code().unwrap_or(-1))
                },
            ),
            WaitStatus::Kill => {
                if wait.cputime() + 50 > job.limit.time {
                    (JudgeResult::TLE, ExecResult::Timeout)
                } else {
                    (JudgeResult::RE, ExecResult::Unknown)
                }
            }
        };
        //let output = serde_json::to_string(&ret)?;
        //println!("{}",output);
        Ok(JobResult {
            usage: JobUsage {
                memory: wait.memory(),
                time: wait.cputime(),
            },
            exec_result: exec_result,
            judge_result: Some(judge_result),
            product_path: None,
        })
    }

    async fn run_loop(
        &mut self,
        jobs: mpsc::Receiver<JobGenerated>,
        report: mpsc::Sender<wire::SendMessage>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut q = jobs;
        let mut sem = Arc::new(Semaphore::new(num_cpus::get()));
        //Run jobs
        while let Some(k) = q.recv().await {
            debug!("job {:?}", k);
            let mut failed = false;
            for i in &k.before {
                //Just bail out on error,as all before/after errors instantly terminates execution.
                let jobresult = self.run(i).await;
                match jobresult {
                    Err(k) => {
                        error!("A job has failed to exec its prerequisite,reason {}.", k);
                        //Drop current job,get a new job.
                        failed = true;
                        break;
                    }
                    Ok(r) => {
                        debug!("{:?}", r);
                        match r.exec_result {
                            ExecResult::Ok => {}
                            _ => {
                                warn!("A job has failed.");
                                break;
                            }
                        }
                    }
                }
            }

            if failed {
                continue;
            }

            let mut judge_result = crate::wire::JudgeResult {
                message: "ok".to_owned(),
                status: wire::JudgeStatus::Judging,
                score: 0,
                subtasks: vec![],
            };

            for jobgroup in k.jobs.iter() {
                let i = jobgroup.id;
                let subtask = jobgroup.jobs.as_slice();
                let mut subtask_result = wire::SubtaskResult {
                    id: i as u32,
                    message: "ok".into(),
                    status: wire::SubtaskStatus::Running,
                    score: 0,
                    tasks: vec![],
                };

                for point in subtask {
                    //If errors happen,mark as UKE instead of stoping all execution.
                    let status = self.run(&point).await;
                    /*
                    let status = status.unwrap_or(JobResult {
                        exec_result:  ExecResult::Unknown,
                        judge_result: Some(JudgeResult::RE),
                        product_path: None,
                        usage : JobUsage {
                            time: 0,
                            memory: 0
                        }
                    });
                    */
                    let status = status.unwrap();
                    debug!("{:?}", status);
                    subtask_result.tasks.push(status.into());
                    //Calculate points.
                }
                //Set subtask result.
                let subtask_ok = subtask_result.tasks.iter().all(|f| match f.status {
                    TaskStatus::Accepted => true,
                    _ => false,
                });
                subtask_result.score += if subtask_ok { jobgroup.score } else { 0 };

                //Get result and publish partial result.
                judge_result.subtasks.push(subtask_result);
                report
                    .send(SendMessage::Progress(Progress {
                        id: k.id,
                        result: judge_result.clone(),
                    }))
                    .await?;
            }
            //Send all results.
            judge_result.score = judge_result.subtasks.iter().map(|a| a.score).sum();
            judge_result.status = match judge_result.score {
                0 => JudgeStatus::WrongAnswer,
                100 => JudgeStatus::Accepted,
                _ => JudgeStatus::WrongAnswer, //There should be something of Partial correct.
            };

            //Postprocess results.
            for i in judge_result.subtasks.iter_mut() {
                let a = i
                    .tasks
                    .iter()
                    .all(|f| matches!(f.status, TaskStatus::Accepted));
                i.status = match a {
                    true => wire::SubtaskStatus::Accepted,
                    false => wire::SubtaskStatus::WrongAnswer,
                }
            }

            //We don't care for post-jobs.
            //Global post job is not a part of scoring.
            for i in &k.after {
                if let Err(k) = self.run(i).await {
                    warn!("job: failed to run post jobs,info {}", k);
                }
            }
            //Send response. This should not fail.
            report
                .send(SendMessage::Finish(Progress {
                    id: k.id,
                    result: judge_result,
                }))
                .await
                .log();
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JobUsage {
    pub time: i64,   //microseconds
    pub memory: i64, //bytes
}

#[derive(Debug, Clone)]
pub struct JobResult {
    pub usage: JobUsage,
    pub exec_result: ExecResult,
    pub judge_result: Option<JudgeResult>,
    pub product_path: Option<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JudgeResultPrint {
    pub memory: i64, //In kilo-bytes.
    pub time: i64,   //In microseconds
    pub result: JudgeResult,
}

#[tokio::test]
async fn test_job_runner() {
    let job = Job {
        program_path: "/home/pomoke/c/c/loop".to_string(),
        job_type: JobType::Oj,
        stdin: IOSource::Null,
        stdout: IOSource::Null,
        stderr: IOSource::File("/dev/stderr".into()),
        param: vec![],
        limit: JobUsage {
            time: 1000,
            memory: 256 * 1024 * 1024,
        },
        answer: IOSource::Null,
        before: vec![],
        after: vec![],
    };
    let mut runner = LocalRunner {};
    let result = runner.run(&job).await.unwrap();
    println!("{:?}", result);
}
