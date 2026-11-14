//! The options of revisor-cli (TASK-47): what a call asks for, or why it is wrong. lexopt
//! takes the arguments apart; which options each command has, and what their values may
//! be, is decided here, so the help and the errors are revisor's own, in both languages.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use lexopt::{Arg, Parser};
use revisor_core::settings::CsvDelimiter;

use revisor_ui::i18n::Lang;

// The commands and the ways a call is wrong are named by the strings, so they live beside
// them in revisor-ui.
pub use revisor_ui::cli::{Command, UsageError};

/// The format of the report `compare` writes (#4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportFormat {
    Text,
    Csv,
}

/// What a call asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// The help of the program, or of a command.
    Help(Option<Command>),
    Version,
    Snapshot(SnapshotArgs),
    Compare(CompareArgs),
}

/// What both commands take.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Common {
    pub profile: String,
    /// How long a busy profile is waited for; `None` waits as long as it takes (#10).
    pub timeout: Option<Duration>,
    /// A line of progress every so often (#9).
    pub progress: bool,
    /// Only errors (#9).
    pub quiet: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotArgs {
    pub common: Common,
    pub comment: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompareArgs {
    pub common: Common,
    pub output: PathBuf,
    /// The snapshot the latest one is compared with, rather than the one before it.
    pub with: Option<i64>,
    /// The format chosen with `--format`; without it the extension of the file decides.
    pub format: Option<ReportFormat>,
    /// The delimiter chosen with `--delimiter`; without it the setting decides.
    pub delimiter: Option<CsvDelimiter>,
}

impl CompareArgs {
    /// The format of the report (#4): the one `--format` chose, or CSV for a file whose
    /// extension is `.csv` in any case, and the text report for any other.
    pub fn report_format(&self) -> ReportFormat {
        self.format.unwrap_or_else(|| {
            let csv = self
                .output
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("csv"));
            if csv {
                ReportFormat::Csv
            } else {
                ReportFormat::Text
            }
        })
    }
}

/// A call parsed: the language it asked for, and what it asks for or why it is wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    /// The language of `--language`; English when the call does not name one (#5).
    pub lang: Lang,
    /// The command named, also when the call is wrong: its help is the one to point to.
    pub command: Option<Command>,
    pub action: Result<Action, UsageError>,
}

// The options as errors name them: by the long name.
const PROFILE: &str = "--profile";
const OUTPUT: &str = "--output";
const COMMENT: &str = "--comment";
const WITH: &str = "--with";
const FORMAT: &str = "--format";
const DELIMITER: &str = "--delimiter";
const LANGUAGE: &str = "--language";
const TIMEOUT: &str = "--timeout";
const PROGRESS: &str = "--progress";
const QUIET: &str = "--quiet";

// Values as they are typed, each with what it stands for.
const FORMATS: [(&str, ReportFormat); 2] =
    [("text", ReportFormat::Text), ("csv", ReportFormat::Csv)];
const DELIMITERS: [(&str, CsvDelimiter); 4] = [
    ("comma", CsvDelimiter::Comma),
    ("semicolon", CsvDelimiter::Semicolon),
    ("tab", CsvDelimiter::Tab),
    ("windows", CsvDelimiter::Windows),
];
const LANGUAGES: [(&str, Lang); 2] = [("en", Lang::En), ("uk", Lang::Uk)];

/// The options of a call as given, before they are checked against its command.
#[derive(Default)]
struct Given {
    command: Option<Command>,
    help: bool,
    version: bool,
    lang: Option<Lang>,
    profile: Option<String>,
    output: Option<PathBuf>,
    comment: Option<String>,
    with: Option<i64>,
    format: Option<ReportFormat>,
    delimiter: Option<CsvDelimiter>,
    timeout: Option<Duration>,
    progress: bool,
    quiet: bool,
    /// The options given, by their long names, in the order they came.
    named: Vec<&'static str>,
}

/// Parses the arguments of a call, without the name of the program.
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Call {
    let mut given = Given::default();
    let result = take(&mut given, args);
    let lang = given.lang.unwrap_or(Lang::En);
    let command = given.command;
    let action = result.and_then(|()| action(given));
    Call {
        lang,
        command,
        action,
    }
}

