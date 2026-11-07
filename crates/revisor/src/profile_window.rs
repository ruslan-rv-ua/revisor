//! The profile window: the name, folders and exclusions of a new or existing profile
//! (TASK-7). A folder is checked as soon as it is added; the rest when the window is
//! confirmed. The window stays open, with everything typed in it, until the profile is
//! saved or the user cancels.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use revisor_core::exclude::parse_lines;
use revisor_core::folders::{Addition, LocateError, ProfileFolder, check_addition, locate};
use revisor_core::profiles::ProfileFile;
use revisor_core::store::{ProfileSettings, Scope};
use wxdragon::prelude::*;

use crate::commands::CommandId;
use crate::failure;
use crate::i18n::Strings;
use crate::keys::Shortcut;
use crate::menus::{button_label, field_label};
use crate::message_box::{confirm, tell};

/// The access key of «Замінити» in the question whether a new folder replaces the
/// profile's folders inside it.
const REPLACE_ACCESS: u8 = b'R';

/// What the window is opened for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    New,
    Edit,
}

/// The labelled controls of the window. OK and Cancel are not among them: they have no
/// access keys, as Enter and Esc press them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Control {
    Name,
    Folders,
    AddFolder,
    RemoveFolder,
    Exclusions,
}

impl Control {
    const ALL: [Control; 5] = [
        Self::Name,
        Self::Folders,
        Self::AddFolder,
        Self::RemoveFolder,
        Self::Exclusions,
    ];

    /// The access key, as the document «Клавіатура й меню» lists it.
    const fn access(self) -> u8 {
        match self {
            Self::Name => b'N',
            Self::Folders => b'F',
            Self::AddFolder => b'A',
            Self::RemoveFolder => b'R',
            Self::Exclusions => b'E',
        }
    }

    fn name(self, s: &dyn Strings) -> &'static str {
        match self {
            Self::Name => s.profile_name(),
            Self::Folders => s.profile_folders(),
            Self::AddFolder => s.add_folder(),
            Self::RemoveFolder => s.remove_folder(),
            Self::Exclusions => s.profile_exclusions(),
        }
    }

    /// The text of the control's label or button: "&Name:", "Додати теку (&A)…".
    fn label(self, s: &dyn Strings) -> String {
        let (name, access) = (self.name(s), self.access());
        match self {
            Self::AddFolder => button_label(s, name, access, true),
            Self::RemoveFolder => button_label(s, name, access, false),
            Self::Name | Self::Folders | Self::Exclusions => field_label(s, name, access),
        }
    }
}

/// Writes the settings of the profile and gives its file, or the message to show when it
/// cannot.
type Save = dyn Fn(&ProfileSettings) -> Result<ProfileFile, String>;

struct Window {
    dialog: Dialog,
    s: &'static dyn Strings,
    title: &'static str,
    name: TextCtrl,
    folders: ListBox,
    add: Button,
    remove: Button,
    exclusions: TextCtrl,
    entries: RefCell<Vec<ProfileFolder>>,
    taken: Box<dyn Fn(&str) -> bool>,
    save: Box<Save>,
    saved: RefCell<Option<ProfileFile>>,
}

