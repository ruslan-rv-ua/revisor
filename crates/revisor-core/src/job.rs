//! Background jobs (ADR-0011): the long actions of the core — a snapshot, a compaction
//! and a comparison — run on threads of their own, so that whoever started
//! one is free meanwhile. A [`Work`] is one kind of job: what it reports while it runs,
//! what it ends with, and the action itself. A [`Job`] runs a work and answers whoever
//! waits for it, and one [`Cancel`] token stops every long action of the core.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rusqlite::InterruptHandle;

/// How long [`Job::poll`] waits for the result of a work that is a moment from its end:
/// a snapshot whose save is over only closes the database and reports, a matter of
/// milliseconds. Whoever waited for that save then reports the job's end together with
/// its own outcome instead of on the next poll (TASK-21).
pub const GRACE: Duration = Duration::from_secs(2);

/// Asks the long actions of the core to stop: scanning, saving and loading a snapshot,
/// comparing trees and snapshots, taking a snapshot and compacting a database. They look
/// at the token now and then and end as cancelled; the VACUUM of a compaction, which
/// never looks, is interrupted instead. Once cancelled, a token stays cancelled.
#[derive(Default)]
pub struct Cancel {
    cancelled: AtomicBool,
    /// The SQLite statement running under the token, which [`Cancel::cancel`] interrupts.
    statement: Mutex<Option<InterruptHandle>>,
}

impl Cancel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Asks the actions under the token to stop.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
        if let Some(statement) = &*self.statement.lock().unwrap() {
            statement.interrupt();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Runs `statement`, a long SQLite statement on the connection of `handle`, so that
    /// [`Cancel::cancel`] interrupts it; on a token cancelled already the statement does
    /// not run, and the result is `None`. A cancel that comes just as the statement
    /// starts may miss it, and the statement runs to its end, as SQLite lets a statement
    /// that is nearly done finish.
    pub(crate) fn interrupting<T>(
        &self,
        handle: InterruptHandle,
        statement: impl FnOnce() -> T,
    ) -> Option<T> {
        *self.statement.lock().unwrap() = Some(handle);
        // Looked at once the handle is in place: a cancel before that is seen here, and
        // one after it interrupts the statement.
        let result = (!self.is_cancelled()).then(statement);
        *self.statement.lock().unwrap() = None;
        result
    }
}

impl fmt::Debug for Cancel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cancel")
            .field("cancelled", &self.is_cancelled())
            .finish_non_exhaustive()
    }
}

/// How a work ended.
#[derive(Debug)]
pub enum Outcome<T, E> {
    /// It did what it was for.
    Done(T),
    /// It could not.
    Failed(E),
    /// It was cancelled before it was done.
    Cancelled,
}

impl<T, E> Outcome<T, E> {
    /// The outcome of an action that ended with `result`; `is_cancellation` tells the
    /// errors that are cancellations from the failures.
    pub fn of(result: Result<T, E>, is_cancellation: impl FnOnce(&E) -> bool) -> Self {
        match result {
            Ok(value) => Outcome::Done(value),
            Err(e) if is_cancellation(&e) => Outcome::Cancelled,
            Err(e) => Outcome::Failed(e),
        }
    }
}

/// One kind of background job: what it reports while it runs, what it ends with, and the
/// action itself. A snapshot ([`crate::snapshot::SnapshotWork`]), a compaction
/// ([`crate::store::Compaction`]) and a comparison ([`crate::compare::CompareWork`]) are
/// works; another kind is another implementation, and this module stays as it is.
pub trait Work: Send + 'static {
    /// What the work reports while it runs; whoever waits for it reads it now and then,
    /// from another thread.
    type Progress: Default + Send + Sync + 'static;
    /// What the work ends with, usually an [`Outcome`].
    type Output: Send + 'static;

    /// Does the work on the job's thread, reporting to `progress`; it stops early once
    /// `cancel` is cancelled.
    fn run(self, progress: &Self::Progress, cancel: &Cancel) -> Self::Output;

    /// Whether `progress` says the work is a moment from its end, so that its result is
    /// worth waiting for (TASK-21): [`Job::poll`] then waits for it up to [`GRACE`]. A
    /// work that says nothing is never waited for.
    fn about_to_end(_progress: &Self::Progress) -> bool {
        false
    }
}

/// A work running on a thread of its own. A job dropped without [`Job::wait`] leaves its
/// thread to end by itself: nothing waits for it, so closing revisor does not either.
pub struct Job<W: Work> {
    progress: Arc<W::Progress>,
    cancel: Arc<Cancel>,
    result: Receiver<W::Output>,
    thread: JoinHandle<()>,
    started: Instant,
}

/// The thread of a job ended without its result, as only a panic ends it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lost;

impl fmt::Display for Lost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the job ended without its result")
    }
}

impl std::error::Error for Lost {}

impl<W: Work> Job<W> {
    /// Starts `work` on a thread of its own.
    pub fn start(work: W) -> Self {
        let progress = Arc::new(W::Progress::default());
        let cancel = Arc::new(Cancel::new());
        let (sender, result) = mpsc::channel();
        let thread = {
            let (progress, cancel) = (Arc::clone(&progress), Arc::clone(&cancel));
            thread::spawn(move || {
                // Nobody may wait for the result any more, which is no error.
                let _ = sender.send(work.run(&progress, &cancel));
            })
        };
        Self {
            progress,
            cancel,
            result,
            thread,
            started: Instant::now(),
        }
    }

    /// What the work has reported so far.
    pub fn progress(&self) -> &W::Progress {
        &self.progress
    }

    /// How long the job has been running.
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// Asks the work to stop. It soon ends as cancelled, unless it is done by then, and
    /// its end comes through [`Job::poll`] or [`Job::wait`] like any other.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    /// The result of the work once it is over, or `None` while it runs. A work that is a
    /// moment from its end ([`Work::about_to_end`]) is waited for up to [`GRACE`] rather
    /// than left to the next poll (TASK-21). Once the job has given its result, it is
    /// over.
    pub fn poll(&mut self) -> Option<Result<W::Output, Lost>> {
        match self.result.try_recv() {
            Ok(output) => Some(Ok(output)),
            Err(TryRecvError::Disconnected) => Some(Err(Lost)),
            Err(TryRecvError::Empty) if W::about_to_end(&self.progress) => {
                match self.result.recv_timeout(GRACE) {
                    Ok(output) => Some(Ok(output)),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => Some(Err(Lost)),
                }
            }
            Err(TryRecvError::Empty) => None,
        }
    }

    /// Waits for the work to end and gives its result.
    pub fn wait(self) -> Result<W::Output, Lost> {
        let output = self.result.recv().map_err(|_| Lost);
        let _ = self.thread.join();
        output
    }
}

#[cfg(test)]
mod tests;
