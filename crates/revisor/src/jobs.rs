//! The background jobs of the window (ADR-0011): one place for each kind, which holds
//! the job with the profile it is of, and what the jobs mean for the window — which one
//! the progress row shows, the flags of the state the commands decide by, and the
//! profile whose database a compaction has closed. The window starts, cancels and polls
//! the jobs here and holds none of their parts itself.
//!
//! «Знімок + порівняння» (TASK-12) is two jobs in a row, which the window chains: the
//! place of the snapshot holds the comparison the snapshot is to have, and the place of
//! the comparison the snapshot's report, which waits for the difference. The command is
//! a comparison from its start, so the rule «the later comparison is the one wanted»
//! (TASK-11 #15) counts it from then.
//!
//! A comment (TASK-24) is a job only while the database is busy: it is tried at once,
//! and a database another connection holds is waited for on the job's thread by the one
//! rule of the store (ADR-0012), which the window shows and can cancel.

use std::path::Path;
use std::time::Duration;

use revisor_core::compare::{self, CompareError, CompareWork, Difference};
use revisor_core::job::{self, Job, Lost, Outcome};
use revisor_core::profiles::ProfileFile;
use revisor_core::snapshot::{self, Report, SnapshotError, SnapshotWork};
use revisor_core::store::{
    self, CommentWork, Compacted, Compaction, SnapshotInfo, Store, StoreError,
};

/// How long a comparison runs before the progress row shows it (TASK-11 #7): a shorter
/// one shows no progress at all.
pub const COMPARISON_SHOWN_AFTER: Duration = Duration::from_secs(1);

/// A kind of background job; at most one of each runs at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Snapshot,
    Comparison,
    Compaction,
    Comment,
}

/// What the progress row shows: the progress of the snapshot being taken, else a comment
/// waiting for the database, else the progress of a comparison that has run for a second
/// or more, else the profile whose database is being compacted.
#[derive(Clone, Copy, Debug)]
pub enum Shown<'a> {
    Snapshot(&'a snapshot::Progress),
    Comparison {
        progress: &'a compare::Progress,
        /// How long the comparison has run.
        elapsed: Duration,
    },
    Compaction(&'a ProfileFile),
    /// A comment waits for the database of its profile (TASK-24).
    Comment,
}

impl Shown<'_> {
    pub fn kind(&self) -> Kind {
        match self {
            Shown::Snapshot(_) => Kind::Snapshot,
            Shown::Comparison { .. } => Kind::Comparison,
            Shown::Compaction(_) => Kind::Compaction,
            Shown::Comment => Kind::Comment,
        }
    }
}

/// The flags of the state that come from the jobs, for the open profile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags {
    pub snapshot_running: bool,
    /// The snapshot being taken is of the open profile, which then cannot be changed or
    /// deleted (TASK-8).
    pub snapshotting_open_profile: bool,
    pub comparison_running: bool,
    pub compaction_running: bool,
    /// The database being compacted is the open profile's, which is then closed even to
    /// reading (TASK-9).
    pub compacting_open_profile: bool,
    /// A comment waits for the database of its profile; one goes at a time (TASK-24).
    pub comment_waiting: bool,
    /// The comment waiting is of the open profile, which then cannot be changed nor its
    /// snapshots deleted, as during a snapshot of it.
    pub commenting_open_profile: bool,
}

/// A job that has ended: the profile it was of, and how it ended; `Err(Lost)` when its
/// thread ended without saying.
#[derive(Debug)]
pub struct Ended<O> {
    pub profile: ProfileFile,
    pub outcome: Result<O, Lost>,
}

/// What «Знімок + порівняння» does once its snapshot is saved (TASK-12): compares it
/// with `with`, the snapshot that was the newest of the profile when the command started
/// (#1); `None` when the profile had none, so there is nothing to compare it with (#4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThenCompare {
    pub with: Option<i64>,
}

/// The report of the snapshot of a «Знімок + порівняння» whose comparison another
/// comparison has taken the place of (TASK-11 #15): it waited for a difference that will
/// not come, so whoever started the other comparison says it now (TASK-12 #8).
#[derive(Debug)]
pub struct Superseded {
    pub profile: ProfileFile,
    pub report: Report,
}

/// The comment of a snapshot: the snapshot `id` is to have `comment`, or none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comment {
    pub id: i64,
    pub comment: Option<String>,
}

/// A job that [`Jobs::poll`] found ended.
#[derive(Debug)]
pub enum Event {
    /// A snapshot ended; the snapshot of «Знімок + порівняння» with the comparison it is
    /// to have, unless another comparison was started meanwhile (TASK-12 #11).
    Snapshot(Ended<Outcome<Report, SnapshotError>>, Option<ThenCompare>),
    /// A comparison ended; that of «Знімок + порівняння» with the report of its snapshot,
    /// which waited for it (TASK-12 #8).
    Comparison(Ended<Outcome<Difference, CompareError>>, Option<Report>),
    Compaction(Ended<Outcome<Compacted, StoreError>>),
    /// A comment that waited for the database ended (TASK-24).
    Comment(Ended<Outcome<(), StoreError>>, Comment),
}

/// A snapshot being taken, the profile it is of, and, for the snapshot of «Знімок +
/// порівняння», the comparison it is to have once saved.
struct Snapshotting {
    profile: ProfileFile,
    job: Job<SnapshotWork>,
    then: Option<ThenCompare>,
}

/// A comparison being made, the profile its snapshots are of, and, for the comparison of
/// «Знімок + порівняння», the report of its snapshot, which waits for the difference.
struct Comparing {
    profile: ProfileFile,
    job: Job<CompareWork>,
    after: Option<Report>,
}

