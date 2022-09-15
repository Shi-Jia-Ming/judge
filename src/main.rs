mod backend;
mod config;
mod conn;
mod file;
mod job;
mod judge;
mod lang;
mod preset;
mod runner;
mod seccomp;
mod usage;
mod util;
mod wait;
mod wire;
mod cgroup;

use crate::backend::*;
use crate::job::*;
use crate::seccomp::Seccomp;
use cgroups_rs::cgroup_builder::*;
use cgroups_rs::CgroupPid;
use cgroups_rs::{Controller, Hierarchy};
use clap::{Arg, ArgGroup, Args, Parser, Subcommand, ValueEnum};
use conn::JudgeClient;
use futures_util::future;
use log::{error, info, warn};
use rlimit::*;
use serde_json;
use std::error::Error;
use std::fs;
use std::os::unix::prelude::{CommandExt, ExitStatusExt};
use std::process::exit;
use std::process::*;
use std::sync::Arc;
use wait::{Wait, WaitStatus};

/// OJ Judger for HITWHoj
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct CliArgs {
    /// Start judger service.
    #[clap(short, long, value_parser)]
    pub config: Option<String>, //s
}

#[cfg(target_os = "linux")]
fn main() {
    let args = CliArgs::parse();
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .env()
        .init()
        .unwrap();
    //If failed to create temporary directory,then bail out.
    fs::create_dir_all("/tmp/judger").unwrap();

    //TODO: Run with runner,not hard-coded!
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            loop {
                info!("Waiting for connection...");
                daemon_main().await.unwrap();
            }
        })
    /*
    let hier = Box::new(cgroups_rs::hierarchies::V2::new());
    //eprintln!("cgroup root : {}",hier.root().to_str().unwrap());
    let cg = CgroupBuilder::new("judger_limit")
        .memory()
            .kernel_memory_limit(32*1024*1024)
            .memory_hard_limit((args.memory*1024*1024).try_into().unwrap())
            .done()
        .cpu()
            .cpus("1".to_owned()) //Currently,no multi-threaded/processed program is allowed for OJ use.
            .done()
        .build(hier);
    //Open input file (if exists)
    let input_file = fs::File::open(&args.input).expect("failed to open input file!");
    let output_file = fs::File::options().write(true).create(true).open(&args.userout).expect("failed to open output file!");
    let cg_clone = cg.clone();

    //Create process
    let mut command = Command::new(args.program);
    unsafe {
        command.pre_exec(move || {
            //Install cgroups
            //TODO: Fix cgroups init.
            match cg_clone.add_task(CgroupPid::from(id() as u64)) {
                Ok(_) => (),
                Err(k) => eprintln!("{}",k),
            };
            //Put resource limits
            setrlimit(Resource::NOFILE, 20, 20)?;
            setrlimit(Resource::CPU,args.time,args.time)?;
            setrlimit(Resource::STACK,INFINITY,INFINITY)?;
            setrlimit(Resource::CORE, 0, 0)?; //No core dump needed
            setrlimit(Resource::NPROC,50,50)?;
            setrlimit(Resource::DATA,args.memory*1024*1024,args.memory*1024*1024)?;
            //Apply seccomp filter
            Seccomp::apply_rule_strict().unwrap();
            Ok(())
        })
        .stdin(input_file)
        .stdout(output_file);
    }
    let mut _child = command.spawn().expect("failed to execute!");

    //Collect child process manually!
    //TODO: using pid directly from process struct requires further investigation.
    let wait = Wait::wait_all().expect("Probably nothing to wait. Bailing out");
    //judge::check_answer_file(&args.userout, &args.userout).unwrap();
    let ret = job::JudgeResultPrint {
        memory : wait.memory(),
        time : wait.cputime(),
        result : match wait.exit_type()  {
            WaitStatus::Ok => match judge::check_answer_file(&args.userout, &args.stdout) {
                Ok(true) => JudgeResult::AC,
                Ok(false) => JudgeResult::WA,
                Err(_) => JudgeResult::RE,
            },
            WaitStatus::MLE => JudgeResult::MLE,
            WaitStatus::TLE => JudgeResult::TLE,
            WaitStatus::RE => JudgeResult::RE,
        }
    };
    let output = serde_json::to_string(&ret).expect("Failed to generate json,this should not happen.");
    println!("{}",output);
    */
}

async fn daemon_main() -> Result<(), Box<dyn Error>> {
    let (mut runner, mut server, mut jobs, mut syncer) =
        conn::WsServer::listen_wait_one("127.0.0.1:1145").await?;
    let recv = tokio::spawn(async move {
        let mut r = runner;
        r.recv_loop().await
    });
    let send = tokio::spawn(async move {
        let mut r = server;
        r.send_loop().await
    });
    let job_runner = tokio::spawn(async move {
        //job_run.run_loop(job, report);
        jobs.run_loop().await
    });
    let syncer = tokio::spawn(async move { syncer.run_loop().await });
    let result = tokio::join!(recv, send, job_runner, syncer);

    Ok(())
}

#[cfg(not(target_os = "linux"))]
#[cfg(not(target_arch = "x86-64"))]
compile_error!(
    "Only support Linux and x86-64 platform. To run on other platforms,please port the program."
);
