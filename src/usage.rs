use libc;
use std::mem;

#[cfg(target_family = "unix")]
#[derive(Debug)]
pub struct Rusage(libc::rusage);

#[cfg(target_family = "unix")]
impl Rusage {
    pub fn get_rusage_for_child() -> Option<Self> {
        let mut data: libc::rusage = unsafe { mem::MaybeUninit::uninit().assume_init() };
        let p = &mut data as *mut libc::rusage;
        unsafe {
            let error = libc::getrusage(libc::RUSAGE_CHILDREN, p);
            if error < 0 {
                return None;
            }
        }
        let ret = Rusage { 0: data };
        Some(ret)
    }

    pub fn get_run_time(&self) -> i64 {
        //In microseconds
        self.0.ru_utime.tv_sec * 1000 + self.0.ru_utime.tv_usec / 1000
    }

    pub fn get_memory(&self) -> i64 {
        self.0.ru_maxrss
    }
}

impl From<libc::rusage> for Rusage {
    fn from(a: libc::rusage) -> Self {
        Rusage { 0: a }
    }
}
