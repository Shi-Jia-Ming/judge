use libc;
use seccompiler::*;
use std::convert::TryInto;
pub struct Seccomp {
    program: Option<BpfProgram>,
}

pub enum SeccompType {
    Strict,
    Compile,
    Relaxed,
    Unlimited,
}

impl Seccomp {
    pub fn new() -> Self {
        Self { program: None }
    }

    pub fn make_filter(&mut self, rule: Vec<(i64, Vec<SeccompRule>)>) {
        self.program = match SeccompFilter::new(
            rule.into_iter().collect(),
            SeccompAction::Allow,
            SeccompAction::Allow,
            std::env::consts::ARCH.try_into().unwrap(),
        ) {
            Ok(k) => Some(k.try_into().unwrap()),
            Err(e) => None,
        }
    }

    pub fn apply(&self) -> Result<()> {
        if let Some(k) = &self.program {
            return seccompiler::apply_filter(&k);
        }
        Err(seccompiler::Error::EmptyFilter)
    }

    // TODO: Use profile-guided filters.
    pub fn apply_rule_strict() -> Result<()> {
        let mut filter = Self::new();
        filter.make_filter(vec![
            (libc::SYS_read, vec![]),
            (libc::SYS_write, vec![]),
            (libc::SYS_close, vec![]),
            (libc::SYS_mmap, vec![]),
            (libc::SYS_mprotect, vec![]),
            (libc::SYS_munmap, vec![]),
            (libc::SYS_brk, vec![]),
            (libc::SYS_pread64, vec![]),
            (libc::SYS_access, vec![]),
            (libc::SYS_open, vec![]),
            (libc::SYS_openat, vec![]),
            (libc::SYS_newfstatat, vec![]),
            (libc::SYS_lseek, vec![]),
            (libc::SYS_arch_prctl, vec![]),
            (libc::SYS_execve, vec![]),
            (libc::SYS_execveat, vec![]),
            (libc::SYS_exit_group, vec![]),
            (libc::SYS_exit, vec![]),
        ]);
        filter.apply()?;
        Ok(())
    }

    pub fn apply_rule_compile() -> Result<()> {
        let mut filter = Self::new();
        filter.make_filter(vec![
            (libc::SYS_futex, vec![]),
            (libc::SYS_read, vec![]),
            (libc::SYS_write, vec![]),
            (libc::SYS_clock_gettime, vec![]),
            (libc::SYS_newfstatat, vec![]),
            (libc::SYS_fstat, vec![]),
            (libc::SYS_readlink, vec![]),
            (libc::SYS_pipe2, vec![]),
            (libc::SYS_dup2, vec![]),
            (libc::SYS_close, vec![]),
            (libc::SYS_mmap, vec![]),
            (libc::SYS_mprotect, vec![]),
            (libc::SYS_munmap, vec![]),
            (libc::SYS_brk, vec![]),
            (libc::SYS_pread64, vec![]),
            (libc::SYS_madvise, vec![]),
            (libc::SYS_fcntl, vec![]),
            (libc::SYS_nanosleep, vec![]),
            (libc::SYS_epoll_ctl, vec![]),
            (libc::SYS_epoll_pwait, vec![]),
            (libc::SYS_access, vec![]),
            (libc::SYS_faccessat2, vec![]),
            (libc::SYS_faccessat, vec![]),
            (libc::SYS_recvmsg, vec![]),
            (libc::SYS_sendmsg, vec![]),
            (libc::SYS_open, vec![]),
            (libc::SYS_openat, vec![]),
            (libc::SYS_getrandom, vec![]),
            (libc::SYS_clock_gettime, vec![]),
            (libc::SYS_unlink, vec![]),
            (libc::SYS_set_robust_list, vec![]),
            (libc::SYS_getpid, vec![]),
            (libc::SYS_getuid, vec![]),
            (libc::SYS_getgid, vec![]),
            (libc::SYS_sysinfo, vec![]),
            (libc::SYS_getresgid, vec![]),
            (libc::SYS_getresuid, vec![]),
            (libc::SYS_waitid, vec![]),
            (libc::SYS_sched_yield, vec![]),
            (libc::SYS_ioctl, vec![]),
            (libc::SYS_socket, vec![]),
            (libc::SYS_connect, vec![]),
            (libc::SYS_getsockname, vec![]),
            (libc::SYS_getpeername, vec![]),
            (libc::SYS_getpeername, vec![]),
            (libc::SYS_epoll_create1, vec![]),
            (libc::SYS_getdents64, vec![]),
            (libc::SYS_uname, vec![]),
            (libc::SYS_umask, vec![]),
            (libc::SYS_chdir, vec![]),
            (libc::SYS_readlink, vec![]),
            (libc::SYS_readlinkat, vec![]),
            (libc::SYS_lseek, vec![]),
            (libc::SYS_pread64, vec![]),
            (libc::SYS_prlimit64, vec![]),
            (libc::SYS_statfs, vec![]),
            (libc::SYS_sched_getaffinity, vec![]),
            (libc::SYS_arch_prctl, vec![]),
            (libc::SYS_clone, vec![]),
            (libc::SYS_flock, vec![]),
            (libc::SYS_chdir, vec![]),
            (libc::SYS_set_tid_address, vec![]),
            (libc::SYS_fork, vec![]),
            (libc::SYS_vfork, vec![]),
            (libc::SYS_sigaltstack, vec![]),
            (libc::SYS_execve, vec![]),
            (libc::SYS_execveat, vec![]),
            (libc::SYS_rt_sigaction, vec![]),
            (libc::SYS_wait4, vec![]),
            (libc::SYS_exit_group, vec![]),
            (libc::SYS_exit, vec![]),
        ]);
        filter.apply()?;
        Ok(())
    }

    /// TODO: Apply relaxed seccomp rules,only blocks significant sensitive calls.
    /// See https://docs.docker.com/engine/security/seccomp/
    /// And https://raw.githubusercontent.com/moby/moby/master/profiles/seccomp/default.json
    pub fn apply_rule_relaxed() -> Result<()> {
        Self::apply_rule_compile()
    }

    pub fn apply_rule_unlimited() -> Result<()> {
        //No rules applied,but works as if we have applied rules!
        Ok(())
    }

    pub fn apply_rule(t: &SeccompType) -> Result<()> {
        match t {
            SeccompType::Compile => Self::apply_rule_compile(),
            SeccompType::Strict => Self::apply_rule_strict(),
            SeccompType::Relaxed => Self::apply_rule_relaxed(),
            SeccompType::Unlimited => Self::apply_rule_unlimited(),
        }
    }
}

pub struct SeccompBuilder {}

impl SeccompBuilder {
    pub fn new() -> Self {
        Self {}
    }
}
