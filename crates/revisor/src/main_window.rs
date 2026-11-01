//! The main window: the menu bar and context menus built from the command table, the
//! status bar that speaks without moving the focus, the key handler, and the two parts of
//! the window. The profile box and both lists are placeholders until TASK-9 and TASK-11
//! fill them.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wxdragon::prelude::*;

use crate::commands::{COMMANDS, CommandId, CommandKind, MenuId, Pane, command};
use crate::help;
use crate::i18n::Strings;
use crate::keyboard::{Control, Focus, Route, route};
use crate::keys::Shortcut;
use crate::menus::{
    context_label, context_menu, field_label, menu_label, submenu_label, top_menu_label,
    window_title,
};
use crate::state::{AppState, ChangeType, SortColumn};
use crate::win32;

/// The wx id of the first command; the others follow in table order.
const FIRST_COMMAND_ID: i32 = 1000;

/// The access key of the profile label. Labels of the main window do not take the letters
/// of the menus, which would no longer open while the focus is in the panel.
const PROFILE_ACCESS: u8 = b'R';

fn wx_id(id: CommandId) -> i32 {
    let index = COMMANDS
        .iter()
        .position(|c| c.id == id)
        .expect("every command id is in the table");
    FIRST_COMMAND_ID + index as i32
}

fn command_of(wx_id: i32) -> Option<CommandId> {
    let index = usize::try_from(wx_id - FIRST_COMMAND_ID).ok()?;
    COMMANDS.get(index).map(|c| c.id)
}

pub struct MainWindow {
    frame: Frame,
    s: &'static dyn Strings,
    state: RefCell<AppState>,
    menu_bar: MenuBar,
    status: StatusBar,
    profile: Choice,
    snapshots: ListCtrl,
    difference: ListCtrl,
}

impl MainWindow {
    pub fn open(s: &'static dyn Strings, state: AppState) -> Rc<Self> {
        let frame = Frame::builder()
            .with_title(&window_title(state.profile.as_deref()))
            .with_size(Size::new(900, 640))
            .build();
        set_frame_icon(&frame);

        frame.set_menu_bar(build_menu_bar(s));
        let menu_bar = frame.get_menu_bar().expect("the menu bar was just set");
        let status = StatusBar::builder(&frame).build();
        frame.set_existing_status_bar(Some(&status));
        win32::make_live_region(status.get_handle());

        let panel = Panel::builder(&frame).build();
        let profile_label = StaticText::builder(&panel)
            .with_label(&field_label(s, s.profile(), PROFILE_ACCESS))
            .build();
        let profile = Choice::builder(&panel).build();
        let snapshots = ListCtrl::builder(&panel)
            .with_style(ListCtrlStyle::Report)
            .build();
        let difference = ListCtrl::builder(&panel)
            .with_style(ListCtrlStyle::Report)
            .build();
        for (list, pane) in [
            (&snapshots, Pane::Snapshots),
            (&difference, Pane::Difference),
        ] {
            list.insert_column(0, s.pane(pane), ListColumnFormat::Left, 600);
            win32::set_accessible_name(list.get_handle(), s.pane(pane));
        }
        if state.demo {
            fill_demo(s, &state, &profile, &snapshots, &difference);
        }

        let profile_row = BoxSizer::builder(Orientation::Horizontal).build();
        profile_row.add(
            &profile_label,
            0,
            SizerFlag::AlignCenterVertical | SizerFlag::All,
            5,
        );
        profile_row.add(&profile, 1, SizerFlag::All, 5);
        let sizer = BoxSizer::builder(Orientation::Vertical).build();
        sizer.add_sizer(&profile_row, 0, SizerFlag::Expand, 0);
        sizer.add(&snapshots, 1, SizerFlag::Expand | SizerFlag::All, 5);
        sizer.add(&difference, 2, SizerFlag::Expand | SizerFlag::All, 5);
        panel.set_sizer(sizer, true);

        let window = Rc::new(Self {
            frame,
            s,
            state: RefCell::new(state),
            menu_bar,
            status,
            profile,
            snapshots,
            difference,
        });
        window.bind_events(&panel);
        win32::subclass_main_window(window.frame.get_handle());
        window.refresh_menus();
        window.frame.centre();
        window.frame.show(true);
        window
    }

