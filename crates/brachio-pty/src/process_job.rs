//! Keeps everything a terminal program starts in one Windows Job Object, so stopping the
//! terminal ends the whole tree instead of only the top process.

use std::io;

use win32job::{ExtendedLimitInfo, Job};

/// A job that kills every process in it when it is closed or dropped.
pub struct ProcessTree {
    job: Job,
}

impl ProcessTree {
    /// Puts the process behind `handle` into a new job.
    ///
    /// Processes it started before this call stay outside the job.
    ///
    /// # Errors
    /// Fails when the job cannot be created or the process cannot join it.
    pub fn adopt(handle: isize) -> io::Result<Self> {
        let mut limits = ExtendedLimitInfo::new();
        limits.limit_kill_on_job_close();
        let job = Job::create_with_limit_info(&limits).map_err(io::Error::other)?;
        job.assign_process(handle).map_err(io::Error::other)?;
        Ok(Self { job })
    }

    /// Ends every process in the job.
    pub fn kill(self) {
        drop(self.job);
    }
}
