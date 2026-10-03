use std::cell::RefCell;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use nix::{
    errno::Errno,
    sys::{
        prctl::set_child_subreaper,
        signal::{Signal, kill},
        wait::{WaitPidFlag, WaitStatus, waitpid},
    },
    unistd::{Pid, getpid, gettid},
};

const CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

#[cfg(all(test, target_os = "linux"))]
#[path = "child_reaper/regression.rs"]
pub(crate) mod regression;

#[derive(Default)]
struct RunnerState {
    active: bool,
    failed: bool,
}

thread_local! {
    // Only the standalone runner opts in. Parallel libtest workers must never reap each other's
    // children. A thread-local capability and a !Send case guard keep reaping on the runner thread.
    static RUNNER: RefCell<Option<RunnerState>> = const { RefCell::new(None) };
}

pub(crate) struct ChildReaper {
    children_file: PathBuf,
    timeout: Duration,
    finished: bool,
    _thread: PhantomData<Rc<()>>,
}

impl ChildReaper {
    pub(crate) fn enable() -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            if getpid() != gettid() {
                return Err("E2E child supervision requires the runner's main thread".into());
            }
            set_child_subreaper(true).map_err(|error| {
                format!(
                    "failed to supervise E2E descendants: {}",
                    std::io::Error::from(error)
                )
            })?;
            RUNNER.with_borrow_mut(|state| {
                state.get_or_insert_with(RunnerState::default);
            });
            Ok(())
        }
        #[cfg(not(target_os = "linux"))]
        Err("real-server E2E child supervision requires Linux".into())
    }

    pub(crate) fn begin_case() -> Result<Self, String> {
        RUNNER.with_borrow_mut(|state| {
            let state = state
                .as_mut()
                .ok_or("E2E child supervision is not enabled")?;
            if state.failed {
                return Err(
                    "cannot start another E2E case after incomplete child cleanup".to_string(),
                );
            }
            if state.active {
                return Err("E2E child supervision requires sequential cases".to_string());
            }
            state.active = true;
            Ok(())
        })?;
        let reaper = Self {
            children_file: PathBuf::from(format!(
                "/proc/self/task/{}/children",
                std::process::id()
            )),
            timeout: CLEANUP_TIMEOUT,
            finished: false,
            _thread: PhantomData,
        };
        if !reaper.reap_exited()? {
            return Err("cannot start an E2E case while earlier child processes remain".into());
        }
        Ok(reaper)
    }

    pub(crate) fn reap_exited(&self) -> Result<bool, String> {
        self.reap_until(Instant::now() + self.timeout)
    }

    #[cfg(target_os = "linux")]
    fn reap_until(&self, deadline: Instant) -> Result<bool, String> {
        loop {
            Self::check_deadline(deadline)?;
            match waitpid(None, Some(WaitPidFlag::WNOHANG)) {
                Err(Errno::ECHILD) => return Ok(true),
                Err(Errno::EINTR) => continue,
                Ok(WaitStatus::StillAlive) => return Ok(false),
                Ok(_) => {}
                Err(error) => {
                    return Err(format!(
                        "failed to reap E2E children: {}",
                        std::io::Error::from(error)
                    ));
                }
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn reap_until(&self, _deadline: Instant) -> Result<bool, String> {
        Err("real-server E2E child supervision requires Linux".into())
    }

    pub(crate) fn finish(&mut self) -> Result<(), String> {
        let result = self.drain();
        self.finished = result.is_ok();
        result
    }

    fn drain(&self) -> Result<(), String> {
        let deadline = Instant::now() + self.timeout;
        while !self.reap_until(deadline)? {
            self.kill_children(deadline)?;
            std::thread::sleep(
                POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
            );
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn kill_children(&self, deadline: Instant) -> Result<(), String> {
        let children = std::fs::read_to_string(&self.children_file)
            .map_err(|error| format!("failed to enumerate E2E children: {error}"))?;
        // No other waiter may run here, and we must not reap between enumeration and signalling:
        // even a child that exits retains its PID until reaped, preventing signals to reused PIDs.
        // /proc can omit children during exits. Only waitpid's ECHILD proves cleanup is complete.
        for child in children.split_whitespace() {
            Self::check_deadline(deadline)?;
            let pid = child
                .parse::<i32>()
                .ok()
                .filter(|pid| *pid > 0)
                .ok_or_else(|| format!("invalid child PID in E2E process list: {child:?}"))?;
            match kill(Pid::from_raw(pid), Signal::SIGKILL) {
                Ok(()) | Err(Errno::ESRCH) => {}
                Err(error) => {
                    return Err(format!(
                        "failed to terminate E2E child {pid}: {}",
                        std::io::Error::from(error)
                    ));
                }
            }
        }
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    fn kill_children(&self, _deadline: Instant) -> Result<(), String> {
        Err("real-server E2E child supervision requires Linux".into())
    }

    fn check_deadline(deadline: Instant) -> Result<(), String> {
        if Instant::now() >= deadline {
            Err("E2E child cleanup exceeded its deadline; descendants may still be running".into())
        } else {
            Ok(())
        }
    }
}

impl Drop for ChildReaper {
    fn drop(&mut self) {
        // A failed or abandoned drain contaminates this runner. Never let a later case clean up
        // processes belonging to an earlier case, including when setup or unwinding failed.
        RUNNER.with_borrow_mut(|state| {
            if let Some(state) = state {
                state.active = false;
                state.failed |= !self.finished;
            }
        });
    }
}
