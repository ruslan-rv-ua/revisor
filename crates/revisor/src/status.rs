//! What the status bar says, apart from the window (TASK-21). The messages of one event
//! (a command, a tick of the jobs' timer) join into one, so that a screen reader, which
//! reads only the last change of a live region, reads all of them: «Коментар збережено.
//! Знімок збережено: …».

use std::cell::RefCell;

/// Gathers the messages of the event being handled.
#[derive(Default)]
pub struct Announcer {
    /// The messages said so far by the event being gathered, or `None` between events.
    gathered: RefCell<Option<Vec<String>>>,
}

impl Announcer {
    /// Takes a message: between events it comes back to be shown at once; during a
    /// gathered event it is kept for the event's end.
    pub fn say(&self, text: &str) -> Option<String> {
        match self.gathered.borrow_mut().as_mut() {
            Some(messages) => {
                messages.push(text.to_owned());
                None
            }
            None => Some(text.to_owned()),
        }
    }

    /// Runs `event` and gives back what it said, joined into one message, or `None` when
    /// it said nothing. An event run inside another leaves its messages to the outer one.
    pub fn gather<T>(&self, event: impl FnOnce() -> T) -> (T, Option<String>) {
        let outermost = {
            let mut gathered = self.gathered.borrow_mut();
            let started = gathered.is_none();
            if started {
                *gathered = Some(Vec::new());
            }
            started
        };
        let answer = event();
        if !outermost {
            return (answer, None);
        }
        let messages = self.gathered.borrow_mut().take().unwrap_or_default();
        let text = (!messages.is_empty()).then(|| messages.join(" "));
        (answer, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_comment_and_the_snapshot_that_ended_meanwhile_are_said_together() {
        let announcer = Announcer::default();
        let (answer, text) = announcer.gather(|| {
            assert_eq!(announcer.say("Коментар збережено."), None);
            // The end of the snapshot, polled at the end of the command.
            assert_eq!(announcer.say("Знімок збережено: 5 записів."), None);
            7
        });
        assert_eq!(answer, 7);
        assert_eq!(
            text.as_deref(),
            Some("Коментар збережено. Знімок збережено: 5 записів.")
        );
    }

    #[test]
    fn one_message_is_said_as_before() {
        let announcer = Announcer::default();
        let ((), text) = announcer.gather(|| {
            announcer.say("Знімок збережено: 5 записів.");
        });
        assert_eq!(text.as_deref(), Some("Знімок збережено: 5 записів."));
        // The next event starts afresh.
        let ((), text) = announcer.gather(|| {
            announcer.say("Коментар прибрано.");
        });
        assert_eq!(text.as_deref(), Some("Коментар прибрано."));
    }

    #[test]
    fn an_event_that_says_nothing_announces_nothing() {
        let announcer = Announcer::default();
        let ((), text) = announcer.gather(|| {});
        assert_eq!(text, None);
    }

    #[test]
    fn between_events_a_message_is_shown_at_once() {
        let announcer = Announcer::default();
        assert_eq!(announcer.say("A").as_deref(), Some("A"));
        announcer.gather(|| {});
        assert_eq!(announcer.say("B").as_deref(), Some("B"));
    }

    #[test]
    fn an_event_inside_another_leaves_its_messages_to_the_outer_one() {
        let announcer = Announcer::default();
        let ((), text) = announcer.gather(|| {
            announcer.say("A");
            let ((), inner) = announcer.gather(|| {
                announcer.say("B");
            });
            assert_eq!(inner, None);
            announcer.say("C");
        });
        assert_eq!(text.as_deref(), Some("A B C"));
    }
}