    fn bind_events(self: &Rc<Self>, panel: &Panel) {
        let w = Rc::clone(self);
        self.frame.on_char_hook(move |data| {
            if let WindowEventData::Keyboard(key) = data {
                let code = key.get_key_code().unwrap_or_default();
                let shortcut =
                    Shortcut::from_wx(code, key.control_down(), key.shift_down(), key.alt_down());
                let routed = route(shortcut, w.focus(), &w.state.borrow(), &[PROFILE_ACCESS]);
                if routed == Route::Pass {
                    key.event.skip(true);
                } else {
                    key.event.skip(false);
                    w.apply(routed);
                }
            }
        });

        // A newly shown frame ends up with the focus on itself once Windows has activated
        // it; the snapshot list should have it. The first idle event comes after that.
        // Later activations are left to wx, which restores whatever had the focus last.
        let w = Rc::clone(self);
        let focus_pending = Cell::new(true);
        self.frame.on_idle(move |data| {
            if focus_pending.replace(false) {
                w.snapshots.set_focus();
            }
            if let WindowEventData::Idle(event) = data {
                event.event.skip(true);
            }
        });

        let w = Rc::clone(self);
        self.frame.on_menu(move |event| {
            if let Some(id) = command_of(event.get_id()) {
                w.run(id);
            }
        });
        let w = Rc::clone(self);
        self.frame.on_menu_opened(move |event| {
            w.refresh_menus();
            event.skip(true);
        });
        // Not skipped, so wxFrame does not put the item's help text in the status bar,
        // where it would overwrite the last message.
        self.frame.on_menu_highlighted(|event| event.skip(false));

        // A right click: the list under the mouse has the focus by then.
        let w = Rc::clone(self);
        panel.on_context_menu(move |event| {
            let focus = w.focus();
            match (focus.control, focus.part) {
                (Control::MainList, Some(pane)) => {
                    let at = event
                        .get_context_position()
                        .filter(|p| p.x != -1 || p.y != -1);
                    w.show_context_menu(pane, at.map(|p| (p.x, p.y)));
                }
                _ => event.skip(true),
            }
        });
    }

    /// Where the keyboard focus is, in the terms of `keyboard::route`.
    fn focus(&self) -> Focus {
        let focused = win32::focused_window();
        if focused == self.snapshots.get_handle() {
            Focus {
                part: Some(Pane::Snapshots),
                control: Control::MainList,
            }
        } else if focused == self.difference.get_handle() {
            Focus {
                part: Some(Pane::Difference),
                control: Control::MainList,
            }
        } else if focused == self.profile.get_handle() {
            Focus {
                part: Some(Pane::Snapshots),
                control: Control::Other,
            }
        } else if !focused.is_null() && win32::is_text_field(focused) {
            Focus {
                part: None,
                control: Control::TextEntry,
            }
        } else {
            Focus {
                part: None,
                control: Control::Other,
            }
        }
    }

    fn list(&self, pane: Pane) -> &ListCtrl {
        match pane {
            Pane::Snapshots => &self.snapshots,
            Pane::Difference => &self.difference,
        }
    }

    fn apply(&self, routed: Route) {
        let s = self.s;
        match routed {
            Route::Run(id) => self.run(id),
            Route::Refuse(id, reason) => self.say(&s.unavailable(s.command(id), s.reason(reason))),
            Route::OnlyIn(key, pane) => self.say(&s.key_only_in(&key.name(s), pane)),
            Route::FocusPane(pane) => self.list(pane).set_focus(),
            Route::FocusLabel(_) => self.profile.set_focus(),
            Route::ContextMenu(pane) => self.show_context_menu(pane, None),
            Route::Pass => {}
        }
    }

    /// Shows a message in the status bar; screen readers read it without the focus moving.
    pub fn say(&self, text: &str) {
        self.status.set_status_text(text, 0);
        win32::announce(self.status.get_handle(), text);
    }

    fn run(&self, id: CommandId) {
        let s = self.s;
        let available = (command(id).available)(&self.state.borrow());
        if let Err(reason) = available {
            self.say(&s.unavailable(s.command(id), s.reason(reason)));
            return;
        }
        match id {
            CommandId::Exit => {
                self.frame.close(false);
                return;
            }
            CommandId::KeyboardShortcuts => help::show_shortcuts(&self.frame, s),
            CommandId::About => help::show_about(&self.frame, s),
            // Only the demo state gets here: the other commands' tasks are not done yet.
            _ => {
                self.demo_run(id);
                self.say(&s.demo_done(s.command(id)));
            }
        }
        self.refresh_menus();
    }

    /// What the demo state does for a command: the filter and sort items keep their marks.
    fn demo_run(&self, id: CommandId) {
        let mut state = self.state.borrow_mut();
        match id {
            CommandId::ShowAdded => state.toggle_shown(ChangeType::Added),
            CommandId::ShowDeleted => state.toggle_shown(ChangeType::Deleted),
            CommandId::ShowModified => state.toggle_shown(ChangeType::Modified),
            CommandId::ShowInaccessible => state.toggle_shown(ChangeType::Inaccessible),
            CommandId::SortByPath => state.sort = SortColumn::Path,
            CommandId::SortByName => state.sort = SortColumn::Name,
            CommandId::SortByChange => state.sort = SortColumn::Change,
            CommandId::SortByWhatChanged => state.sort = SortColumn::WhatChanged,
            CommandId::SortBySize => state.sort = SortColumn::Size,
            CommandId::SortByDateModified => state.sort = SortColumn::DateModified,
            CommandId::SortByFolder => state.sort = SortColumn::Folder,
            CommandId::SortAscending => state.descending = false,
            CommandId::SortDescending => state.descending = true,
            _ => {}
        }
    }

