//! The columns of the difference list as the user chose them (TASK-49): which of the six
//! it shows, in which order, and how wide each is. «Ім'я» is always shown, but it moves
//! like the others.

use super::SortColumn;

/// A column of the difference list, in the order it shows them by default (TASK-13 #12):
/// the name first and the longest, the folder, last, as screen readers read a row column
/// by column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DifferenceColumn {
    Name,
    Change,
    WhatChanged,
    Size,
    DateModified,
    Folder,
}

impl DifferenceColumn {
    /// Every column, in the default order.
    pub const ALL: [Self; 6] = [
        Self::Name,
        Self::Change,
        Self::WhatChanged,
        Self::Size,
        Self::DateModified,
        Self::Folder,
    ];

    /// The value as the database stores it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Change => "change",
            Self::WhatChanged => "what_changed",
            Self::Size => "size",
            Self::DateModified => "date_modified",
            Self::Folder => "folder",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == text)
    }

    /// What sorting by this column is.
    pub fn sort(self) -> SortColumn {
        match self {
            Self::Name => SortColumn::Name,
            Self::Change => SortColumn::Change,
            Self::WhatChanged => SortColumn::WhatChanged,
            Self::Size => SortColumn::Size,
            Self::DateModified => SortColumn::DateModified,
            Self::Folder => SortColumn::Folder,
        }
    }

    /// The width the column has until the user drags it, in pixels at 96 DPI.
    pub fn default_width(self) -> i32 {
        match self {
            Self::Name => 220,
            Self::Change => 140,
            Self::WhatChanged => 200,
            Self::Size => 90,
            Self::DateModified => 160,
            Self::Folder => 420,
        }
    }

    /// Whether the user may hide the column: all but «Ім'я» (TASK-49 #3).
    pub fn can_hide(self) -> bool {
        self != Self::Name
    }
}

/// One column as the user chose it: whether it is shown, and how wide it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColumnChoice {
    pub column: DifferenceColumn,
    pub shown: bool,
    pub width: i32,
}

impl ColumnChoice {
    fn default_of(column: DifferenceColumn) -> Self {
        Self {
            column,
            shown: true,
            width: column.default_width(),
        }
    }
}

/// The six columns of the difference list in the order the list shows them, each shown
/// or hidden, with its width; hidden columns keep their place in the order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DifferenceColumns([ColumnChoice; 6]);

impl Default for DifferenceColumns {
    /// All six, in their default order and widths (TASK-49 #1, #5).
    fn default() -> Self {
        Self(DifferenceColumn::ALL.map(ColumnChoice::default_of))
    }
}

impl DifferenceColumns {
    /// The columns in their order, hidden ones too.
    pub fn choices(&self) -> &[ColumnChoice; 6] {
        &self.0
    }

    /// The columns the list shows, in the order it shows them.
    pub fn shown(&self) -> Vec<DifferenceColumn> {
        self.0
            .iter()
            .filter(|c| c.shown)
            .map(|c| c.column)
            .collect()
    }

    /// Whether `other` shows the same columns in the same order, whatever their widths.
    pub fn same_layout(&self, other: &Self) -> bool {
        self.shown() == other.shown()
    }

    /// Shows or hides the column at `index` of the order; «Ім'я» stays shown.
    pub fn set_shown(&mut self, index: usize, shown: bool) {
        if let Some(choice) = self.0.get_mut(index) {
            choice.shown = shown || !choice.column.can_hide();
        }
    }

    /// The width of `column`.
    pub fn width(&self, column: DifferenceColumn) -> i32 {
        self.0
            .iter()
            .find(|c| c.column == column)
            .map_or(column.default_width(), |c| c.width)
    }

    /// Sets the width of `column`; a width that is not positive gives the default one.
    pub fn set_width(&mut self, column: DifferenceColumn, width: i32) {
        if let Some(choice) = self.0.iter_mut().find(|c| c.column == column) {
            choice.width = if width > 0 {
                width
            } else {
                column.default_width()
            };
        }
    }

    /// Moves the column at `index` one place up, towards the start of the row, and returns
    /// its new index; `None` when it is first already.
    pub fn move_up(&mut self, index: usize) -> Option<usize> {
        if index == 0 || index >= self.0.len() {
            return None;
        }
        self.0.swap(index - 1, index);
        Some(index - 1)
    }

    /// Moves the column at `index` one place down, towards the end of the row, and returns
    /// its new index; `None` when it is last already.
    pub fn move_down(&mut self, index: usize) -> Option<usize> {
        if index + 1 >= self.0.len() {
            return None;
        }
        self.0.swap(index, index + 1);
        Some(index + 1)
    }