/// Shows the window with the `initial` settings and returns the profile file once it is
/// saved, or `None` when the user cancels. `taken` says whether another profile has a
/// name already; `save` writes the settings, or returns the message to show when it
/// cannot.
pub fn show(
    parent: &Frame,
    s: &'static dyn Strings,
    purpose: Purpose,
    initial: &ProfileSettings,
    taken: impl Fn(&str) -> bool + 'static,
    save: impl Fn(&ProfileSettings) -> Result<ProfileFile, String> + 'static,
) -> Option<ProfileFile> {
    let (title, commit) = match purpose {
        Purpose::New => (s.command(CommandId::NewProfile), s.create()),
        Purpose::Edit => (s.command(CommandId::EditProfile), s.ok()),
    };
    let dialog = Dialog::builder(parent, title)
        .with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder)
        .with_size(640, 560)
        .build();
    // Made in tab order. Each label comes right before its control, so screen readers name
    // the control after it.
    let name_label = StaticText::builder(&dialog)
        .with_label(&Control::Name.label(s))
        .build();
    let name = TextCtrl::builder(&dialog).build();
    let folders_label = StaticText::builder(&dialog)
        .with_label(&Control::Folders.label(s))
        .build();
    let folders = ListBox::builder(&dialog).build();
    let add = Button::builder(&dialog)
        .with_label(&Control::AddFolder.label(s))
        .build();
    let remove = Button::builder(&dialog)
        .with_label(&Control::RemoveFolder.label(s))
        .build();
    let exclusions_label = StaticText::builder(&dialog)
        .with_label(&Control::Exclusions.label(s))
        .build();
    let exclusions = TextCtrl::builder(&dialog)
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::DontWrap)
        .build();
    let ok = Button::builder(&dialog)
        .with_id(ID_OK)
        .with_label(commit)
        .build();
    let cancel = Button::builder(&dialog)
        .with_id(ID_CANCEL)
        .with_label(s.cancel())
        .build();
    ok.set_default();
    dialog.set_escape_id(ID_CANCEL);

    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    let side = SizerFlag::Left | SizerFlag::Right;
    sizer.add(&name_label, 0, side | SizerFlag::Top, 8);
    sizer.add(&name, 0, SizerFlag::Expand | SizerFlag::All, 8);
    sizer.add(&folders_label, 0, side, 8);
    let folders_row = BoxSizer::builder(Orientation::Horizontal).build();
    folders_row.add(&folders, 1, SizerFlag::Expand | SizerFlag::All, 8);
    let folder_buttons = BoxSizer::builder(Orientation::Vertical).build();
    folder_buttons.add(&add, 0, SizerFlag::Expand | SizerFlag::Bottom, 4);
    folder_buttons.add(&remove, 0, SizerFlag::Expand, 0);
    folders_row.add_sizer(&folder_buttons, 0, SizerFlag::Top | SizerFlag::Right, 8);
    sizer.add_sizer(&folders_row, 1, SizerFlag::Expand, 0);
    sizer.add(&exclusions_label, 0, side, 8);
    sizer.add(&exclusions, 2, SizerFlag::Expand | SizerFlag::All, 8);
    let commit_row = BoxSizer::builder(Orientation::Horizontal).build();
    commit_row.add(&ok, 0, SizerFlag::Right, 8);
    commit_row.add(&cancel, 0, SizerFlag::AlignLeft, 0);
    sizer.add_sizer(&commit_row, 0, SizerFlag::AlignRight | SizerFlag::All, 8);
    dialog.set_sizer(sizer, true);

    name.set_value(&initial.name);
    exclusions.set_value(&initial.scope.exclusions.join("\n"));
    let entries = initial
        .scope
        .folders
        .iter()
        .map(|path| ProfileFolder::new(path.clone()))
        .collect();
    let window = Rc::new(Window {
        dialog,
        s,
        title,
        name,
        folders,
        add,
        remove,
        exclusions,
        entries: RefCell::new(entries),
        taken: Box::new(taken),
        save: Box::new(save),
        saved: RefCell::new(None),
    });
    window.show_folders(Some(0));
    window.bind_events(&ok);

    window.dialog.centre();
    // Selected, so typing replaces the name, as in a rename box.
    window.name.set_focus();
    window.name.select_all();
    window.dialog.show_modal();
    // A plain wxDragon Dialog is not destroyed when dropped; its handlers, which hold the
    // window, go with it.
    window.dialog.destroy();
    window.saved.borrow_mut().take()
}

impl Window {
    fn bind_events(self: &Rc<Self>, ok: &Button) {
        let w = Rc::clone(self);
        self.add.on_click(move |_| w.add_folder());
        let w = Rc::clone(self);
        self.remove.on_click(move |_| w.remove_folder());
        let w = Rc::clone(self);
        ok.on_click(move |event| {
            // Not skipped: the dialog closes only once the profile is saved.
            event.event.skip(false);
            w.commit();
        });
        // Access keys by the key's position, so they work in the Ukrainian layout too
        // (document «Клавіатура й меню»).
        let w = Rc::clone(self);
        self.dialog.on_char_hook(move |data| {
            if let WindowEventData::Keyboard(key) = data {
                let handled = Shortcut::from_event(&key)
                    .access_letter()
                    .is_some_and(|letter| w.access_key(letter));
                key.event.skip(!handled);
            }
        });
    }

    /// Acts on the access key `letter`; returns whether it is one of the window's.
    fn access_key(&self, letter: u8) -> bool {
        let Some(control) = Control::ALL.into_iter().find(|c| c.access() == letter) else {
            return false;
        };
        match control {
            Control::Name => self.name.set_focus(),
            Control::Folders => self.folders.set_focus(),
            Control::AddFolder => self.add_folder(),
            Control::RemoveFolder if self.remove.is_enabled() => self.remove_folder(),
            Control::RemoveFolder => {}
            Control::Exclusions => self.exclusions.set_focus(),
        }
        true
    }

    /// Fills the folder list, selects the folder at `selected` if there is one, and lets
    /// "Remove folder" act only when there is a folder to remove.
    fn show_folders(&self, selected: Option<usize>) {
        let entries = self.entries.borrow();
        self.folders.clear();
        for folder in entries.iter() {
            self.folders.append(&folder.path.to_string_lossy());
        }
        if let Some(i) = selected.filter(|&i| i < entries.len()) {
            self.folders.set_selection(i as u32, true);
        }
        self.remove.enable(!entries.is_empty());
    }

