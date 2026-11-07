# Changelog

All notable changes to revisor, newest first, in the format of
[Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/). revisor follows
[Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).

A notable change adds its line under [Unreleased] together with the change itself. At a
release that section becomes the version's own, and the release page publishes it as its text.

## [Unreleased]

### Changed

- **Failures of «Show in File Explorer» and «Restart as administrator» open a window.**
  Before, they were said only in the status bar; now, like the failures of the other
  commands, they open a window that names the command. So does a failure to restart revisor
  after the interface language was changed in the settings.

### Fixed

- **A path exclusion with a name that is not valid Unicode leaves that folder out.** When
  the variable a pattern starts with, or the real location of its folder, held a name with
  an unpaired surrogate, the name was spoiled on the way and the exclusion left nothing
  out. Such names are rare: Windows allows them, but programs seldom make them.
- **A path exclusion whose folder is really on a network share matches.** A real location
  written with the `\\?\UNC\` prefix had an extra part, so it never matched the folder it
  named. revisor still takes snapshots of local NTFS folders only.

## [0.1.0] - 2026-11-07

The first release. revisor takes snapshots of chosen folders and shows what changed between
two of them: take one before you install, update or run a program and another after it, and
revisor lists the files that program created, changed or deleted. Everything works from the
keyboard and with a screen reader.

### Added

- **Profiles.** A profile names the folders to watch and the exclusions to leave out: name
  patterns such as `Code Cache`, which match at any depth, and path patterns, which may start
  with an environment variable such as `%TEMP%`. A new profile starts with 63 default
  exclusions for caches, logs and other files that change on their own. A profile, with its
  settings and snapshots, is one database file in `data\profiles`. Only folders on NTFS
  volumes can be added, and revisor says why another volume cannot.
- **Snapshots.** A snapshot records every file, folder and link in the profile's folders: the
  dates created and modified, the attributes, the size of a file, its NTFS file ID, and the
  file and product versions of `.exe` and `.dll` files. It reads no file contents beyond those
  versions, does not download files that are only in the cloud, and records junctions and
  symbolic links with their targets without going inside. Long paths are supported. An entry
  revisor cannot read is kept, marked inaccessible, with the reason.
- **Background work.** Snapshots and comparisons run in the background with progress, and
  `Shift+F5` and `Shift+F7` cancel them; the window keeps responding.
- **The snapshot list.** A profile's snapshots from the oldest to the newest, named by date
  and time, each with an optional comment (`F2`). A new snapshot takes the focus if you were
  waiting for it in the list. Snapshots can be deleted (`Delete`) after a confirmation that can
  be turned off, and the profile database is then compacted.
- **Restart as administrator.** When entries are inaccessible because of permissions, revisor
  says how many and offers to restart as administrator with the same profile open. The File
  menu does the same beforehand, so the snapshots before and after are taken with the same
  rights. As administrator, a snapshot also reads folders closed even to administrators.
- **Comparisons.** Compare the current snapshot with the previous one (`F7`, or `Enter` in the
  snapshot list) or with any other snapshot of the profile (`Ctrl+F7`). The difference always
  runs from the earlier snapshot to the later one. When the two cover different folders or
  exclusions, only what both cover is compared, and revisor warns about it.
- **Snapshot and compare** (`Ctrl+F5`) takes a new snapshot and shows its difference from the
  one before it.
- **The difference list.** A row for every added, deleted, modified or inaccessible entry, what
  changed in it, and a summary above the list. Filter the list by type (`Ctrl+1` to `Ctrl+4`)
  and by text in the path (`Ctrl+F`), and sort it by any column. A difference of a million
  rows reads without delay.
- **Details and File Explorer.** `Alt+Enter`, or `Enter` in the difference list, shows every
  property of the change, before and after. `Ctrl+Enter` opens File Explorer with the entry
  selected.
- **Keyboard and screen readers.** Every command is in the menus with an access key, and the
  frequent ones have shortcuts that work in any keyboard layout; `F1` lists them. A command
  that cannot run now is unavailable in the menu, and its shortcut says why in the status bar,
  which screen readers read without moving the focus. `F6` moves between the snapshots and the
  difference. The window is built from native Windows controls and tested with NVDA.
- **English and Ukrainian.** The interface is in Ukrainian when Windows is and in English
  otherwise, or in the language chosen in Settings.
- **Settings** for where the focus goes after a snapshot and after a comparison, how sizes and
  attributes are shown, whether deleting snapshots asks first, and whether to warn before File
  Explorer opens the folder of an entry that is no longer on disk.
- **Portable.** One executable without DLLs, and everything it writes lives in a `data` folder
  beside it. About revisor shows the licenses of the components built into it.
- **Error messages** name the causes revisor recognizes in its own words—what happened, why,
  and what to do—and give the Windows or SQLite text and its code for the rest.

[Unreleased]: https://github.com/ruslan-rv-ua/revisor/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ruslan-rv-ua/revisor/releases/tag/v0.1.0
