# Changelog

All notable changes to revisor, newest first, in the format of
[Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/). revisor follows
[Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).

A notable change adds its line under [Unreleased] together with the change itself. At a
release that section becomes the version's own, and the release page publishes it as its text.

## [Unreleased]

### Added

- **The difference recognises renamed and moved entries.** A file, folder or link renamed or
  moved within its volume, also between the folders of a profile, is one entry at its new
  path instead of one deleted and one added. The Change column says Renamed, Moved, or Moved
  and renamed, and What changed says where the entry was, then what else changed in it. A
  renamed or moved folder is one entry, followed only by what changed inside it. revisor
  knows an entry by its NTFS file ID, kind and creation time, which every snapshot since 0.1
  holds, so earlier snapshots compare this way too. The summary counts these entries apart,
  the Filter field also finds an entry by the path it had, and the details show the path
  before and after.
- **A filter for renamed or moved entries.** The check box Renamed or moved and the same item
  of the Difference menu, with `Ctrl+4`, show or hide them. They are shown after the update
  whatever other types were hidden before.

### Changed

- **Settings are on tabs**: General (language and theme), Snapshots, and Difference.
  `Ctrl+Tab` or `Ctrl+PgDn` switch them, and NVDA names the tab it comes to. An access key
  works on the tab shown, and OK keeps the settings of every tab.
- **`Ctrl+5` shows or hides inaccessible entries**, which `Ctrl+4` did: the shortcuts follow
  the check boxes, and Renamed or moved is the fourth of them now.
- **A name that changed in case only is a rename.** An entry whose name differs only in the
  case of its letters shows as Renamed, from its earlier name; it used to show as Modified.
- **A new file in the place of one that moved away.** Where the file at a path is another one
  now and the earlier one is found at another path, the old place still shows as Replaced, and
  the file that left shows as renamed or moved. A folder made in the place of a renamed one
  shows as Folder replaced, with all it holds as added.

## [0.3.0] - 2026-11-10

### Added

- **A profile can take in the typical folders programs write to at once.** Add typical
  folders in the profile window lists Program Files, Program Files (x86), ProgramData, the
  user's AppData, Windows, the user's desktop and the public desktop, with their paths on this
  computer, all checked. The checked ones join the profile's folders as plain paths; a folder
  the profile already has or covers is not listed, and a typical folder replaces the profile's
  folders inside it without asking.

### Changed

- **The difference tree shows a big folder a thousand entries at a time.** A folder of more
  than 1,000 entries shows them in pages of 1,000, with a node that says how many more there
  are; `Enter` on it shows the next page, or the previous one. A folder of tens of thousands
  of files no longer stops the window for many seconds while a screen reader stays silent.

### Fixed

- **A screen reader says the focused node of a new difference tree once.** When a comparison
  showed its difference as a tree, or the first `Ctrl+T` after it switched to the tree, NVDA
  said the focused node twice.
- **A screen reader says the tree node once after `Ctrl+T` from the list onto another entry.**
  When the difference had been shown as a tree before and the list moved to another row, NVDA
  said the node twice. The folders that were open in the tree stay open.

## [0.2.0] - 2026-11-09

### Added

- **The difference can be viewed as a tree of its folders.** `Ctrl+T`, or List and Tree in
  the Difference menu, switch between the list and the tree in the same place, with the focus
  on the same entry, and revisor keeps the view for its next start. The tree shows only the
  folders with changes, by name; a folder gets its contents when you first expand it, so even
  a difference of a million rows opens at once, expanded to its first change. A node says its
  change and what changed, and a folder how many changes it holds; the new setting Tree nodes
  shortens that to the name and the change. The filter and the type check boxes work in the
  tree too, and Details and Show in File Explorer act on a node as on a row: for a folder that
  did not change itself, Details gives its path and the summary of its contents, and File
  Explorer opens it. Sorting stays with the list.
- **A dark theme.** The new setting Theme shows revisor as Windows has it, which is the
  default, light or dark. A new theme shows at once, without a restart, and with Same as
  Windows revisor follows Windows as it switches. While a contrast theme of Windows is on,
  revisor keeps its colors whatever the setting says. NVDA reads every window the same in
  either theme. In the dark theme an empty list does not show its hint, which NVDA still reads
  as the list's name, and the selected node of the tree is faint while the tree does not have
  the focus; a contrast theme switched on while revisor is dark leaves the column headers
  faint until revisor is restarted.

### Changed

- **Failures of «Show in File Explorer» and «Restart as administrator» open a window.**
  Before, they were said only in the status bar; now, like the failures of the other
  commands, they open a window that names the command. So does a failure to restart revisor
  after the interface language was changed in the settings.
- **A button or check box pressed with Alt and its letter acts once Alt is released.**
  This now holds in every window: the main window's Cancel and type check boxes, the
  profile window's Add folder and Remove folder, and the check boxes of the settings. Before,
  only the buttons of questions waited for it. The keys pressed next are no longer lost in
  the window menu. A field or list still takes the focus at once.

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

[Unreleased]: https://github.com/ruslan-rv-ua/revisor/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/ruslan-rv-ua/revisor/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/ruslan-rv-ua/revisor/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/ruslan-rv-ua/revisor/releases/tag/v0.1.0