    fn add_folder(&self) {
        let s = self.s;
        let start = self
            .entries
            .borrow()
            .last()
            .and_then(|f| f.path.parent().map(Path::to_path_buf))
            .unwrap_or_default();
        let picker = DirDialog::builder(&self.dialog, s.choose_folder(), &start.to_string_lossy())
            .with_style(DirDialogStyle::Default.bits() | DirDialogStyle::MustExist.bits())
            .build();
        let chosen = (picker.show_modal() == ID_OK)
            .then(|| picker.get_path())
            .flatten();
        picker.destroy();
        let Some(path) = chosen.map(PathBuf::from) else {
            return;
        };
        let shown = path.to_string_lossy().into_owned();
        let location = match locate(&path) {
            Ok(location) => location,
            Err(e) => {
                let text = match e {
                    LocateError::NotNtfs(fs) => s.folder_not_ntfs(&shown, &fs),
                    LocateError::Network => s.folder_on_network(&shown),
                    LocateError::Io(e) => s.folder_not_examined(&shown, &failure::reason(s, &e)),
                };
                tell(&self.dialog, s, self.title, &text);
                return;
            }
        };
        let addition = check_addition(&self.entries.borrow(), &location);
        match addition {
            Addition::Covered(i) => {
                let by = self.entries.borrow()[i].path.to_string_lossy().into_owned();
                tell(&self.dialog, s, self.title, &s.folder_covered(&shown, &by));
                return;
            }
            Addition::Replaces(inside) => {
                let names: Vec<String> = inside
                    .iter()
                    .map(|&i| self.entries.borrow()[i].path.to_string_lossy().into_owned())
                    .collect();
                let question = s.replace_folders(&shown, &names);
                let replace = (s.replace(), REPLACE_ACCESS);
                if !confirm(&self.dialog, s, self.title, &question, replace, false) {
                    return;
                }
                let mut entries = self.entries.borrow_mut();
                for &i in inside.iter().rev() {
                    entries.remove(i);
                }
            }
            Addition::New => {}
        }
        let added = {
            let mut entries = self.entries.borrow_mut();
            entries.push(ProfileFolder {
                path,
                location: Some(location),
            });
            entries.len() - 1
        };
        self.show_folders(Some(added));
        self.folders.set_focus();
    }

    fn remove_folder(&self) {
        let Some(selected) = self.folders.get_selection().map(|i| i as usize) else {
            return;
        };
        let left = {
            let mut entries = self.entries.borrow_mut();
            if selected >= entries.len() {
                return;
            }
            entries.remove(selected);
            entries.len()
        };
        // The folder after the removed one takes its place, or the one before it at the end.
        self.show_folders(Some(selected.min(left.saturating_sub(1))));
        if left == 0 {
            self.add.set_focus();
        } else {
            self.folders.set_focus();
        }
    }

    /// Checks what was typed and saves the profile; on a problem, says what it is, puts the
    /// focus where it can be fixed and leaves the window open.
    fn commit(&self) {
        let s = self.s;
        let name = self.name.get_value().trim().to_string();
        if name.is_empty() {
            tell(&self.dialog, s, self.title, s.name_missing());
            self.name.set_focus();
            return;
        }
        if (self.taken)(&name) {
            tell(&self.dialog, s, self.title, &s.name_taken(&name));
            self.name.set_focus();
            self.name.select_all();
            return;
        }
        if self.entries.borrow().is_empty() {
            tell(&self.dialog, s, self.title, s.no_folders());
            self.add.set_focus();
            return;
        }
        let exclusions = match parse_lines(&self.exclusions.get_value()) {
            Ok(exclusions) => exclusions,
            Err(e) => {
                let text = s.bad_pattern(e.line, &e.pattern, e.error);
                tell(&self.dialog, s, self.title, &text);
                self.select_line(e.line - 1);
                return;
            }
        };
        let settings = ProfileSettings {
            name,
            scope: Scope {
                folders: self
                    .entries
                    .borrow()
                    .iter()
                    .map(|f| f.path.clone())
                    .collect(),
                exclusions,
            },
        };
        match (self.save)(&settings) {
            Ok(file) => {
                *self.saved.borrow_mut() = Some(file);
                self.dialog.end_modal(ID_OK);
            }
            Err(message) => tell(&self.dialog, s, self.title, &message),
        }
    }

    /// Puts the focus on a line of the exclusions and selects it.
    fn select_line(&self, line: usize) {
        let field = &self.exclusions;
        field.set_focus();
        let start = field.xy_to_position(0, line as i64);
        let length = i64::from(field.get_line_length(line as i64).max(0));
        field.set_selection(start, start + length);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{En, Uk};

    #[test]
    fn each_labelled_control_has_its_own_latin_access_key() {
        for (i, control) in Control::ALL.into_iter().enumerate() {
            let letter = char::from(control.access());
            assert!(
                control.name(&En).to_ascii_uppercase().contains(letter),
                "{control:?}"
            );
            assert!(
                Control::ALL[..i]
                    .iter()
                    .all(|c| c.access() != control.access()),
                "{control:?}"
            );
        }
        assert!(
            En.replace()
                .to_ascii_uppercase()
                .contains(char::from(REPLACE_ACCESS))
        );
        assert_eq!(Control::Name.label(&En), "&Name:");
        assert_eq!(Control::Name.label(&Uk), "Назва (&N):");
        assert_eq!(Control::AddFolder.label(&En), "&Add folder…");
        assert_eq!(Control::AddFolder.label(&Uk), "Додати теку (&A)…");
        assert_eq!(Control::RemoveFolder.label(&Uk), "Прибрати теку (&R)");
        assert_eq!(Control::Exclusions.label(&Uk), "Винятки (&E):");
    }
}
