//! The sound that ends a snapshot or «Знімок + порівняння» (TASK-46), apart from the
//! window and from Windows: which one, if any, and the WAV data of revisor's own two
//! sounds, made from plain tones so that they need no file and no license.

use std::time::Duration;

/// How long an operation must have run for its end to sound while «Лише після операцій,
/// довших за 10 секунд» is on (TASK-46 #5): Nielsen's limit for keeping the attention on
/// the task, after which people turn to something else.
pub const LONG_AFTER: Duration = Duration::from_secs(10);

/// revisor's two sounds (TASK-46 #3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chime {
    /// The operation succeeded.
    Done,
    /// It failed or was cancelled.
    NoResult,
}

/// What the sound settings of one operation say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundSetting {
    /// The operation's own check box: «Відтворювати звук після …».
    pub on: bool,
    /// «Лише після операцій, довших за 10 секунд».
    pub only_after_long: bool,
}

/// The operation whose sound settings an end follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Snapshot,
    SnapshotAndCompare,
}

/// How a snapshot or «Знімок + порівняння» came to an end, as the window sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// A snapshot ended without going on to a comparison: a regular one — also one a
    /// comparison took from «Знімок + порівняння» (TASK-12 #11) — or that of «Знімок +
    /// порівняння» failed or cancelled, `of_snapshot_and_compare`; `stopped` on closing
    /// or restarting.
    Snapshot {
        of_snapshot_and_compare: bool,
        stopped: bool,
    },
    /// «Знімок + порівняння» saved its snapshot and had nothing to compare it with.
    NothingToCompare,
    /// The comparison of «Знімок + порівняння» ended.
    Comparison,
    /// Another comparison took the place of the comparison of «Знімок + порівняння».
    Superseded,
}

/// Whose settings `end` sounds by, or `None` when it ends silent: a snapshot stopped on
/// closing or restarting, and a comparison the user replaced themselves (TASK-46 #6, #7).
pub fn operation(end: End) -> Option<Operation> {
    match end {
        End::Snapshot { stopped: true, .. } | End::Superseded => None,
        End::Snapshot {
            of_snapshot_and_compare: false,
            ..
        } => Some(Operation::Snapshot),
        End::Snapshot { .. } | End::NothingToCompare | End::Comparison => {
            Some(Operation::SnapshotAndCompare)
        }
    }
}

/// The sound an operation that ran for `lasted` ends with, `done` telling whether it
/// succeeded; none while its check box is off, or for a short operation while the
/// threshold is on — strictly longer than [`LONG_AFTER`] (TASK-46 #3, #5).
pub fn chime(setting: SoundSetting, lasted: Duration, done: bool) -> Option<Chime> {
    if !setting.on || (setting.only_after_long && lasted <= LONG_AFTER) {
        return None;
    }
    Some(if done { Chime::Done } else { Chime::NoResult })
}

/// One tone of a sound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Note {
    /// Pitch, in hertz.
    pub hz: f32,
    /// Length, in milliseconds.
    pub ms: u32,
}

/// The notes of each sound: «готово» goes up, «без результату» goes down.
pub fn notes(chime: Chime) -> &'static [Note] {
    match chime {
        Chime::Done => &[
            Note { hz: 784.0, ms: 110 },
            Note {
                hz: 1046.5,
                ms: 170,
            },
        ],
        Chime::NoResult => &[Note { hz: 523.3, ms: 150 }, Note { hz: 392.0, ms: 230 }],
    }
}

const SAMPLE_RATE: u32 = 22_050;
/// The loudness of a note's peak, of the loudest a sample can be.
const VOLUME: f32 = 0.35;