/// A database being compacted, with its profile and that profile's snapshots left after
/// the deletion: the database is closed even to reading meanwhile (TASK-9), so whoever
/// shows the profile takes them from here.
struct Compacting {
    profile: ProfileFile,
    snapshots: Vec<SnapshotInfo>,
    job: Job<Compaction>,
}

/// A comment waiting for the database of its profile.
struct Commenting {
    profile: ProfileFile,
    comment: Comment,
    job: Job<CommentWork>,
    /// Whether the window has said that the comment waits.
    announced: bool,
}

/// The background jobs, one place for each kind.
#[derive(Default)]
pub struct Jobs {
    snapshot: Option<Snapshotting>,
    comparison: Option<Comparing>,
    compaction: Option<Compacting>,
    comment: Option<Commenting>,
}

impl Jobs {
    /// Starts a snapshot of `profile`; the commands see to it that no other one runs.
    pub fn start_snapshot(&mut self, profile: ProfileFile) {
        self.start_snapshot_then(profile, None);
    }

    /// Starts «Знімок + порівняння» on `profile` (TASK-12): a snapshot that is to be
    /// compared with `with` once saved. The command is a comparison from its start, the
    /// latest one, so a comparison that still runs is cancelled and forgotten, as by
    /// [`Jobs::start_comparison`]; the report it held, if it was the comparison of an
    /// earlier «Знімок + порівняння», comes back to be said.
    #[must_use]
    pub fn start_snapshot_and_compare(
        &mut self,
        profile: ProfileFile,
        with: Option<i64>,
    ) -> Option<Superseded> {
        let superseded = self.forget_comparison();
        self.start_snapshot_then(profile, Some(ThenCompare { with }));
        superseded
    }

    fn start_snapshot_then(&mut self, profile: ProfileFile, then: Option<ThenCompare>) {
        debug_assert!(self.snapshot.is_none(), "one snapshot at a time");
        let job = Job::start(SnapshotWork::new(&profile.path, None));
        self.snapshot = Some(Snapshotting { profile, job, then });
    }

    /// Starts comparing the snapshots `a` and `b` of `profile`. A comparison that still
    /// runs is cancelled and forgotten: only one difference can be shown, and the later
    /// comparison is the one wanted (TASK-11 #15). So is the comparison the snapshot of a
    /// «Знімок + порівняння» being taken was to have: that snapshot ends as a regular one
    /// (TASK-12 #11). The report a forgotten comparison of «Знімок + порівняння» held
    /// comes back to be said.
    #[must_use]
    pub fn start_comparison(&mut self, profile: ProfileFile, a: i64, b: i64) -> Option<Superseded> {
        if let Some(snapshotting) = &mut self.snapshot {
            snapshotting.then = None;
        }
        let superseded = self.forget_comparison();
        self.compare(profile, a, b, None);
        superseded
    }

    /// Starts the comparison of «Знімок + порівняння» once its snapshot is saved
    /// (TASK-12 #8): of the snapshot of `report` with `with`. The report waits for the
    /// comparison's end, and the progress row shows the comparison at once, in place of
    /// the snapshot (#5, #9). The command holds the place of the comparison from its
    /// start, so none runs by then.
    pub fn start_comparison_after_snapshot(
        &mut self,
        profile: ProfileFile,
        with: i64,
        report: Report,
    ) {
        debug_assert!(
            self.comparison.is_none(),
            "«Знімок + порівняння» holds the place of the comparison from its start"
        );
        let _ = self.forget_comparison();
        let later = report.id;
        self.compare(profile, with, later, Some(report));
    }

    fn compare(&mut self, profile: ProfileFile, a: i64, b: i64, after: Option<Report>) {
        let job = Job::start(CompareWork::new(&profile.path, a, b));
        self.comparison = Some(Comparing {
            profile,
            job,
            after,
        });
    }

    /// Cancels and forgets the comparison that runs, if one does; gives back the report
    /// it held when it was the comparison of «Знімок + порівняння».
    fn forget_comparison(&mut self) -> Option<Superseded> {
        let Comparing {
            profile,
            job,
            after,
        } = self.comparison.take()?;
        job.cancel();
        after.map(|report| Superseded { profile, report })
    }

    /// Starts compacting the database of `profile` on `store`, the connection its
    /// snapshots were deleted on; `snapshots` are the ones left. The commands see to it
    /// that no other compaction runs.
    pub fn start_compaction(
        &mut self,
        profile: ProfileFile,
        snapshots: Vec<SnapshotInfo>,
        store: Store,
    ) {
        debug_assert!(self.compaction.is_none(), "one compaction at a time");
        let job = Job::start(Compaction::new(store));
        self.compaction = Some(Compacting {
            profile,
            snapshots,
            job,
        });
    }

    /// Writes `comment` in the database of `profile` (TASK-24). A database that is free
    /// takes it at once, so the comment is over as the command is, and gives the result;
    /// a busy one makes it a job, which waits for the database and ends through
    /// [`Jobs::poll`], and gives `None`. The commands see to it that no other comment
    /// waits.
    pub fn comment(
        &mut self,
        profile: ProfileFile,
        comment: Comment,
    ) -> Option<Result<(), StoreError>> {
        debug_assert!(self.comment.is_none(), "one comment at a time");
        match store::write_comment(&profile.path, comment.id, comment.comment.as_deref()) {
            Err(e) if e.is_busy() => {
                let work = CommentWork::new(&profile.path, comment.id, comment.comment.clone());
                self.comment = Some(Commenting {
                    profile,
                    comment,
                    job: Job::start(work),
                    announced: false,
                });
                None
            }
            written => Some(written),
        }
    }

