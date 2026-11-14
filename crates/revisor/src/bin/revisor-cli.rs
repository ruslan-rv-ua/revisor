//! revisor-cli.exe, the command line of revisor (TASK-47, ADR-0015): a console program, so
//! cmd, .bat files and PowerShell wait for it and get its exit code.

fn main() -> std::process::ExitCode {
    revisor::cli::run()
}
