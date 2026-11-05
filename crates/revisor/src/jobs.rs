//! The background jobs of the window (ADR-0011): one place for each kind, which holds
//! the job with the profile it is of, and what the jobs mean for the window — which one
//! the progress row shows, the flags of the state the commands decide by, and the
//! profile whose database a compaction has closed. The window starts, cancels and polls
//! the jobs here and holds none of their parts itself.

use std::path::Path;

use revisor_core::job::{Job, Lost, Outcome};
use revisor_core::profiles::ProfileFile;
use revisor_core::snapshot::{self, Report, SnapshotError, SnapshotWork};
use revisor_core::store::{Compacted, Compaction, SnapshotInfo, Store, StoreError};

/// A kind of background job; at most one of each runs at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Snapshot,
    Compaction,
}

/// What the progress row shows: the progress of the snapshot being taken, else the
/// profile whose database is being compacted.
#[derive(Clone, Copy, Debug)]
pub enum Shown<'a> {
    Snapshot(&'a snapshot::Progress),
    Compaction(&'a ProfileFile),
}

impl Shown<'_> {
    pub fn kind(&self) -> Kind {
        match self {
            Shown::Snapshot(_) => Kind::Snapshot,
            Shown::Compaction(_) => Kind::Compaction,
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
}

/// A job that has ended: the profile it was of, and how it ended; `Err(Lost)` when its
/// thread ended without saying.
#[derive(Debug)]
pub struct Ended<O> {
    pub profile: ProfileFile,
    pub outcome: Result<O, Lost>,
}

/// A job that [`Jobs::poll`] found ended.
#[derive(Debug)]
pub enum Event {
    Snapshot(Ended<Outcome<Report, SnapshotError>>),
    Compaction(Ended<Outcome<Compacted, StoreError>>),
}

/// A snapshot being taken, and the profile it is of.
struct Snapshotting {
    profile: ProfileFile,
    job: Job<SnapshotWork>,
}

/// A database being compacted, with its profile and that profile's snapshots left after
/// the deletion: the database is closed even to reading meanwhile (TASK-9), so whoever
/// shows the profile takes them from here.
struct Compacting {
    profile: ProfileFile,
    snapshots: Vec<SnapshotInfo>,
    job: Job<Compaction>,
}

/// The background jobs, one place for each kind.
#[derive(Default)]
pub struct Jobs {
    snapshot: Option<Snapshotting>,
    compaction: Option<Compacting>,
}

impl Jobs {
    /// Starts a snapshot of `profile`; the commands see to it that no other one runs.
    pub fn start_snapshot(&mut self, profile: ProfileFile) {
        debug_assert!(self.snapshot.is_none(), "one snapshot at a time");
        let job = Job::start(SnapshotWork::new(&profile.path, None));
        self.snapshot = Some(Snapshotting { profile, job });
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

    /// Asks the job of `kind` to stop, if one runs; its end comes through
    /// [`Jobs::poll`], like any other.
    pub fn cancel(&self, kind: Kind) {
        match kind {
            Kind::Snapshot => {
                if let Some(snapshotting) = &self.snapshot {
                    snapshotting.job.cancel();
                }
            }
            Kind::Compaction => {
                if let Some(compacting) = &self.compaction {
                    compacting.job.cancel();
                }
            }
        }
    }

    /// The jobs that have ended, a snapshot before a compaction; they leave their places.
    /// A snapshot whose save is over is waited for a moment, so that it ends with the
    /// command that waited for its save (TASK-21).
    pub fn poll(&mut self) -> Vec<Event> {
        let mut ended = Vec::new();
        if let Some(outcome) = self.snapshot.as_mut().and_then(|s| s.job.poll()) {
            let Snapshotting { profile, .. } = self.snapshot.take().expect("polled just now");
            ended.push(Event::Snapshot(Ended { profile, outcome }));
        }
        if let Some(outcome) = self.compaction.as_mut().and_then(|c| c.job.poll()) {
            let Compacting { profile, .. } = self.compaction.take().expect("polled just now");
            ended.push(Event::Compaction(Ended { profile, outcome }));
        }
        ended
    }

    /// What the progress row shows: a snapshot before a compaction.
    pub fn shown(&self) -> Option<Shown<'_>> {
        match (&self.snapshot, &self.compaction) {
            (Some(snapshotting), _) => Some(Shown::Snapshot(snapshotting.job.progress())),
            (None, Some(compacting)) => Some(Shown::Compaction(&compacting.profile)),
            (None, None) => None,
        }
    }

    /// Whether a job of `kind` runs.
    pub fn runs(&self, kind: Kind) -> bool {
        match kind {
            Kind::Snapshot => self.snapshot.is_some(),
            Kind::Compaction => self.compaction.is_some(),
        }
    }

    /// Whether any job runs.
    pub fn any(&self) -> bool {
        self.snapshot.is_some() || self.compaction.is_some()
    }

    /// The flags of the state, with `open` the database of the open profile.
    pub fn flags(&self, open: Option<&Path>) -> Flags {
        let is_open = |profile: &ProfileFile| open == Some(profile.path.as_path());
        Flags {
            snapshot_running: self.snapshot.is_some(),
            snapshotting_open_profile: self.snapshot.as_ref().is_some_and(|s| is_open(&s.profile)),
            // Comparisons come with TASK-11.
            comparison_running: false,
            compaction_running: self.compaction.is_some(),
            compacting_open_profile: self
                .compaction
                .as_ref()
                .is_some_and(|c| is_open(&c.profile)),
        }
    }

    /// The profile whose database is at `path`, with its snapshots, while a compaction
    /// has closed that database.
    pub fn closed_profile(&self, path: &Path) -> Option<(&ProfileFile, &[SnapshotInfo])> {
        let compacting = self.compaction.as_ref()?;
        (compacting.profile.path == path)
            .then_some((&compacting.profile, compacting.snapshots.as_slice()))
    }

    /// Stops the snapshot being taken, if there is one, and waits for it, which is quick:
    /// the scan notices between folders and files, the save every few thousand entries.
    /// A snapshot that was done by then ends as it did.
    pub fn stop_snapshot(&mut self) -> Option<Ended<Outcome<Report, SnapshotError>>> {
        let Snapshotting { profile, job } = self.snapshot.take()?;
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
    use revisor_core::profiles;
    use revisor_core::store::ProfileSettings;
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

    /// Starts compacting the database of `profile`, with `snapshots` as its snapshots.
    fn compact(jobs: &mut Jobs, profile: &ProfileFile, snapshots: Vec<SnapshotInfo>) {
        let store = Store::open(&profile.path).unwrap();
        jobs.start_compaction(profile.clone(), snapshots, store);
    }

    /// Polls `jobs`, as the window's timer does, until no job is left, and gives the
    /// jobs that ended.
    fn poll_until_idle(jobs: &mut Jobs) -> Vec<Event> {
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut ended = Vec::new();
        while jobs.any() {
            assert!(Instant::now() < deadline, "the jobs never ended");
            ended.extend(jobs.poll());
            thread::sleep(Duration::from_millis(5));
        }
        ended
    }

    #[test]
    fn the_flags_come_from_the_jobs_for_the_open_profile() {
        let dir = Profiles::new("flags");
        let (a, b) = (dir.profile("A"), dir.profile("B"));
        let mut jobs = Jobs::default();
        assert_eq!(jobs.flags(Some(&a.path)), Flags::default());

        jobs.start_snapshot(a.clone());
        compact(&mut jobs, &b, Vec::new());
        let running = Flags {
            snapshot_running: true,
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

        poll_until_idle(&mut jobs);
        assert_eq!(jobs.flags(Some(&a.path)), Flags::default());
        assert_eq!(jobs.flags(Some(&b.path)), Flags::default());
    }

    #[test]
    fn the_progress_row_shows_a_snapshot_before_a_compaction() {
        let dir = Profiles::new("shown");
        let (a, b) = (dir.profile("A"), dir.profile("B"));
        let mut jobs = Jobs::default();
        assert!(jobs.shown().is_none());

        compact(&mut jobs, &b, Vec::new());
        assert!(matches!(jobs.shown(), Some(Shown::Compaction(p)) if *p == b));
        jobs.start_snapshot(a.clone());
        assert!(matches!(jobs.shown(), Some(Shown::Snapshot(_))));
        assert_eq!(jobs.shown().map(|s| s.kind()), Some(Kind::Snapshot));

        let ended = poll_until_idle(&mut jobs);
        assert!(jobs.shown().is_none());
        assert!(
            ended.iter().any(|e| matches!(
                e,
                Event::Snapshot(Ended { profile, outcome: Ok(Outcome::Done(_)) }) if *profile == a
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
}