/// Takes the arguments apart into `given`, which keeps what came before an error: the
/// language and the command, which the error is told in and points to.
fn take(given: &mut Given, args: impl IntoIterator<Item = OsString>) -> Result<(), UsageError> {
    let mut parser = Parser::from_args(args);
    while let Some(arg) = parser.next().map_err(usage)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => given.help = true,
            Arg::Long("version") => given.version = true,
            Arg::Short('p') | Arg::Long("profile") => {
                let value = text(&mut parser, PROFILE)?;
                set(&mut given.profile, PROFILE, value, &mut given.named)?;
            }
            Arg::Short('o') | Arg::Long("output") => {
                let value = PathBuf::from(parser.value().map_err(usage)?);
                set(&mut given.output, OUTPUT, value, &mut given.named)?;
            }
            Arg::Long("comment") => {
                let value = text(&mut parser, COMMENT)?;
                set(&mut given.comment, COMMENT, value, &mut given.named)?;
            }
            Arg::Long("with") => {
                let value = text(&mut parser, WITH)?;
                let number = value
                    .parse::<i64>()
                    .ok()
                    .filter(|&n| n > 0)
                    .ok_or_else(|| invalid(WITH, &value))?;
                set(&mut given.with, WITH, number, &mut given.named)?;
            }
            Arg::Long("format") => {
                let value = one_of(&mut parser, FORMAT, &FORMATS)?;
                set(&mut given.format, FORMAT, value, &mut given.named)?;
            }
            Arg::Long("delimiter") => {
                let value = one_of(&mut parser, DELIMITER, &DELIMITERS)?;
                set(&mut given.delimiter, DELIMITER, value, &mut given.named)?;
            }
            Arg::Long("language") => {
                let value = one_of(&mut parser, LANGUAGE, &LANGUAGES)?;
                set(&mut given.lang, LANGUAGE, value, &mut given.named)?;
            }
            Arg::Long("timeout") => {
                let value = text(&mut parser, TIMEOUT)?;
                let seconds = value.parse::<u64>().map_err(|_| invalid(TIMEOUT, &value))?;
                let timeout = Duration::from_secs(seconds);
                set(&mut given.timeout, TIMEOUT, timeout, &mut given.named)?;
            }
            Arg::Long("progress") => flag(&mut given.progress, PROGRESS, &mut given.named)?,
            Arg::Short('q') | Arg::Long("quiet") => {
                flag(&mut given.quiet, QUIET, &mut given.named)?;
            }
            Arg::Value(value) if given.command.is_none() => {
                let name = value.to_string_lossy();
                given.command = Some(
                    Command::ALL
                        .into_iter()
                        .find(|command| command.name() == name)
                        .ok_or_else(|| UsageError::UnknownCommand(name.into_owned()))?,
                );
            }
            Arg::Value(value) => {
                return Err(UsageError::UnexpectedArgument(
                    value.to_string_lossy().into_owned(),
                ));
            }
            Arg::Short(c) => return Err(UsageError::UnknownOption(format!("-{c}"))),
            Arg::Long(name) => return Err(UsageError::UnknownOption(format!("--{name}"))),
        }
    }
    Ok(())
}

/// What the options given ask for, once they are all there.
fn action(given: Given) -> Result<Action, UsageError> {
    if given.help {
        return Ok(Action::Help(given.command));
    }
    if given.version {
        return Ok(Action::Version);
    }
    let Some(command) = given.command else {
        return Err(UsageError::NoCommand);
    };
    let foreign: &[&str] = match command {
        Command::Snapshot => &[OUTPUT, WITH, FORMAT, DELIMITER],
        Command::Compare => &[COMMENT],
    };
    if let Some(option) = given.named.iter().find(|named| foreign.contains(named)) {
        return Err(UsageError::NotForCommand {
            option: option.to_string(),
            command,
        });
    }
    if given.quiet && given.progress {
        return Err(UsageError::QuietWithProgress);
    }
    let profile = given
        .profile
        .ok_or_else(|| UsageError::Missing(PROFILE.into()))?;
    let common = Common {
        profile,
        timeout: given.timeout,
        progress: given.progress,
        quiet: given.quiet,
    };
    Ok(match command {
        Command::Snapshot => Action::Snapshot(SnapshotArgs {
            common,
            comment: given.comment,
        }),
        Command::Compare => Action::Compare(CompareArgs {
            common,
            output: given
                .output
                .ok_or_else(|| UsageError::Missing(OUTPUT.into()))?,
            with: given.with,
            format: given.format,
            delimiter: given.delimiter,
        }),
    })
}

/// Sets an option's value, which may be given once.
fn set<T>(
    slot: &mut Option<T>,
    option: &'static str,
    value: T,
    named: &mut Vec<&'static str>,
) -> Result<(), UsageError> {
    if slot.is_some() {
        return Err(UsageError::Repeated(option.into()));
    }
    *slot = Some(value);
    named.push(option);
    Ok(())
}

/// Sets a flag, which may be given once.
fn flag(
    slot: &mut bool,
    option: &'static str,
    named: &mut Vec<&'static str>,
) -> Result<(), UsageError> {
    if *slot {
        return Err(UsageError::Repeated(option.into()));
    }
    *slot = true;
    named.push(option);
    Ok(())
}

/// The value of `option` as text.
fn text(parser: &mut Parser, option: &str) -> Result<String, UsageError> {
    let value = parser.value().map_err(usage)?;
    value
        .into_string()
        .map_err(|value| invalid(option, &value.to_string_lossy()))
}

/// The value of `option`: one of `values` as typed, in any case.
fn one_of<T: Copy>(
    parser: &mut Parser,
    option: &str,
    values: &[(&str, T)],
) -> Result<T, UsageError> {
    let value = text(parser, option)?;
    values
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(&value))
        .map(|&(_, value)| value)
        .ok_or_else(|| invalid(option, &value))
}

fn invalid(option: &str, value: &str) -> UsageError {
    UsageError::InvalidValue {
        option: option.into(),
        value: value.into(),
    }
}

/// An error of lexopt as a wrong call.
fn usage(e: lexopt::Error) -> UsageError {
    match e {
        lexopt::Error::MissingValue { option } => {
            UsageError::MissingValue(option.unwrap_or_default())
        }
        lexopt::Error::UnexpectedOption(option) => UsageError::UnknownOption(option),
        lexopt::Error::UnexpectedValue { option, .. } => UsageError::UnexpectedValue(option),
        lexopt::Error::UnexpectedArgument(value) | lexopt::Error::NonUnicodeValue(value) => {
            UsageError::UnexpectedArgument(value.to_string_lossy().into_owned())
        }
        e => UsageError::UnexpectedArgument(e.to_string()),
    }
}
