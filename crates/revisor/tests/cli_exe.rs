//! revisor-cli.exe as it is built (TASK-47 #1): a console program, which cmd and
//! PowerShell wait for and get the exit code of. A program for the GUI subsystem would be
//! started and left behind, with no exit code for the script.

use std::fs;
use std::process::{Command, Output};

const EXE: &str = env!("CARGO_BIN_EXE_revisor-cli");

/// `IMAGE_SUBSYSTEM_WINDOWS_CUI`: the console subsystem.
const CONSOLE: u16 = 3;

/// The subsystem in the optional header of the PE file at `path`.
fn subsystem(path: &str) -> u16 {
    let image = fs::read(path).unwrap();
    let at = |offset: usize, len: usize| &image[offset..offset + len];
    let pe = u32::from_le_bytes(at(0x3C, 4).try_into().unwrap()) as usize;
    assert_eq!(at(pe, 4), b"PE\0\0", "not a PE file");
    // The COFF header is 20 bytes after the signature; Subsystem is at 68 in the optional
    // header, in PE32 and PE32+ alike.
    u16::from_le_bytes(at(pe + 4 + 20 + 68, 2).try_into().unwrap())
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[test]
fn revisor_cli_is_a_console_program() {
    assert_eq!(subsystem(EXE), CONSOLE);
}

#[test]
fn the_version_goes_to_the_output_with_exit_code_0() {
    let output = Command::new(EXE).arg("--version").output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        text(&output.stdout),
        format!("revisor-cli {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(output.stderr.is_empty());
}

fn wrong_call(output: &Output) {
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(
        text(&output.stderr).contains("revisor-cli: unknown option “--bogus”."),
        "{output:?}"
    );
}

#[test]
fn a_wrong_call_ends_with_exit_code_2() {
    wrong_call(&Command::new(EXE).arg("--bogus").output().unwrap());
}

#[test]
fn cmd_waits_for_revisor_cli_and_gets_its_exit_code() {
    // A .bat file runs its commands as cmd /c does.
    let output = Command::new("cmd")
        .args(["/d", "/c"])
        .arg(EXE)
        .arg("--bogus")
        .output()
        .unwrap();
    wrong_call(&output);
}

#[test]
fn powershell_waits_for_revisor_cli_and_gets_its_exit_code() {
    let script = format!("& '{EXE}' --bogus; exit $LASTEXITCODE");
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "{output:?}");
}
