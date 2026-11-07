use super::*;
use crate::testing::{cancelled, poll_until_ended};
use std::sync::mpsc::Sender;

/// A work that runs until the test lets it end, and ends with what it was given.
struct Gated {
    gate: Receiver<()>,
    output: u32,
}

impl Work for Gated {
    type Progress = ();
    type Output = u32;

    fn run(self, _: &(), _: &Cancel) -> u32 {
        let _ = self.gate.recv();
        self.output
    }
}

/// A job of [`Gated`], and what opens its gate.
fn gated(output: u32) -> (Job<Gated>, Sender<()>) {
    let (open, gate) = mpsc::channel();
    (Job::start(Gated { gate, output }), open)
}

/// Opens a gate after `delay`, from another thread.
fn open_after(open: Sender<()>, delay: Duration) {
    thread::spawn(move || {
        thread::sleep(delay);
        let _ = open.send(());
    });
}

/// A work that says it is a moment from its end, then ends once the test lets it.
struct Ending {
    gate: Receiver<()>,
}

#[derive(Default)]
struct EndingProgress {
    ending: AtomicBool,
}

impl Work for Ending {
    type Progress = EndingProgress;
    type Output = &'static str;

    fn run(self, progress: &EndingProgress, _: &Cancel) -> &'static str {
        progress.ending.store(true, Ordering::Relaxed);
        let _ = self.gate.recv();
        "done"
    }

    fn about_to_end(progress: &EndingProgress) -> bool {
        progress.ending.load(Ordering::Relaxed)
    }
}

/// A job of [`Ending`] that has said it is about to end, and what lets it end.
fn ending() -> (Job<Ending>, Sender<()>) {
    let (open, gate) = mpsc::channel();
    let job = Job::start(Ending { gate });
    let deadline = Instant::now() + Duration::from_secs(10);
    while !job.progress().ending.load(Ordering::Relaxed) {
        assert!(
            Instant::now() < deadline,
            "the work never said it was ending"
        );
        thread::sleep(Duration::from_millis(1));
    }
    (job, open)
}

/// A work that goes on until it is cancelled.
struct UntilCancelled;

impl Work for UntilCancelled {
    type Progress = ();
    type Output = Outcome<(), ()>;

    fn run(self, _: &(), cancel: &Cancel) -> Self::Output {
        while !cancel.is_cancelled() {
            thread::sleep(Duration::from_millis(1));
        }
        Outcome::Cancelled
    }
}

/// A work whose thread panics.
struct Panicking;

impl Work for Panicking {
    type Progress = ();
    type Output = ();

    fn run(self, _: &(), _: &Cancel) {
        panic!("a work that panics, on purpose");
    }
}

#[test]
fn a_job_runs_on_its_own_thread_and_gives_its_result_once_it_is_over() {
    let (mut job, open) = gated(7);
    assert!(job.poll().is_none(), "the work waits for the test");
    open.send(()).unwrap();
    assert_eq!(poll_until_ended(&mut job), Ok(7));
}

#[test]
fn a_work_that_says_nothing_is_not_waited_for() {
    let (mut job, open) = gated(1);
    let polling = Instant::now();
    assert!(job.poll().is_none());
    assert!(polling.elapsed() < GRACE / 2, "{:?}", polling.elapsed());
    open.send(()).unwrap();
    assert_eq!(job.wait(), Ok(1));
}

#[test]
fn a_work_about_to_end_is_waited_for_by_the_poll_that_finds_it_so() {
    let (mut job, open) = ending();
    let delay = Duration::from_millis(300);
    open_after(open, delay);
    let polling = Instant::now();
    // The result is not there yet, yet this poll gives it: it waits (TASK-21).
    assert_eq!(job.poll(), Some(Ok("done")));
    let waited = polling.elapsed();
    assert!(waited >= delay / 2 && waited < GRACE, "{waited:?}");
}

#[test]
fn the_wait_for_a_work_about_to_end_is_short() {
    let (mut job, open) = ending();
    let polling = Instant::now();
    assert_eq!(job.poll(), None);
    let waited = polling.elapsed();
    assert!(waited >= GRACE / 2 && waited < GRACE * 2, "{waited:?}");
    open.send(()).unwrap();
    assert_eq!(job.wait(), Ok("done"));
}