    /// The value as the database stores it: the columns in their order, each as its name
    /// and width, a hidden one after a minus: "name:220,change:140,-folder:420".
    pub fn to_text(&self) -> String {
        self.0
            .iter()
            .map(|c| {
                let hidden = if c.shown { "" } else { "-" };
                format!("{hidden}{}:{}", c.column.as_str(), c.width)
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    /// The columns from `text` as [`Self::to_text`] writes it. What it cannot read is
    /// passed over: an unknown or repeated column, a width that is not a positive number,
    /// which gives the default one. A column it does not name comes last, shown, as a
    /// column a later revisor adds would; «Ім'я» is shown whatever the text says.
    pub fn parse(text: &str) -> Self {
        let mut choices: Vec<ColumnChoice> = Vec::with_capacity(6);
        for item in text.split(',') {
            let item = item.trim();
            let (shown, item) = match item.strip_prefix('-') {
                Some(rest) => (false, rest),
                None => (true, item),
            };
            let (name, width) = match item.split_once(':') {
                Some((name, width)) => (name, width.trim().parse::<i32>().ok()),
                None => (item, None),
            };
            let Some(column) = DifferenceColumn::parse(name.trim()) else {
                continue;
            };
            if choices.iter().any(|c| c.column == column) {
                continue;
            }
            choices.push(ColumnChoice {
                column,
                shown: shown || !column.can_hide(),
                width: width.filter(|&w| w > 0).unwrap_or(column.default_width()),
            });
        }
        for column in DifferenceColumn::ALL {
            if !choices.iter().any(|c| c.column == column) {
                choices.push(ColumnChoice::default_of(column));
            }
        }
        let mut all = Self::default();
        all.0.copy_from_slice(&choices);
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use DifferenceColumn::*;

    #[test]
    fn by_default_all_six_show_in_their_order_with_their_widths() {
        let columns = DifferenceColumns::default();
        assert_eq!(columns.shown(), DifferenceColumn::ALL.to_vec());
        let widths: Vec<i32> = columns.choices().iter().map(|c| c.width).collect();
        assert_eq!(widths, [220, 140, 200, 90, 160, 420]);
        assert_eq!(
            columns.to_text(),
            "name:220,change:140,what_changed:200,size:90,date_modified:160,folder:420"
        );
    }

    #[test]
    fn the_order_the_visibility_and_the_widths_go_through_the_text() {
        let mut columns = DifferenceColumns::default();
        columns.move_up(1);
        columns.set_shown(5, false);
        columns.set_shown(4, false);
        columns.set_width(WhatChanged, 333);
        let text = columns.to_text();
        assert_eq!(
            text,
            "change:140,name:220,what_changed:333,size:90,-date_modified:160,-folder:420"
        );
        assert_eq!(DifferenceColumns::parse(&text), columns);
        assert_eq!(columns.shown(), [Change, Name, WhatChanged, Size]);
    }

    #[test]
    fn the_name_cannot_be_hidden_but_moves() {
        let mut columns = DifferenceColumns::default();
        columns.set_shown(0, false);
        assert!(columns.choices()[0].shown);
        assert_eq!(columns.move_down(0), Some(1));
        assert_eq!(columns.choices()[1].column, Name);
        // Nor from the database.
        let read = DifferenceColumns::parse("-name:100,change:140");
        assert!(read.choices()[0].shown);
        assert!(!Name.can_hide());
        assert!(DifferenceColumn::ALL[1..].iter().all(|c| c.can_hide()));
    }

    #[test]
    fn moves_stop_at_the_ends() {
        let mut columns = DifferenceColumns::default();
        assert_eq!(columns.move_up(0), None);
        assert_eq!(columns.move_down(5), None);
        assert_eq!(columns.move_up(6), None);
        assert_eq!(columns, DifferenceColumns::default());
        assert_eq!(columns.move_up(5), Some(4));
        assert_eq!(columns.choices()[4].column, Folder);
    }

    #[test]
    fn what_the_text_lacks_or_spoils_takes_the_default() {
        let read = DifferenceColumns::parse("folder:0,bogus:5,folder:9,-size:x,name");
        let order: Vec<DifferenceColumn> = read.choices().iter().map(|c| c.column).collect();
        assert_eq!(
            order,
            [Folder, Size, Name, Change, WhatChanged, DateModified]
        );
        assert_eq!(read.width(Folder), 420, "not positive");
        assert_eq!(read.width(Size), 90, "not a number");
        assert!(!read.choices()[1].shown);
        assert_eq!(read.width(Name), 220, "no width");
        assert!(read.choices()[3..].iter().all(|c| c.shown));
        assert_eq!(DifferenceColumns::parse(""), DifferenceColumns::default());
    }

    #[test]
    fn a_width_that_is_not_positive_is_the_default() {
        let mut columns = DifferenceColumns::default();
        columns.set_width(Size, 0);
        assert_eq!(columns.width(Size), 90);
        columns.set_width(Size, 55);
        assert_eq!(columns.width(Size), 55);
    }

    #[test]
    fn the_layout_ignores_the_widths() {
        let mut columns = DifferenceColumns::default();
        columns.set_width(Name, 1);
        assert!(columns.same_layout(&DifferenceColumns::default()));
        columns.set_shown(3, false);
        assert!(!columns.same_layout(&DifferenceColumns::default()));
    }
}
