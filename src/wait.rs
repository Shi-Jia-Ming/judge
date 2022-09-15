use crate::usage;
use libc;
use std::mem;

#[cfg(target_family = "unix")]
#[derive(Debug)]
/// The Wait struct
///
/// The struct hold the exit status and resource usage of a sub-process
///
pub struct Wait {
    rusage: usage::Rusage,
    code: i32,
}

pub enum WaitStatus {
    Ok,
    TLE,
    MLE,
    RE,
    Kill,
}

#[cfg(target_family = "unix")]
impl Wait {
    pub fn wait_for(pid: i32) -> Option<Wait> {
        //This probably should be unsafe!
        let mut status: i32 = 0;
        let mut rusage = mem::MaybeUninit::uninit();
        let code = unsafe { libc::wait4(pid, &mut status, 0, rusage.as_mut_ptr()) };
        if code < 0 {
            return None;
        }
        let rusage = unsafe { rusage.assume_init() };
        Some(Wait {
            rusage: usage::Rusage::from(rusage),
            code: status,
        })
    }

    pub fn wait_all() -> Option<Wait> {
        Self::wait_for(-1)
    }

    pub fn is_signal(&self) -> bool {
        libc::WIFSIGNALED(self.code)
    }

    pub fn signal_code(&self) -> Option<i32> {
        if self.is_signal() {
            Some(libc::WTERMSIG(self.code))
        } else {
            None
        }
    }

    pub fn is_exited(&self) -> bool {
        libc::WIFEXITED(self.code)
    }

    pub fn exit_code(&self) -> Option<i32> {
        match self.is_exited() {
            true => Some(libc::WEXITSTATUS(self.code)),
            false => None,
        }
    }

    pub fn is_ok(&self) -> bool {
        if self.is_signal() {
            return false;
        }
        if self.is_exited() {
            return self.code == 0;
        }
        return false;
    }

    pub fn cputime(&self) -> i64 {
        self.rusage.get_run_time()
    }

    pub fn memory(&self) -> i64 {
        self.rusage.get_memory()
    }

    pub fn exit_type(&self) -> WaitStatus {
        if self.is_ok() {
            return WaitStatus::Ok;
        } else if self.is_signal() {
            eprintln!("signal {}", self.signal_code().unwrap());
            return match self.signal_code().unwrap() {
                libc::SIGXCPU => WaitStatus::TLE,
                libc::SIGKILL => WaitStatus::Kill,
                libc::SIGSEGV => WaitStatus::RE,
                libc::SIGXFSZ => WaitStatus::MLE,
                _ => WaitStatus::RE,
            };
        }
        WaitStatus::RE
    }
}