/// A 16-bit mono PCM WAV file of `notes`, one after another: each a sine with a little
/// of its octave, so that it sounds like a soft bell, which starts in 5 ms and fades out.
pub fn wav(notes: &[Note]) -> Vec<u8> {
    let mut samples: Vec<i16> = Vec::new();
    for note in notes {
        let count = (SAMPLE_RATE as u64 * u64::from(note.ms) / 1000) as usize;
        let attack = (SAMPLE_RATE as usize * 5 / 1000).max(1);
        for i in 0..count {
            let t = i as f32 / SAMPLE_RATE as f32;
            let phase = std::f32::consts::TAU * note.hz * t;
            let tone = phase.sin() + 0.3 * (2.0 * phase).sin();
            let rise = (i as f32 / attack as f32).min(1.0);
            let fade = (-4.0 * i as f32 / count as f32).exp() * (1.0 - i as f32 / count as f32);
            let value = tone / 1.3 * rise * fade * VOLUME;
            samples.push((value * f32::from(i16::MAX)) as i16);
        }
    }
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ON: SoundSetting = SoundSetting {
        on: true,
        only_after_long: true,
    };
    const LONG: Duration = Duration::from_secs(11);

    #[test]
    fn a_long_operation_ends_with_its_result() {
        assert_eq!(chime(ON, LONG, true), Some(Chime::Done));
        assert_eq!(chime(ON, LONG, false), Some(Chime::NoResult));
    }

    #[test]
    fn an_operation_whose_check_box_is_off_is_silent() {
        let off = SoundSetting { on: false, ..ON };
        for done in [true, false] {
            assert_eq!(chime(off, LONG, done), None);
            let always = SoundSetting {
                only_after_long: false,
                ..off
            };
            assert_eq!(chime(always, Duration::ZERO, done), None);
        }
    }

    #[test]
    fn the_threshold_silences_both_sounds_up_to_ten_seconds() {
        // TASK-46 #5: «довших за 10 секунд» — exactly 10 seconds is not longer.
        for done in [true, false] {
            assert_eq!(chime(ON, Duration::ZERO, done), None);
            assert_eq!(chime(ON, LONG_AFTER, done), None);
            assert!(chime(ON, LONG_AFTER + Duration::from_millis(1), done).is_some());
        }
    }

    #[test]
    fn without_the_threshold_every_end_sounds() {
        let always = SoundSetting {
            only_after_long: false,
            ..ON
        };
        assert_eq!(chime(always, Duration::ZERO, true), Some(Chime::Done));
        assert_eq!(chime(always, Duration::ZERO, false), Some(Chime::NoResult));
    }

    #[test]
    fn each_end_follows_the_settings_of_its_operation() {
        let snapshot = |of_snapshot_and_compare, stopped| End::Snapshot {
            of_snapshot_and_compare,
            stopped,
        };
        assert_eq!(operation(snapshot(false, false)), Some(Operation::Snapshot));
        // The snapshot of «Знімок + порівняння» failed or cancelled ends that command.
        assert_eq!(
            operation(snapshot(true, false)),
            Some(Operation::SnapshotAndCompare)
        );
        assert_eq!(
            operation(End::Comparison),
            Some(Operation::SnapshotAndCompare)
        );
        // TASK-46 #6: nothing to compare with is still the end of «Знімок + порівняння».
        assert_eq!(
            operation(End::NothingToCompare),
            Some(Operation::SnapshotAndCompare)
        );
        // TASK-46 #7: a comparison that replaced the comparison is silent; the snapshot it
        // took over arrives as a regular one and follows the snapshot's settings.
        assert_eq!(operation(End::Superseded), None);
        // Stopped on closing or restarting: silent.
        assert_eq!(operation(snapshot(false, true)), None);
        assert_eq!(operation(snapshot(true, true)), None);
    }

    #[test]
    fn the_two_sounds_differ_and_are_valid_wav() {
        let done = wav(notes(Chime::Done));
        let no_result = wav(notes(Chime::NoResult));
        assert_ne!(done, no_result);
        for data in [&done, &no_result] {
            assert_eq!(&data[..4], b"RIFF");
            assert_eq!(&data[8..16], b"WAVEfmt ");
            let riff = u32::from_le_bytes(data[4..8].try_into().unwrap());
            assert_eq!(riff as usize, data.len() - 8);
            let len = u32::from_le_bytes(data[40..44].try_into().unwrap());
            assert_eq!(len as usize, data.len() - 44);
            // Short enough not to hold anything up: under half a second.
            assert!(len < SAMPLE_RATE);
        }
    }
}
