use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use lengua_host::{Input, Outcome};
use lengua_idea::Build;
use lengua_idea::lock::{self, Kind};

use crate::worker::{self, Request};

const WAIT: Duration = Duration::from_secs(30);
pub(crate) const GRACE: Duration = Duration::from_secs(2);

type Slot = Arc<(Mutex<Option<Outcome>>, Condvar)>;

#[derive(Clone)]
struct Job {
    key: String,
    started: Instant,
    slot: Slot,
}

pub(crate) enum Progress {
    Running(Duration),
    Ended(Outcome, Duration),
}

#[derive(Clone, Default)]
pub(crate) struct Runs {
    jobs: Arc<Mutex<HashMap<PathBuf, Job>>>,
}

fn lock_of<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn key(folder: &Path, build: &Build, input: &Input) -> String {
    let place = folder.to_string_lossy().to_string();
    let mode: &[u8] = if input.fixed { b"fixed" } else { b"live" };
    let mut parts: Vec<&[u8]> = vec![place.as_bytes(), build.inputs.as_bytes(), mode];
    parts.extend(input.arguments.iter().map(String::as_bytes));
    parts.push(&input.stdin);
    let mut all = Vec::new();
    for part in parts {
        all.extend_from_slice(part.len().to_string().as_bytes());
        all.push(0);
        all.extend_from_slice(part);
    }
    lengua_idea::sha256_hex(&all)
}

impl Job {
    fn start(folder: &Path, build: &Build, input: Input, key: String) -> Self {
        let job = Self {
            key,
            started: Instant::now(),
            slot: Arc::new((Mutex::new(None), Condvar::new())),
        };
        let slot = Arc::clone(&job.slot);
        let folder = folder.to_path_buf();
        let limit = Duration::from_millis(build.grants.time_ms) + GRACE;
        thread::spawn(move || {
            let lock_path = folder.join(lock::LOCK);
            let outcome = worker::run(&Request {
                kind: Kind::Program,
                lock: &lock_path,
                folder: &folder,
                input: &input,
                limit,
            });
            let (result, done) = &*slot;
            *lock_of(result) = Some(outcome);
            done.notify_all();
        });
        job
    }

    fn wait(&self, longest: Duration) -> Option<Outcome> {
        let (result, done) = &*self.slot;
        let guard = lock_of(result);
        let (guard, _) = done
            .wait_timeout_while(guard, longest, |outcome| outcome.is_none())
            .unwrap_or_else(PoisonError::into_inner);
        guard.clone()
    }
}

impl Runs {
    pub(crate) fn run(&self, folder: &Path, build: &Build, input: Input) -> Progress {
        let key = key(folder, build, &input);
        let job = {
            let mut jobs = lock_of(&self.jobs);
            match jobs.get(folder) {
                Some(job) if job.key == key => job.clone(),
                _ => {
                    let job = Job::start(folder, build, input, key);
                    jobs.insert(folder.to_path_buf(), job.clone());
                    job
                }
            }
        };
        let outcome = job.wait(WAIT);
        let taken = job.started.elapsed();
        match outcome {
            Some(outcome) => {
                let mut jobs = lock_of(&self.jobs);
                if jobs.get(folder).is_some_and(|j| j.key == job.key) {
                    jobs.remove(folder);
                }
                Progress::Ended(outcome, taken)
            }
            None => Progress::Running(taken),
        }
    }
}