    /// Whether a comment has started to wait for the database since the last look, which
    /// the window then says (TASK-24 #2): true once for each comment that waits. The
    /// window looks at the end of every event, after [`Jobs::poll`], so a comment that
    /// waited only for the save that ended in the same event is never said to wait.
    pub fn comment_started_waiting(&mut self) -> bool {
        match &mut self.comment {
            Some(commenting) if !commenting.announced => {
                commenting.announced = true;
                true
            }
            _ => false,
        }
    }

    /// Asks the job of `kind` to stop, if one runs; its end comes through
    /// [`Jobs::poll`], like any other.
    pub fn cancel(&self, kind: Kind) {
        match kind {
            Kind::Snapshot => {
                if let Some(snapshotting) = &self.snapshot {
                    snapshotting.job.cancel();
                }
            }
            Kind::Comparison => {
                if let Some(comparing) = &self.comparison {
                    comparing.job.cancel();
                }
            }
            Kind::Compaction => {
                if let Some(compacting) = &self.compaction {
                    compacting.job.cancel();
                }
            }
            Kind::Comment => {
                if let Some(commenting) = &self.comment {
                    commenting.job.cancel();
                }
            }
        }
    }

    /// The jobs that have ended, a comment before a snapshot before a comparison before
    /// a compaction; they leave their places. A snapshot whose save is over is waited for
    /// a moment, so that it ends with the command that waited for its save (TASK-21); so
    /// is a comment that waited for the save of a snapshot of its profile, once that
    /// snapshot has ended, so that the two are said together, the comment first
    /// (TASK-24 #1).
    pub fn poll(&mut self) -> Vec<Event> {
        let mut ended = Vec::new();
        let mut snapshot_of = None;
        if let Some(outcome) = self.snapshot.as_mut().and_then(|s| s.job.poll()) {
            let Snapshotting { profile, then, .. } = self.snapshot.take().expect("polled just now");
            snapshot_of = Some(profile.path.clone());
            ended.push(Event::Snapshot(Ended { profile, outcome }, then));
        }
        let comment_outcome = self.comment.as_mut().and_then(|c| {
            if snapshot_of.as_ref() == Some(&c.profile.path) {
                c.job.poll_within(job::GRACE)
            } else {
                c.job.poll()
            }
        });
        if let Some(outcome) = comment_outcome {
            let Commenting {
                profile, comment, ..
            } = self.comment.take().expect("polled just now");
            ended.insert(0, Event::Comment(Ended { profile, outcome }, comment));
        }
        if let Some(outcome) = self.comparison.as_mut().and_then(|c| c.job.poll()) {
            let Comparing { profile, after, .. } = self.comparison.take().expect("polled just now");
            ended.push(Event::Comparison(Ended { profile, outcome }, after));
        }
        if let Some(outcome) = self.compaction.as_mut().and_then(|c| c.job.poll()) {
            let Compacting { profile, .. } = self.compaction.take().expect("polled just now");
            ended.push(Event::Compaction(Ended { profile, outcome }));
        }
        ended
    }

