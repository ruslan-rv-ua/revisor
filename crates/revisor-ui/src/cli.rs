//! What the strings say of a call of revisor-cli (TASK-47): its commands, and the ways a
//! call is wrong. The options themselves are taken apart in revisor-cli; the strings name
//! these, so they live beside the strings.

/// A command of revisor-cli.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Snapshot,
    Compare,
}

impl Command {
    pub const ALL: [Self; 2] = [Self::Snapshot, Self::Compare];

    /// The command as it is typed.
    pub fn name(self) -> &'static str {
        match self {
            Command::Snapshot => "snapshot",
            Command::Compare => "compare",
        }
    }
}

/// Why a call is wrong (#7, #8): revisor-cli says so and ends with exit code 2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UsageError {
    NoCommand,
    UnknownCommand(String),
    UnknownOption(String),
    /// An option of the other command.
    NotForCommand {
        option: String,
        command: Command,
    },
    MissingValue(String),
    /// A value given to an option that takes none: `--quiet=yes`.
    UnexpectedValue(String),
    UnexpectedArgument(String),
    Repeated(String),
    InvalidValue {
        option: String,
        value: String,
    },
    /// A required option is not there.
    Missing(String),
    QuietWithProgress,
}
