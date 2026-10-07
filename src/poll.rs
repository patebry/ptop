//! Own asynchronous sensor children through cancellation and reap on shutdown.
use std::io::{self, Read};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::JoinHandle;
use std::time::Duration;

#[derive(Default)]
pub(crate) struct PollJobs {
    closed: AtomicBool,
    workers: Mutex<Vec<JoinHandle<()>>>,
    children: Mutex<Vec<Arc<Mutex<OwnedChild>>>>,
}
/// This is the only code that reaps these children. Keep status ownership under
/// the same lock used for group cancellation, including an exited group leader.
struct OwnedChild {
    child: Child,
    reaped: bool,
}
impl OwnedChild {
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        let status = self.child.try_wait()?;
        self.reaped |= status.is_some();
        Ok(status)
    }
    fn cancel_and_reap(&mut self) {
        if !self.reaped {
            // Each command created its own group (PGID == owned child PID).
            // An unreaped leader's PID cannot be reused, even after it exits.
            unsafe {
                libc::kill(-(self.child.id() as libc::pid_t), libc::SIGKILL);
            }
        }
        if self.child.wait().is_ok() {
            self.reaped = true;
        }
    }
}

impl PollJobs {
    pub(crate) fn spawn(&self, task: impl FnOnce() + Send + 'static) {
        let mut workers = self.workers.lock().unwrap();
        if self.closed.load(Ordering::Acquire) {
            return;
        }
        // Finished handles are reaped on the next dispatch, not accumulated per poll.
        let mut index = 0;
        while index < workers.len() {
            if workers[index].is_finished() {
                let _ = workers.swap_remove(index).join();
            } else {
                index += 1;
            }
        }
        workers.push(std::thread::spawn(task));
    }

    pub(crate) fn output(&self, command: &mut Command) -> io::Result<Output> {
        let (child, mut stdout, mut stderr) = {
            let mut children = self.children.lock().unwrap();
            if self.closed.load(Ordering::Acquire) {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "sensor stopped"));
            }
            let mut process = command
                .process_group(0)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            let stdout = process.stdout.take().expect("piped sensor stdout");
            let stderr = process.stderr.take().expect("piped sensor stderr");
            let child = Arc::new(Mutex::new(OwnedChild {
                child: process,
                reaped: false,
            }));
            children.push(Arc::clone(&child));
            (child, stdout, stderr)
        };
        let errors = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).map(|_| bytes)
        });
        let mut bytes = Vec::new();
        let read = stdout.read_to_end(&mut bytes);
        // Keep the leader unreaped until both pipes close: a wrapper may have
        // exited while a descendant still holds only stderr open.
        let stderr = errors
            .join()
            .map_err(|_| io::Error::other("sensor stderr reader failed"));
        // Reading stdout never holds the child mutex, so shutdown can kill it.
        // try_wait caches reaped status inside Child, preventing PID reuse races.
        let waited = loop {
            let status = { child.lock().unwrap().try_wait() };
            match status {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => std::thread::sleep(Duration::from_millis(1)),
                Err(error) => break Err(error),
            }
        };
        self.children
            .lock()
            .unwrap()
            .retain(|entry| !Arc::ptr_eq(entry, &child));
        read?;
        Ok(Output {
            status: waited?,
            stdout: bytes,
            stderr: stderr??,
        })
    }

    pub(crate) fn shutdown(&self) {
        self.closed.store(true, Ordering::Release);
        let children = self.children.lock().unwrap().clone();
        for child in children {
            let mut child = child.lock().unwrap();
            child.cancel_and_reap();
        }
        // Every pre-shutdown dispatch is registered under the same mutex that
        // checks closed. Late jobs cannot spawn an unregistered child.
        let workers = std::mem::take(&mut *self.workers.lock().unwrap());
        for worker in workers {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_kills_and_reaps_only_the_owned_pending_child() {
        for script in [
            r#": > "$PTOP_TEST_READY"; exec sleep 30"#,
            r#"sleep 30 & child=$!; : > "$PTOP_TEST_READY"; wait "$child""#,
            r#"sleep 30 & : > "$PTOP_TEST_READY"; exit 0"#,
            r#"sleep 30 >/dev/null & : > "$PTOP_TEST_READY"; exit 0"#,
        ] {
            let marker = std::env::temp_dir().join(format!(
                "ptop-child-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let worker_marker = marker.clone();
            let jobs = Arc::new(PollJobs::default());
            let worker_jobs = Arc::clone(&jobs);
            jobs.spawn(move || {
                let _ = worker_jobs.output(
                    Command::new("sh")
                        .args(["-c", script])
                        .env("PTOP_TEST_READY", worker_marker),
                );
            });
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            let child = loop {
                if let Some(child) = jobs.children.lock().unwrap().first().cloned() {
                    break child;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "owned child never started"
                );
                std::thread::sleep(Duration::from_millis(1));
            };
            while !marker.exists() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "wrapper never spawned child"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
            let start = std::time::Instant::now();
            jobs.shutdown();
            std::fs::remove_file(marker).unwrap();
            assert!(start.elapsed() < Duration::from_secs(1));
            assert!(child.lock().unwrap().try_wait().unwrap().is_some());
            assert!(jobs.children.lock().unwrap().is_empty());
            assert!(jobs.workers.lock().unwrap().is_empty());
        }
    }

    #[test]
    fn shutdown_prevents_late_spawn_even_if_a_poll_was_already_dispatched() {
        let jobs = PollJobs::default();
        jobs.shutdown();
        let error = jobs
            .output(&mut Command::new("must-not-be-executed"))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        jobs.spawn(|| panic!("post-shutdown dispatch ran"));
        assert!(jobs.workers.lock().unwrap().is_empty());
    }
}