#[test]
fn wait_waits_for_the_work_to_end() {
    let (job, open) = gated(3);
    let delay = Duration::from_millis(100);
    open_after(open, delay);
    let waiting = Instant::now();
    assert_eq!(job.wait(), Ok(3));
    assert!(waiting.elapsed() >= delay / 2);
}

#[test]
fn a_job_dropped_without_waiting_leaves_its_thread_to_end_by_itself() {
    let (job, open) = gated(0);
    open_after(open, Duration::from_secs(2));
    let dropping = Instant::now();
    drop(job);
    assert!(dropping.elapsed() < Duration::from_secs(1));
}

#[test]
fn the_elapsed_time_counts_from_the_start() {
    let (job, open) = gated(0);
    let delay = Duration::from_millis(50);
    thread::sleep(delay);
    assert!(job.elapsed() >= delay);
    open.send(()).unwrap();
    job.wait().unwrap();
}

#[test]
fn a_cancelled_job_ends_as_cancelled() {
    let job = Job::start(UntilCancelled);
    job.cancel();
    assert!(matches!(job.wait(), Ok(Outcome::Cancelled)));
}

#[test]
fn a_job_whose_thread_panicked_is_lost() {
    let mut job = Job::start(Panicking);
    assert_eq!(poll_until_ended(&mut job), Err(Lost));
    assert_eq!(Job::start(Panicking).wait(), Err(Lost));
}

#[test]
fn an_outcome_tells_cancellations_from_failures() {
    let of = |result: Result<u8, &'static str>| Outcome::of(result, |e| *e == "cancelled");
    assert!(matches!(of(Ok(1)), Outcome::Done(1)));
    assert!(matches!(of(Err("cancelled")), Outcome::Cancelled));
    assert!(matches!(of(Err("disk full")), Outcome::Failed("disk full")));
}

#[test]
fn a_token_cancelled_already_does_not_run_its_statement() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    assert_eq!(
        cancelled().interrupting(conn.get_interrupt_handle(), || 1),
        None
    );
    let cancel = Cancel::new();
    assert_eq!(
        cancel.interrupting(conn.get_interrupt_handle(), || 1),
        Some(1)
    );
    // Once the statement is over, cancelling interrupts nothing of it.
    cancel.cancel();
    assert!(cancel.statement.lock().unwrap().is_none());
}

#[test]
fn cancelling_interrupts_the_statement_running_under_the_token() {
    // Counting to ten billion takes hours: only an interrupt ends it soon.
    const COUNT: &str = "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n
                         LIMIT 10000000000) SELECT count(*) FROM n";
    let cancel = Arc::new(Cancel::new());
    let (started, starting) = mpsc::channel();
    let counting = {
        let cancel = Arc::clone(&cancel);
        thread::spawn(move || {
            let conn = rusqlite::Connection::open_in_memory().unwrap();
            cancel.interrupting(conn.get_interrupt_handle(), || {
                started.send(()).unwrap();
                conn.query_row(COUNT, [], |r| r.get::<_, i64>(0))
            })
        })
    };
    starting.recv().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    // Cancelled until the statement answers: one cancel may come just before it runs.
    while !counting.is_finished() {
        assert!(
            Instant::now() < deadline,
            "the statement was not interrupted"
        );
        cancel.cancel();
        thread::sleep(Duration::from_millis(10));
    }
    let counted = counting.join().unwrap();
    assert!(
        matches!(
            &counted,
            Some(Err(e)) if e.sqlite_error_code() == Some(rusqlite::ErrorCode::OperationInterrupted)
        ),
        "{counted:?}"
    );
}

#[test]
fn a_poll_within_a_time_waits_for_the_result_up_to_that_time() {
    let (mut job, open) = gated(9);
    assert!(job.poll_within(Duration::from_millis(50)).is_none());
    open_after(open, Duration::from_millis(50));
    assert_eq!(job.poll_within(Duration::from_secs(60)), Some(Ok(9)));
}