    /// What the progress row shows: a snapshot before a comment waiting for the database
    /// (TASK-24), before a comparison that has run for [`COMPARISON_SHOWN_AFTER`] or
    /// longer, before a compaction. The comparison of
    /// «Знімок + порівняння» shows at once: it takes the row over from its snapshot, so
    /// the row, and the focus on it, stay (TASK-12 #5, #9).
    pub fn shown(&self) -> Option<Shown<'_>> {
        if let Some(snapshotting) = &self.snapshot {
            return Some(Shown::Snapshot(snapshotting.job.progress()));
        }
        if self.comment.is_some() {
            return Some(Shown::Comment);
        }
        if let Some(comparing) = &self.comparison {
            let elapsed = comparing.job.elapsed();
            if elapsed >= COMPARISON_SHOWN_AFTER || comparing.after.is_some() {
                return Some(Shown::Comparison {
                    progress: comparing.job.progress(),
                    elapsed,
                });
            }
        }
        self.compaction
            .as_ref()
            .map(|compacting| Shown::Compaction(&compacting.profile))
    }

    /// Whether a job of `kind` runs.
    pub fn runs(&self, kind: Kind) -> bool {
        match kind {
            Kind::Snapshot => self.snapshot.is_some(),
            Kind::Comparison => self.comparison.is_some(),
            Kind::Compaction => self.compaction.is_some(),
            Kind::Comment => self.comment.is_some(),
        }
    }

    /// Whether any job runs.
    pub fn any(&self) -> bool {
        self.snapshot.is_some()
            || self.comparison.is_some()
            || self.compaction.is_some()
            || self.comment.is_some()
    }

    /// The flags of the state, with `open` the database of the open profile.
    pub fn flags(&self, open: Option<&Path>) -> Flags {
        let is_open = |profile: &ProfileFile| open == Some(profile.path.as_path());
        Flags {
            snapshot_running: self.snapshot.is_some(),
            snapshotting_open_profile: self.snapshot.as_ref().is_some_and(|s| is_open(&s.profile)),
            comparison_running: self.comparison.is_some(),
            compaction_running: self.compaction.is_some(),
            compacting_open_profile: self
                .compaction
                .as_ref()
                .is_some_and(|c| is_open(&c.profile)),
            comment_waiting: self.comment.is_some(),
            commenting_open_profile: self.comment.as_ref().is_some_and(|c| is_open(&c.profile)),
        }
    }

    /// The profile whose database is at `path`, with its snapshots, while a compaction
    /// has closed that database.
    pub fn closed_profile(&self, path: &Path) -> Option<(&ProfileFile, &[SnapshotInfo])> {
        let compacting = self.compaction.as_ref()?;
        (compacting.profile.path == path)
            .then_some((&compacting.profile, compacting.snapshots.as_slice()))
    }

    /// The profile whose database was at `old` is now `profile`: renamed, and its file
    /// maybe with it (TASK-7). A job of it ends under the new name and path, so the
    /// difference of a comparison that still runs names its profile as it is now
    /// (TASK-13 #5).
    pub fn profile_renamed(&mut self, old: &Path, profile: &ProfileFile) {
        let places = [
            self.snapshot.as_mut().map(|s| &mut s.profile),
            self.comparison.as_mut().map(|c| &mut c.profile),
            self.compaction.as_mut().map(|c| &mut c.profile),
            self.comment.as_mut().map(|c| &mut c.profile),
        ];
        for place in places.into_iter().flatten() {
            if place.path == old {
                *place = profile.clone();
            }
        }
    }

    /// Whether a comparison of the profile whose database is at `path` reads that
    /// database now: it runs and loads its trees. Deleting the profile's snapshots, or
    /// the profile, waits for that, which is a matter of seconds (TASK-11 #17); a
    /// comparison that waits for the database holds nothing of it, and one whose trees
    /// are loaded has closed it.
    pub fn comparison_holds_database(&self, path: &Path) -> bool {
        self.comparison.as_ref().is_some_and(|c| {
            c.profile.path == path
                && c.job.is_running()
                && c.job.progress().phase() == compare::Phase::Loading
        })
    }

    /// Stops the snapshot being taken, if there is one, and waits for it, which is quick:
    /// the scan notices between folders and files, the save every few thousand entries.
    /// A snapshot that was done by then ends as it did.
    pub fn stop_snapshot(&mut self) -> Option<Ended<Outcome<Report, SnapshotError>>> {
        let Snapshotting { profile, job, .. } = self.snapshot.take()?;
        job.cancel();
        Some(Ended {
            profile,
            outcome: job.wait(),
        })
    }

    /// Waits for the compaction to end, if there is one; with `interrupt`, interrupts it
    /// first, which leaves the database as it was.
    pub fn finish_compaction(&mut self, interrupt: bool) {
        if let Some(Compacting { job, .. }) = self.compaction.take() {
            if interrupt {
                job.cancel();
            }
            let _ = job.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use revisor_core::job::Cancel;
    use revisor_core::profiles;
    use revisor_core::scan::synthetic;
    use revisor_core::store::{ProfileSettings, Snapshot};
    use std::fs;
    use std::path::PathBuf;
    use std::thread;
    use std::time::{Duration, Instant};

    /// A folder of profiles under the temp folder, removed on drop.
    struct Profiles(PathBuf);

    impl Profiles {
        fn new(test: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("revisor-jobs-{test}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            Self(dir)
        }

        /// A new profile without folders, so its snapshot is over at once.
        fn profile(&self, name: &str) -> ProfileFile {
            let mut settings = ProfileSettings::new_profile();
            settings.name = name.into();
            profiles::create(&self.0, &settings).unwrap()
        }
    }

    impl Drop for Profiles {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// An exclusive lock on the database of `profile`, held until the connection is
    /// dropped: no other connection can read the database meanwhile, as none can while a
    /// snapshot is committed or the database compacted.
    fn lock(profile: &ProfileFile) -> rusqlite::Connection {
        let conn = rusqlite::Connection::open(&profile.path).unwrap();
        conn.execute_batch("BEGIN EXCLUSIVE").unwrap();
        conn
    }

    /// Starts compacting the database of `profile`, with `snapshots` as its snapshots.
    fn compact(jobs: &mut Jobs, profile: &ProfileFile, snapshots: Vec<SnapshotInfo>) {
        let store = Store::open(&profile.path).unwrap();
        jobs.start_compaction(profile.clone(), snapshots, store);
    }

    /// Takes two snapshots of `profile` and gives their ids, oldest first.
    fn two_snapshots(profile: &ProfileFile) -> (i64, i64) {
        let mut jobs = Jobs::default();
        for _ in 0..2 {
            jobs.start_snapshot(profile.clone());
            poll_until_idle(&mut jobs);
        }
        let ids: Vec<i64> = profiles::snapshots_of(&profile.path)
            .unwrap()
            .iter()
            .map(|s| s.id)
            .collect();
        (ids[0], ids[1])
    }

    /// Takes a snapshot of `profile` and gives its report.
    fn snapshot(profile: &ProfileFile) -> Report {
        let mut jobs = Jobs::default();
        jobs.start_snapshot(profile.clone());
        match poll_until_idle(&mut jobs).pop() {
            Some(Event::Snapshot(
                Ended {
                    outcome: Ok(Outcome::Done(report)),
                    ..
                },
                None,
            )) => report,
            other => panic!("{other:?}"),
        }
    }

    /// How long a test waits for a job before it fails: far longer than any job of the
    /// tests takes, so that a slow machine does not fail them.
    const PATIENCE: Duration = Duration::from_secs(60);

    /// Waits until `done`, looking every `every`; fails, saying `what` never happened,
    /// after [`PATIENCE`].
    fn wait_until(what: &str, every: Duration, mut done: impl FnMut() -> bool) {
        let deadline = Instant::now() + PATIENCE;
        while !done() {
            assert!(Instant::now() < deadline, "{what} never happened");
            thread::sleep(every);
        }
    }

    /// Polls `jobs`, as the window's timer does, until no job is left, and gives the
    /// jobs that ended.
    fn poll_until_idle(jobs: &mut Jobs) -> Vec<Event> {
        poll_until_idle_by_poll(jobs)
            .into_iter()
            .flatten()
            .collect()
    }

    /// Polls `jobs` until no job is left, and gives what each poll found ended.
    fn poll_until_idle_by_poll(jobs: &mut Jobs) -> Vec<Vec<Event>> {
        let mut polls = Vec::new();
        wait_until("the end of the jobs", Duration::from_millis(5), || {
            polls.push(jobs.poll());
            !jobs.any()
        });
        polls
    }

    #[test]
    fn the_flags_come_from_the_jobs_for_the_open_profile() {
        let dir = Profiles::new("flags");
        let (a, b) = (dir.profile("A"), dir.profile("B"));
        let (first, second) = two_snapshots(&a);
        let mut jobs = Jobs::default();
        assert_eq!(jobs.flags(Some(&a.path)), Flags::default());

        // The comparison waits for the locked database, so it runs meanwhile.
        let held = lock(&a);
        assert!(jobs.start_comparison(a.clone(), first, second).is_none());
        jobs.start_snapshot(a.clone());
        compact(&mut jobs, &b, Vec::new());
        let running = Flags {
            snapshot_running: true,
            comparison_running: true,
            compaction_running: true,
            ..Flags::default()
        };
        assert_eq!(
            jobs.flags(Some(&a.path)),
            Flags {
                snapshotting_open_profile: true,
                ..running
            }
        );
        assert_eq!(
            jobs.flags(Some(&b.path)),
            Flags {
                compacting_open_profile: true,
                ..running
            }
        );
        assert_eq!(jobs.flags(None), running);

        drop(held);
        let ended = poll_until_idle(&mut jobs);
        assert_eq!(jobs.flags(Some(&a.path)), Flags::default());
        assert_eq!(jobs.flags(Some(&b.path)), Flags::default());
        assert!(
            ended.iter().any(|e| matches!(
                e,
                Event::Comparison(Ended { profile, outcome: Ok(Outcome::Done(d)) }, None)
                    if *profile == a && (d.earlier.id, d.later.id) == (first, second)
            )),
            "{ended:?}"
        );
    }

    #[test]
    fn the_progress_row_shows_a_snapshot_before_a_comparison_before_a_compaction() {
        let dir = Profiles::new("shown");
        let (a, b) = (dir.profile("A"), dir.profile("B"));
        let (first, second) = two_snapshots(&a);
        let mut jobs = Jobs::default();
        assert!(jobs.shown().is_none());

        compact(&mut jobs, &b, Vec::new());
        assert!(matches!(jobs.shown(), Some(Shown::Compaction(p)) if *p == b));
        // A comparison shows only once it has run for a second (TASK-11 #7).
        let held = lock(&a);
        assert!(jobs.start_comparison(a.clone(), first, second).is_none());
        assert!(matches!(jobs.shown(), Some(Shown::Compaction(_))));
        thread::sleep(COMPARISON_SHOWN_AFTER + Duration::from_millis(100));
        assert!(matches!(
            jobs.shown(),
            Some(Shown::Comparison { progress, elapsed })
                if progress.phase() == compare::Phase::Waiting && elapsed >= COMPARISON_SHOWN_AFTER
        ));
        assert_eq!(jobs.shown().map(|s| s.kind()), Some(Kind::Comparison));
        jobs.start_snapshot(a.clone());
        assert!(matches!(jobs.shown(), Some(Shown::Snapshot(_))));
        assert_eq!(jobs.shown().map(|s| s.kind()), Some(Kind::Snapshot));

        drop(held);
        let ended = poll_until_idle(&mut jobs);
        assert!(jobs.shown().is_none());
        assert!(
            ended.iter().any(|e| matches!(
                e,
                Event::Snapshot(Ended { profile, outcome: Ok(Outcome::Done(_)) }, None)
                    if *profile == a
            )),
            "{ended:?}"
        );
        assert!(
            ended.iter().any(|e| matches!(
                e,
                Event::Comparison(Ended { profile, outcome: Ok(Outcome::Done(_)) }, None)
                    if *profile == a
            )),
            "{ended:?}"
        );
        assert!(
            ended.iter().any(|e| matches!(
                e,
                Event::Compaction(Ended { profile, outcome: Ok(Outcome::Done(_)) }) if *profile == b
            )),
            "{ended:?}"
        );
    }

    #[test]
    fn a_new_comparison_takes_the_place_of_the_one_that_runs() {
        let dir = Profiles::new("replace");
        let a = dir.profile("A");
        let (first, second) = two_snapshots(&a);
        let mut jobs = Jobs::default();
        let held = lock(&a);
        assert!(jobs.start_comparison(a.clone(), first, first).is_none());
        assert!(jobs.start_comparison(a.clone(), first, second).is_none());
        assert!(jobs.runs(Kind::Comparison));
        drop(held);
        let ended = poll_until_idle(&mut jobs);
        // The first comparison is forgotten: only the second one ends.
        let comparisons: Vec<(i64, i64)> = ended
            .iter()
            .filter_map(|e| match e {
                Event::Comparison(
                    Ended {
                        outcome: Ok(Outcome::Done(d)),
                        ..
                    },
                    _,
                ) => Some((d.earlier.id, d.later.id)),
                _ => None,
            })
            .collect();
        assert_eq!(comparisons, [(first, second)]);
        assert_eq!(ended.len(), 1, "{ended:?}");
    }

    #[test]
    fn a_comparison_of_a_renamed_profile_ends_under_its_new_name() {
        let dir = Profiles::new("renamed");
        let a = dir.profile("A");
        let (first, second) = two_snapshots(&a);
        let mut jobs = Jobs::default();
        let held = lock(&a);
        assert!(jobs.start_comparison(a.clone(), first, second).is_none());
        let renamed = ProfileFile {
            path: a.path.with_file_name("B.db"),
            name: "B".into(),
        };
        // Another profile's name changes nothing.
        jobs.profile_renamed(&renamed.path, &a);
        jobs.profile_renamed(&a.path, &renamed);
        drop(held);
        match poll_until_idle(&mut jobs).pop() {
            Some(Event::Comparison(Ended { profile, .. }, None)) => assert_eq!(profile, renamed),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_comparison_holds_the_database_while_it_loads_its_trees() {
        let dir = Profiles::new("holds");
        let (a, b) = (dir.profile("A"), dir.profile("B"));
        // Two large snapshots, so that loading them takes a while.
        let (first, second) = {
            let mut store = Store::open(&a.path).unwrap();
            let tree = synthetic(200_000);
            let first = store
                .save(&Snapshot::of_tree(tree.clone()), &Cancel::new())
                .unwrap();
            let second = store
                .save(&Snapshot::of_tree(tree), &Cancel::new())
                .unwrap();
            (first, second)
        };
        let mut jobs = Jobs::default();
        assert!(!jobs.comparison_holds_database(&a.path));

        // A comparison that waits for the database holds nothing of it.
        let held = lock(&a);
        assert!(jobs.start_comparison(a.clone(), first, second).is_none());
        wait_until("the comparison showing", Duration::from_millis(5), || {
            jobs.shown().is_some_and(|s| s.kind() == Kind::Comparison)
        });
        assert!(!jobs.comparison_holds_database(&a.path));
        // Then it loads, holding the database, and lets it go once the trees are loaded,
        // polled or not.
        drop(held);
        wait_until("the comparison loading", Duration::from_millis(1), || {
            assert!(
                jobs.poll().is_empty(),
                "the comparison ended before it was seen loading"
            );
            jobs.comparison_holds_database(&a.path)
        });
        assert!(!jobs.comparison_holds_database(&b.path));
        wait_until("the end of the loading", Duration::from_millis(1), || {
            !jobs.comparison_holds_database(&a.path)
        });
        assert!(jobs.runs(Kind::Comparison));
        poll_until_idle(&mut jobs);
        assert!(!jobs.comparison_holds_database(&a.path));
    }

    #[test]
    fn the_snapshot_of_snapshot_and_compare_ends_with_the_comparison_it_is_to_have() {
        let dir = Profiles::new("then");
        let a = dir.profile("A");
        let first = snapshot(&a);
        let mut jobs = Jobs::default();
        // A regular snapshot is to have no comparison (TASK-12 #2).
        jobs.start_snapshot(a.clone());
        let ended = poll_until_idle(&mut jobs);
        assert!(
            matches!(
                ended.as_slice(),
                [Event::Snapshot(
                    Ended {
                        outcome: Ok(Outcome::Done(_)),
                        ..
                    },
                    None
                )]
            ),
            "{ended:?}"
        );
        // «Знімок + порівняння» carries the snapshot to compare the new one with (#1), or
        // none (#4), to the snapshot's end; the window starts the comparison.
        for with in [Some(first.id), None] {
            assert!(jobs.start_snapshot_and_compare(a.clone(), with).is_none());
            assert!(jobs.runs(Kind::Snapshot));
            assert!(!jobs.runs(Kind::Comparison));
            let ended = poll_until_idle(&mut jobs);
            assert!(
                matches!(
                    ended.as_slice(),
                    [Event::Snapshot(
                        Ended { profile, outcome: Ok(Outcome::Done(_)) },
                        Some(ThenCompare { with: then }),
                    )] if *profile == a && *then == with
                ),
                "{ended:?}"
            );
        }
    }

    #[test]
    fn a_comparison_started_during_the_snapshot_leaves_it_a_regular_one() {
        let dir = Profiles::new("meanwhile");
        let a = dir.profile("A");
        let (first, second) = two_snapshots(&a);
        let mut jobs = Jobs::default();
        assert!(
            jobs.start_snapshot_and_compare(a.clone(), Some(second))
                .is_none()
        );
        // The later comparison is the one wanted (TASK-12 #11).
        assert!(jobs.start_comparison(a.clone(), first, second).is_none());
        let ended = poll_until_idle(&mut jobs);
        assert!(
            ended.iter().any(|e| matches!(
                e,
                Event::Snapshot(
                    Ended {
                        outcome: Ok(Outcome::Done(_)),
                        ..
                    },
                    None
                )
            )),
            "{ended:?}"
        );
        assert!(
            ended.iter().any(|e| matches!(
                e,
                Event::Comparison(Ended { outcome: Ok(Outcome::Done(d)), .. }, None)
                    if (d.earlier.id, d.later.id) == (first, second)
            )),
            "{ended:?}"
        );
        assert_eq!(ended.len(), 2, "{ended:?}");
    }

    #[test]
    fn snapshot_and_compare_takes_the_place_of_the_comparison_that_runs() {
        let dir = Profiles::new("command-later");
        let (a, b) = (dir.profile("A"), dir.profile("B"));
        let (first, second) = two_snapshots(&a);
        let mut jobs = Jobs::default();
        let held = lock(&a);
        assert!(jobs.start_comparison(a.clone(), first, second).is_none());
        // The command is the later comparison from its start (TASK-12 #11).
        assert!(jobs.start_snapshot_and_compare(b.clone(), None).is_none());
        assert!(!jobs.runs(Kind::Comparison));
        assert!(!jobs.flags(None).comparison_running);
        drop(held);
        let ended = poll_until_idle(&mut jobs);
        // The comparison was forgotten: only the snapshot ends.
        assert!(
            matches!(
                ended.as_slice(),
                [Event::Snapshot(Ended { profile, .. }, Some(ThenCompare { with: None }))]
                    if *profile == b
            ),
            "{ended:?}"
        );
    }

    #[test]
    fn the_comparison_after_the_snapshot_shows_at_once_and_ends_with_its_report() {
        let dir = Profiles::new("after");
        let a = dir.profile("A");
        let first = snapshot(&a);
        let second = snapshot(&a);
        let later = second.id;
        let mut jobs = Jobs::default();
        jobs.start_comparison_after_snapshot(a.clone(), first.id, second);
        // In the progress row at once, in place of its snapshot (TASK-12 #5, #9).
        assert!(matches!(
            jobs.shown(),
            Some(Shown::Comparison { elapsed, .. }) if elapsed < COMPARISON_SHOWN_AFTER
        ));
        assert!(jobs.flags(Some(&a.path)).comparison_running);
        let ended = poll_until_idle(&mut jobs);
        match ended.as_slice() {
            [
                Event::Comparison(
                    Ended {
                        profile,
                        outcome: Ok(Outcome::Done(d)),
                    },
                    Some(report),
                ),
            ] => {
                assert_eq!(*profile, a);
                assert_eq!((d.earlier.id, d.later.id), (first.id, later));
                assert_eq!(report.id, later);
            }
            other => panic!("{other:?}"),
        }

        // Cancelled, it still gives the report back: the snapshot stays (#9). It is
        // cancelled while it waits for the database: snapshots without folders are
        // compared before a cancel is looked at otherwise.
        let third = snapshot(&a);
        let third_id = third.id;
        let held = lock(&a);
        jobs.start_comparison_after_snapshot(a.clone(), first.id, third);
        wait_until("the comparison waiting", Duration::from_millis(5), || {
            matches!(
                jobs.shown(),
                Some(Shown::Comparison { progress, .. })
                    if progress.phase() == compare::Phase::Waiting
            )
        });
        // The database stays held: a comparison sees a cancel while it waits.
        jobs.cancel(Kind::Comparison);
        let ended = poll_until_idle(&mut jobs);
        drop(held);
        assert!(
            matches!(
                ended.as_slice(),
                [Event::Comparison(Ended { outcome: Ok(Outcome::Cancelled), .. }, Some(report))]
                    if report.id == third_id
            ),
            "{ended:?}"
        );
    }

    #[test]
    fn a_comparison_in_place_of_that_of_snapshot_and_compare_gives_its_report_back() {
        let dir = Profiles::new("superseded");
        let (a, b) = (dir.profile("A"), dir.profile("B"));
        let first = snapshot(&a);
        let second = snapshot(&a);
        let later = second.id;
        let mut jobs = Jobs::default();
        let held = lock(&a);
        // By a comparison (TASK-11 #15)…
        jobs.start_comparison_after_snapshot(a.clone(), first.id, second.clone());
        let superseded = jobs
            .start_comparison(a.clone(), first.id, later)
            .expect("the command's comparison was superseded");
        assert_eq!(
            (superseded.profile, superseded.report.id),
            (a.clone(), later)
        );
        // …and by another «Знімок + порівняння».
        let _ = jobs.forget_comparison();
        jobs.start_comparison_after_snapshot(a.clone(), first.id, second);
        let superseded = jobs
            .start_snapshot_and_compare(b.clone(), Some(first.id))
            .expect("the command's comparison was superseded");
        assert_eq!((superseded.profile, superseded.report.id), (a, later));
        drop(held);
        let ended = poll_until_idle(&mut jobs);
        assert!(
            matches!(ended.as_slice(), [Event::Snapshot(Ended { profile, .. }, Some(_))] if *profile == b),
            "{ended:?}"
        );
    }

    #[test]
    fn a_profile_being_compacted_is_known_with_its_snapshots() {
        let dir = Profiles::new("closed");
        let (a, b) = (dir.profile("A"), dir.profile("B"));
        let snapshots = vec![SnapshotInfo {
            id: 3,
            taken_at: 1_793_000_000_000,
            comment: Some("kept".into()),
        }];
        let mut jobs = Jobs::default();
        assert!(jobs.closed_profile(&b.path).is_none());

        compact(&mut jobs, &b, snapshots);
        let (profile, kept) = jobs.closed_profile(&b.path).expect("B is being compacted");
        assert_eq!(profile, &b);
        assert_eq!(kept.len(), 1);
        assert_eq!((kept[0].id, kept[0].comment.as_deref()), (3, Some("kept")));
        assert!(jobs.closed_profile(&a.path).is_none());
        // A snapshot of another profile changes nothing of it.
        jobs.start_snapshot(a.clone());
        assert!(jobs.closed_profile(&a.path).is_none());
        assert!(jobs.closed_profile(&b.path).is_some());

        poll_until_idle(&mut jobs);
        assert!(jobs.closed_profile(&b.path).is_none());
    }

    #[test]
    fn closing_stops_the_snapshot_and_waits_for_the_compaction() {
        let dir = Profiles::new("closing");
        let (a, b) = (dir.profile("A"), dir.profile("B"));
        let mut jobs = Jobs::default();
        assert!(jobs.stop_snapshot().is_none());

        jobs.start_snapshot(a.clone());
        compact(&mut jobs, &b, Vec::new());
        let stopped = jobs.stop_snapshot().expect("a snapshot runs");
        assert_eq!(stopped.profile, a);
        // Stopped, or done by then: either way it is over.
        assert!(
            matches!(stopped.outcome, Ok(Outcome::Cancelled | Outcome::Done(_))),
            "{stopped:?}"
        );
        assert!(!jobs.runs(Kind::Snapshot));
        assert!(jobs.runs(Kind::Compaction));

        jobs.finish_compaction(false);
        assert!(!jobs.any());
        assert!(jobs.poll().is_empty());
    }

    /// The comment of the snapshot `id` of `profile`, as its database keeps it.
    fn comment_of(profile: &ProfileFile, id: i64) -> Option<String> {
        profiles::snapshots_of(&profile.path)
            .unwrap()
            .into_iter()
            .find(|s| s.id == id)
            .unwrap()
            .comment
    }

    fn comment(id: i64, text: &str) -> Comment {
        Comment {
            id,
            comment: Some(text.into()),
        }
    }

    #[test]
    fn a_comment_on_a_free_database_is_written_at_once_without_a_job() {
        let dir = Profiles::new("comment-free");
        let a = dir.profile("A");
        let id = snapshot(&a).id;
        let mut jobs = Jobs::default();
        assert!(matches!(
            jobs.comment(a.clone(), comment(id, "Перед оновленням")),
            Some(Ok(()))
        ));
        assert!(!jobs.any());
        assert!(!jobs.comment_started_waiting(), "nothing waits");
        assert_eq!(comment_of(&a, id).as_deref(), Some("Перед оновленням"));
        let removed = Comment { id, comment: None };
        assert!(matches!(jobs.comment(a.clone(), removed), Some(Ok(()))));
        assert_eq!(comment_of(&a, id), None);
    }

    #[test]
    fn a_comment_on_a_busy_database_waits_in_the_progress_row_and_is_written_once_free() {
        let dir = Profiles::new("comment-busy");
        let (a, b) = (dir.profile("A"), dir.profile("B"));
        let id = snapshot(&a).id;
        let mut jobs = Jobs::default();
        let held = lock(&a);
        assert!(jobs.comment(a.clone(), comment(id, "Після")).is_none());

        // It waits as a job of its own: the progress row shows it, the flags say so for
        // its profile, and it holds on for longer than SQLite's own wait would.
        assert!(jobs.runs(Kind::Comment));
        assert!(matches!(jobs.shown(), Some(Shown::Comment)));
        let waiting = Flags {
            comment_waiting: true,
            ..Flags::default()
        };
        assert_eq!(
            jobs.flags(Some(&a.path)),
            Flags {
                commenting_open_profile: true,
                ..waiting
            }
        );
        assert_eq!(jobs.flags(Some(&b.path)), waiting);
        // The window says once that it waits.
        assert!(jobs.comment_started_waiting());
        assert!(!jobs.comment_started_waiting());
        thread::sleep(Duration::from_millis(300));
        assert!(jobs.poll().is_empty());

        // A snapshot being taken shows before it.
        jobs.start_snapshot(b.clone());
        assert!(matches!(jobs.shown(), Some(Shown::Snapshot(_))));

        drop(held);
        let ended = poll_until_idle(&mut jobs);
        assert!(
            ended.iter().any(|e| matches!(
                e,
                Event::Comment(Ended { profile, outcome: Ok(Outcome::Done(())) }, c)
                    if *profile == a && *c == comment(id, "Після")
            )),
            "{ended:?}"
        );
        assert_eq!(comment_of(&a, id).as_deref(), Some("Після"));
        assert_eq!(jobs.flags(Some(&a.path)), Flags::default());
    }

    #[test]
    fn a_comment_cancelled_while_it_waits_leaves_the_earlier_one() {
        let dir = Profiles::new("comment-cancel");
        let a = dir.profile("A");
        let id = snapshot(&a).id;
        let mut jobs = Jobs::default();
        assert!(matches!(
            jobs.comment(a.clone(), comment(id, "Раніший")),
            Some(Ok(()))
        ));
        let held = lock(&a);
        assert!(jobs.comment(a.clone(), comment(id, "Новий")).is_none());
        jobs.cancel(Kind::Comment);
        let ended = poll_until_idle(&mut jobs);
        drop(held);
        assert!(
            matches!(
                ended.as_slice(),
                [Event::Comment(
                    Ended {
                        outcome: Ok(Outcome::Cancelled),
                        ..
                    },
                    _
                )]
            ),
            "{ended:?}"
        );
        assert_eq!(comment_of(&a, id).as_deref(), Some("Раніший"));
    }

    #[test]
    fn a_comment_that_waited_for_a_snapshot_of_its_profile_ends_with_it_and_first() {
        // The snapshot and the comment both wait for the lock; whichever gets it first,
        // the comment never ends in a later poll than the snapshot, and when both end in
        // one poll, the comment comes first (TASK-21, TASK-24 #1).
        let dir = Profiles::new("comment-snapshot");
        let a = dir.profile("A");
        let id = snapshot(&a).id;
        for _ in 0..3 {
            let mut jobs = Jobs::default();
            let held = lock(&a);
            jobs.start_snapshot(a.clone());
            assert!(jobs.comment(a.clone(), comment(id, "Разом")).is_none());
            thread::sleep(Duration::from_millis(150));
            drop(held);
            let polls = poll_until_idle_by_poll(&mut jobs);
            let poll_of = |kind: Kind| {
                polls.iter().position(|poll| {
                    poll.iter().any(|e| match e {
                        Event::Comment(..) => kind == Kind::Comment,
                        Event::Snapshot(..) => kind == Kind::Snapshot,
                        _ => false,
                    })
                })
            };
            let (commented, snapshotted) = (poll_of(Kind::Comment), poll_of(Kind::Snapshot));
            assert!(commented <= snapshotted, "{polls:?}");
            if commented == snapshotted {
                let poll = &polls[commented.unwrap()];
                assert!(
                    matches!(poll.as_slice(), [Event::Comment(..), Event::Snapshot(..)]),
                    "{poll:?}"
                );
            }
            assert_eq!(comment_of(&a, id).as_deref(), Some("Разом"));
        }
    }
}
