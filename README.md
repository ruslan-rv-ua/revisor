# revisor

*English · [Українська](README.uk.md)*

Snapshots of chosen folders, and what changed between two of them. Take a snapshot before you
install, update or run a program and another one after it, and revisor lists the files that
program created, changed or deleted.

[![CI status](https://github.com/ruslan-rv-ua/revisor/actions/workflows/ci.yml/badge.svg)](https://github.com/ruslan-rv-ua/revisor/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/ruslan-rv-ua/revisor)](https://github.com/ruslan-rv-ua/revisor/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
![Windows 10 and 11, 64-bit](https://img.shields.io/badge/Windows-10%20%7C%2011%20x64-blue)

revisor is a portable Windows application, built for the keyboard and for screen readers
first.

## Contents

- [Features](#features)
- [Install](#install)
- [Keyboard](#keyboard)
- [Where the data lives](#where-the-data-lives)
- [What revisor deliberately does not do](#what-revisor-deliberately-does-not-do)
- [Building from source](#building-from-source)
- [License](#license)

## Features

The main scenario takes four steps, all from the keyboard:

1. Create a profile (`Ctrl+N`) with the folders to watch, such as `C:\Program Files`,
   `C:\ProgramData` and your user folder.
2. Take a snapshot (`F5`) before you install or run the program.
3. Install or run it.
4. Press `Ctrl+F5`: revisor takes a second snapshot and shows the difference.

- **Snapshots of what matters.** A profile names the folders to watch and the exclusions to
  leave out. A new profile starts with 63 default exclusions for caches, logs and temporary
  files, which you can edit. A snapshot records every file, folder and link with its dates,
  attributes, size and NTFS file ID, and the version of `.exe` and `.dll` files, without
  reading file contents or downloading files that are only in the cloud.
- **Compare any two.** The current snapshot with the previous one (`F7`), with any other
  snapshot of the profile (`Ctrl+F7`), or a new snapshot with the one before it in a single
  command (`Ctrl+F5`). The difference always runs from the earlier snapshot to the later one.
- **Read the difference.** A list of added, deleted, modified and inaccessible entries, with
  what changed in each and a summary above it, or the same entries as a tree of their folders
  (`Ctrl+T`), where a folder opens when you expand it. Filter it by type and by text, sort the
  list by any column, read the details of a change, before and after (`Alt+Enter`), or show it
  in File Explorer (`Ctrl+Enter`). A difference of a million rows reads without delay.
- **In the background.** Snapshots and comparisons show their progress and can be canceled,
  and the window keeps responding.
- **Nothing hidden by permissions.** An entry revisor cannot read is kept, marked inaccessible,
  with the reason. When permissions are the reason, revisor offers to restart as administrator
  with the same profile open, and as administrator it also reads folders closed even to
  administrators.
- **Accessibility first.** Native Windows controls throughout, tested with NVDA. Every command
  is in the menus with an access key, and the frequent ones have shortcuts. A command that
  cannot run now says why in the status bar, which screen readers read without moving the
  focus.
- **English and Ukrainian**, following Windows or chosen in Settings.
- **Light or dark**, following Windows or chosen in Settings; a contrast theme of Windows
  always keeps its colors.
- **Portable.** One executable without DLLs, and everything it saves lives in a `data` folder
  beside it.

## Install

### scoop

```powershell
scoop bucket add ruslan-rv-ua https://github.com/ruslan-rv-ua/scoop-bucket
```

```powershell
scoop install revisor
```

scoop puts `revisor` on your `PATH` and in the Start menu and keeps your data across updates
(see [Where the data lives](#where-the-data-lives)); `scoop update revisor` brings the newest
release.

### Direct download

Take `revisor-v<version>-x64.exe` and the `.sha256` file beside it from the
[releases page](https://github.com/ruslan-rv-ua/revisor/releases). There is no archive to unpack
and no installer to run: the `.exe` is the whole application. Put it in a folder you can write
to, not in `Program Files`, because revisor creates its `data` folder beside itself.

revisor is **not code-signed**, so the first time you run a downloaded copy, Windows SmartScreen
shows "Windows protected your PC": choose **More info**, then **Run anyway**. Where Smart App
Control is on, Windows may refuse to run an unsigned program at all. A copy installed through
scoop carries no mark of having come from the internet, so SmartScreen does not stop it.

The warning means that nobody has vouched for the file, not that the file is broken, so check
it yourself. The `.sha256` file holds the exact fingerprint of the release. Run this in
PowerShell, in the folder with both files:

```powershell
$exe = Get-Item .\revisor-v*-x64.exe
$want = ((Get-Content "$exe.sha256") -split ' ')[0]
$got = (Get-FileHash $exe -Algorithm SHA256).Hash
if ($want -and $got -and $want -eq $got) { 'MATCHES the published fingerprint' } else { 'DOES NOT MATCH - do not run it' }
```

It prints **MATCHES the published fingerprint** when the file is byte for byte the one that was
published. Anything else, including an error above the answer, means do not run it. With
several versions downloaded into the folder, name the file in the first line instead of
`revisor-v*-x64.exe`.

Each release file is built by GitHub Actions from the tagged source. The same run checks that
the file needs no Visual C++ runtime, is at most 10 MiB and names its version in its
properties, and only then writes the `.sha256` and publishes both.

## Keyboard

Everything is reachable the ordinary way, with `Tab`, the arrow keys and the menus: every menu
item has an access key, and its shortcut, if it has one, stands beside it. Shortcuts and access
keys are Latin letters and work in any keyboard layout. These are revisor's shortcuts; `F1`
lists them in the application too.

| Keys | What it does |
|---|---|
| `Ctrl+N` | Create a profile |
| `F5` | Take a snapshot of the open profile |
| `Ctrl+F5` | Snapshot and compare: take a snapshot and show its difference from the previous one |
| `F2` | Comment on the current snapshot |
| `Delete` | Delete the selected snapshots (in the snapshot list only) |
| `F7` | Compare the current snapshot with the previous one; `Enter` in the snapshot list does the same |
| `Ctrl+F7` | Compare the current snapshot with another one you choose |
| `Alt+Enter` | Show the details of the change in focus, before and after; `Enter` in the difference list or tree does the same |
| `Ctrl+Enter` | Show the change in File Explorer |
| `Ctrl+F` | Move to the Filter field; `Esc` there clears it |
| `Ctrl+1` to `Ctrl+4` | Show or hide added, deleted, modified and inaccessible entries |
| `Ctrl+T` | Show the difference as a tree, or as a list again |
| `F6`, `Shift+F6` | Move between the snapshot list and the difference list or tree |
| `Shift+F5`, `Shift+F7`, `Shift+F2` | Cancel the snapshot, the comparison, or a comment waiting for the profile database |
| `Shift+F10` or the Application key | Open the context menu of the list or tree in focus |
| `F1` | List the keyboard shortcuts |
| `Alt+F4` | Exit |

A command that cannot run now is unavailable in its menu, and its shortcut says why in the
status bar; NVDA reads that without moving the focus, and `NVDA+End` repeats it. Editing and
deleting a profile, Settings, Restart as administrator, sorting and About revisor are in the
menus only.

## Where the data lives

revisor is portable: everything it saves is in a `data` folder beside its executable, and no
setting moves it.

- `data\settings.db` holds the settings and the profile you had open last.
- `data\profiles\<profile name>.db` is one profile: its folders, exclusions and snapshots, all
  in one SQLite database. To copy or move a profile, copy its file. Characters a file name
  cannot hold become `_`, a number is added when the name is taken, and renaming a profile
  renames its file.

Restart as administrator uses the same `data` folder. To move revisor elsewhere, move the
executable and its `data` folder together.

**Under scoop**, revisor lives in `~\scoop\apps\revisor`, and scoop links its `data` folder to
`~\scoop\persist\revisor\data`. Updates and `scoop uninstall revisor` keep your profiles and
snapshots there; `scoop uninstall revisor --purge` deletes them.

## What revisor deliberately does not do

- **It snapshots files and folders, not the registry.** The registry may become another source
  of snapshots in a much later version.
- **It works on NTFS only.** Folders on FAT32 or exFAT flash drives, on a ReFS Dev Drive or on
  a network share cannot be added to a profile. The FAT driver does not support the way revisor
  reads folders, a FAT file ID can change, and FAT32 times shift by an hour when daylight
  saving time starts or ends, which would show changes that never happened. ReFS and network
  folders have not been tested.
- **It does not record everything about a file.** Alternate data streams, permissions, owners
  and last access times are not in a snapshot, and file contents are not read, except for the
  version of `.exe` and `.dll` files.
- **It does not freeze the disk.** A snapshot does not use shadow copies (VSS), so it is not
  instantaneous: take it once the program you are watching has finished.
- **It does not go inside links.** Junctions, symbolic links and mounted volumes are recorded
  with their targets, not followed.
- **It does not download cloud files.** A file that is only in OneDrive or another cloud is
  recorded without its version rather than downloaded to read it.
- **It never deletes snapshots on its own.** Only you delete them.
- **It does not run in the background.** There is no tray icon, no global shortcut and no
  scheduled snapshot: its shortcuts work only in its own window.
- **It is not signed**, by decision; see [Direct download](#direct-download).

Planned for later 0.x versions: export to a text report and CSV, content hashes, detection of
renames and moves, and a command line.

## Building from source

revisor is written in Rust on [wxDragon](https://github.com/AllenDang/wxDragon), which builds
wxWidgets from source as part of `cargo build`. You need, on Windows 10 or 11 (x64):

- [Rust](https://rustup.rs/) with the MSVC toolchain;
- Visual Studio Build Tools with the "Desktop development with C++" workload;
- [CMake](https://cmake.org/) and [Ninja](https://ninja-build.org/) on `PATH`;
- [LLVM](https://llvm.org/), with the `LIBCLANG_PATH` environment variable pointing at its
  `bin` folder (the one that holds `libclang.dll`).

Keep the clone at a short path such as `C:\dev\revisor`: a deep build folder breaks the
wxWidgets build on the Windows path length limit.

```powershell
git clone https://github.com/ruslan-rv-ua/revisor.git
cd revisor
cargo build --release
```

The first build compiles wxWidgets and takes a few minutes; later builds take seconds. The
result is a single executable, `target\release\revisor.exe`, with no DLLs beside it.

Run the tests with `cargo test --workspace`. What changed in each release is in
[CHANGELOG.md](CHANGELOG.md).

## License

[MIT](LICENSE). Copyright (c) 2026 Ruslan Iskov.