    /// Greys out the commands that cannot run now and sets the check and radio marks.
    fn refresh_menus(&self) {
        let state = self.state.borrow();
        for c in COMMANDS {
            let id = wx_id(c.id);
            self.menu_bar.enable_item(id, (c.available)(&state).is_ok());
            if let Some(checked) = c.checked {
                let checked = checked(&state);
                // Checking a radio item clears the others of its group.
                if checked || c.kind == CommandKind::Check {
                    self.menu_bar.check_item(id, checked);
                }
            }
        }
    }

    /// Opens a list's context menu at `at` (screen coordinates), or by its focused row.
    fn show_context_menu(&self, pane: Pane, at: Option<(i32, i32)>) {
        let menu_items = context_menu(pane, &self.state.borrow());
        if menu_items.groups.is_empty() {
            self.say(self.s.no_actions());
            return;
        }
        let mut menu = Menu::builder().build();
        for (i, group) in menu_items.groups.iter().enumerate() {
            if i > 0 {
                menu.append_separator();
            }
            for &id in group {
                menu.append(
                    wx_id(id),
                    &context_label(self.s, command(id)),
                    "",
                    ItemKind::Normal,
                );
            }
        }
        let (x, y) = at.unwrap_or_else(|| win32::context_menu_point(self.list(pane).get_handle()));
        win32::bold_first_item_of_next_popup(menu_items.default.is_some());
        self.frame.popup_menu(&mut menu, Some(Point::new(x, y)));
        win32::bold_first_item_of_next_popup(false);
    }
}

fn build_menu_bar(s: &dyn Strings) -> MenuBar {
    let item_kind = |kind| match kind {
        CommandKind::Normal => ItemKind::Normal,
        CommandKind::Check => ItemKind::Check,
        CommandKind::Radio => ItemKind::Radio,
    };
    let mut bar = MenuBar::builder();
    for menu_id in MenuId::ALL {
        let menu = Menu::builder().build();
        let mut last_group = None;
        let mut submenus_done = Vec::new();
        for c in COMMANDS.iter().filter(|c| c.place.menu == menu_id) {
            if let Some((sub, _)) = c.place.submenu {
                if submenus_done.contains(&sub) {
                    continue;
                }
                submenus_done.push(sub);
            }
            if last_group.is_some_and(|g| g != c.place.group) {
                menu.append_separator();
            }
            last_group = Some(c.place.group);
            match c.place.submenu {
                None => {
                    menu.append(wx_id(c.id), &menu_label(s, c), "", item_kind(c.kind));
                }
                Some((sub, _)) => {
                    let submenu = Menu::builder().build();
                    let mut last_sub_group = None;
                    for item in COMMANDS
                        .iter()
                        .filter(|i| i.place.submenu.is_some_and(|(id, _)| id == sub))
                    {
                        let group = item.place.submenu.map(|(_, g)| g);
                        if last_sub_group.is_some_and(|g| g != group) {
                            submenu.append_separator();
                        }
                        last_sub_group = Some(group);
                        submenu.append(
                            wx_id(item.id),
                            &menu_label(s, item),
                            "",
                            item_kind(item.kind),
                        );
                    }
                    menu.append_submenu(submenu, &submenu_label(s, sub), "");
                }
            }
        }
        bar = bar.append(menu, &top_menu_label(s, menu_id));
    }
    bar.build()
}

/// Stand-in rows so the lists' keys and context menus can be checked with NVDA.
fn fill_demo(
    s: &dyn Strings,
    state: &AppState,
    profile: &Choice,
    snapshots: &ListCtrl,
    difference: &ListCtrl,
) {
    if let Some(name) = &state.profile {
        profile.append(name);
        profile.set_selection(0);
    }
    for n in 0..state.snapshots {
        snapshots.insert_item(n as i64, &s.demo_snapshot(n + 1), None);
    }
    for n in 0..3 {
        difference.insert_item(n as i64, &s.demo_change(n + 1), None);
    }
}

/// Gives the window the same icon Explorer shows for the exe. wxMSW does not adopt the
/// exe's icon resource for a frame, so the frame takes it from the SVG that app.ico is
/// generated from. A failure leaves the generic window icon, which is not worth failing over.
fn set_frame_icon(frame: &Frame) {
    const ICON: &[u8] = include_bytes!("../resources/icon.svg");
    if let Some(bitmap) = BitmapBundle::from_svg_data(ICON, Size::new(32, 32))
        .and_then(|bundle| bundle.get_bitmap_for(frame))
    {
        frame.set_icon(&bitmap);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_profile_label_takes_no_menu_letter() {
        assert!(MenuId::ALL.iter().all(|m| m.access() != PROFILE_ACCESS));
    }
}
