use bitter_core::session::{self, Session};
use std::io;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const SAVE_DELAY: Duration = Duration::from_millis(250);

pub struct SessionWriter {
    requests: Option<Sender<Session>>,
    errors: Receiver<io::Result<()>>,
    worker: Option<JoinHandle<()>>,
}

impl SessionWriter {
    pub fn new(path: PathBuf) -> io::Result<Self> {
        let (requests, pending) = mpsc::channel::<Session>();
        let (status, errors) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("bitter-session".into())
            .spawn(move || {
                let mut previous = None;
                while let Ok(mut latest) = pending.recv() {
                    let until = Instant::now() + SAVE_DELAY;
                    while let Some(remaining) = until.checked_duration_since(Instant::now()) {
                        match pending.recv_timeout(remaining) {
                            Ok(next) => latest = next,
                            Err(_) => break,
                        }
                    }
                    // Save the newest snapshot even when the application is shutting down.
                    if previous.as_ref() != Some(&latest) {
                        let outcome = session::save(&path, latest.clone());
                        if outcome.is_ok() {
                            previous = Some(latest);
                        }
                        let _ = status.send(outcome);
                    }
                }
            })?;
        Ok(Self {
            requests: Some(requests),
            errors,
            worker: Some(worker),
        })
    }

    pub fn queue(&self, snapshot: Session) {
        if let Some(requests) = &self.requests {
            let _ = requests.send(snapshot);
        }
    }

    pub fn newest_result(&self) -> Option<io::Result<()>> {
        self.errors.try_iter().last()
    }

    pub fn finish(&mut self) {
        self.requests.take();
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            tracing::error!("Session writer stopped unexpectedly");
        }
        if let Some(Err(error)) = self.newest_result() {
            tracing::warn!("Could not save session: {error}");
        }
    }
}

impl Drop for SessionWriter {
    fn drop(&mut self) {
        self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitter_core::session::{SavedTab, WorkspaceId};

    #[test]
    fn shutdown_flushes_the_latest_order_and_selection() {
        let profile = tempfile::tempdir().unwrap();
        let path = profile.path().join("session.json");
        let writer = SessionWriter::new(path.clone()).unwrap();
        let mut snapshot = Session::default();
        writer.queue(snapshot.clone());
        snapshot.workspace = WorkspaceId::Second;
        snapshot.workspaces[1]
            .tabs
            .push(SavedTab::new("https://example.test/", "Example"));
        writer.queue(snapshot.clone());
        drop(writer);
        assert_eq!(session::load(&path).unwrap(), snapshot);
    }

    #[test]
    fn finish_flushes_queued_snapshots_and_stays_idle() {
        let profile = tempfile::tempdir().unwrap();
        let path = profile.path().join("session.json");
        let mut writer = SessionWriter::new(path.clone()).unwrap();
        writer.queue(Session::default());
        writer.finish();
        // Queuing after the worker is gone is a no-op; the file keeps the
        // snapshot that was flushed when the worker stopped.
        writer.queue(Session::default());
        assert_eq!(session::load(&path).unwrap(), Session::default());
    }

    #[test]
    fn newest_result_is_empty_before_any_save_completes() {
        let profile = tempfile::tempdir().unwrap();
        let path = profile.path().join("session.json");
        let writer = SessionWriter::new(path).unwrap();
        assert!(writer.newest_result().is_none());
        drop(writer);
    }
}
