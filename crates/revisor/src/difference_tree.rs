//! The tree of the difference in the main window (TASK-33): the items of a tree control
//! kept in line with the nodes of the model, [`difference::List`]. The control has items
//! only for the nodes of the top level and of the folders opened so far: the children of
//! a node come when it is first expanded (#2), so a difference of a million rows opens at
//! once and UI Automation has no more than that to walk. When the filter or the setting
//! «Вузли дерева» changes, the items change in place — those hidden go, those shown come,
//! the texts that differ are written again —, so the item with the focus, when it stays,
//! is left as it is.
//!
//! A level of more than [`PAGE`] nodes shows them a page at a time (TASK-37): putting tens
//! of thousands of items in the control takes many seconds, with a screen reader told of
//! each. The pages have fixed bounds — the first [`PAGE`] nodes, the next ones, and so on
//! —, and an item «Ще…» stands after the pages shown, or before them, for the nodes not
//! shown there: [`TreeView::turn`] puts the next page in its place. A level shows the
//! first page, or the one of the node the focus goes to; when the filter changes, it
//! shows one page again.
//!
//! The view reaches its control only through [`Control`]: [`Native`] over a `TreeCtrl` in
//! the window, a mock in the tests. It sets the selection itself while it changes the
//! control, and is [`TreeView::busy`] then: the control's events are its own, and the
//! window leaves them alone.
//!
//! [`difference::List`]: crate::difference::List

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ops::Range;

use wxdragon::prelude::*;

use crate::difference::{List, NodeId};

/// How many nodes of one level come into the control at a time (TASK-37). The texts of
/// the items «Ще…» name the number too.
pub const PAGE: usize = 1000;

/// An item «Ще…» of a level shown by pages: it stands for the nodes not shown before the
/// pages shown, or after them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct More {
    /// The node whose children the level is; none for the top level.
    pub parent: Option<NodeId>,
    /// Whether it stands for the nodes before the pages shown.
    pub above: bool,
}

/// What an item of the control stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Entry {
    Node(NodeId),
    More(More),
}

/// What the view needs of the model: the nodes the filter shows, and what they say.
pub trait Source {
    /// The nodes of the top level, in order.
    fn roots(&self) -> Vec<NodeId>;
    /// The nodes inside `node`, in order.
    fn children(&self, node: NodeId) -> Vec<NodeId>;
    fn has_children(&self, node: NodeId) -> bool;
    /// The text of `node`.
    fn label(&self, node: NodeId) -> String;
    /// The nodes `node` is inside, from the top level down.
    fn path(&self, node: NodeId) -> Vec<NodeId>;
    /// The text of the item «Ще…» that stands for `count` nodes, before the pages shown
    /// when `above`, else after them.
    fn more_label(&self, count: usize, above: bool) -> String;
}

impl Source for List {
    fn roots(&self) -> Vec<NodeId> {
        self.tree_roots()
    }

    fn children(&self, node: NodeId) -> Vec<NodeId> {
        self.tree_children(node)
    }

    fn has_children(&self, node: NodeId) -> bool {
        self.tree_has_children(node)
    }

    fn label(&self, node: NodeId) -> String {
        self.tree_label(node)
    }

    fn path(&self, node: NodeId) -> Vec<NodeId> {
        self.tree_path(node)
    }

    fn more_label(&self, count: usize, above: bool) -> String {
        self.tree_more_label(count, above)
    }
}

/// What the view needs of its control.
pub trait Control {
    /// An item of the control.
    type Item: Clone;
    /// Takes every item out.
    fn clear(&self);
    /// Puts an item for `entry` with `text` under `parent`, or at the top level, right
    /// after `after`, or first; `has_children` gives it the button that expands it.
    fn insert(
        &self,
        parent: Option<&Self::Item>,
        after: Option<&Self::Item>,
        entry: Entry,
        text: &str,
        has_children: bool,
    ) -> Option<Self::Item>;
    /// Takes `item` out, with the items under it.
    fn delete(&self, item: &Self::Item);
    fn set_text(&self, item: &Self::Item, text: &str);
    fn set_has_children(&self, item: &Self::Item, has_children: bool);
    fn expand(&self, item: &Self::Item);
    fn is_expanded(&self, item: &Self::Item) -> bool;
    /// What the selected item stands for, when there is one.
    fn selected(&self) -> Option<Entry>;
    /// Makes `item` the selected one, with the focus rectangle, scrolled into view; the
    /// keyboard focus stays where it is.
    fn select(&self, item: &Self::Item);
    /// Stops drawing the control while many of its items change, or draws it again: an
    /// item put under an expanded one, or taken from there, otherwise moves the scroll
    /// bar each time.
    fn freeze(&self, on: bool);
}

/// An item of the control as the view knows it.
struct Item<I> {
    item: I,
    text: String,
    has_children: bool,
    /// The level under it, once it has been expanded.
    children: Option<Level<I>>,
}

/// An item «Ще…» as the view knows it.
struct MoreItem<I> {
    item: I,
    text: String,
}

/// The items of one level: of the top one, or of the children of a node.
struct Level<I> {
    /// The nodes that have items, in order: the pages shown.
    nodes: Vec<NodeId>,
    /// How many nodes the filter shows before them.
    first: usize,
    /// The item «Ще…» for the nodes before them.
    above: Option<MoreItem<I>>,
    /// The item «Ще…» for the nodes after them.
    below: Option<MoreItem<I>>,
}

impl<I> Default for Level<I> {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            first: 0,
            above: None,
            below: None,
        }
    }
}

impl<I> Level<I> {
    fn more(&self, above: bool) -> Option<&MoreItem<I>> {
        if above {
            self.above.as_ref()
        } else {
            self.below.as_ref()
        }
    }
}

struct Mirror<I> {
    top: Level<I>,
    items: HashMap<NodeId, Item<I>>,
}

impl<I> Default for Mirror<I> {
    fn default() -> Self {
        Self {
            top: Level::default(),
            items: HashMap::new(),
        }
    }
}

impl<I> Mirror<I> {
    /// The level under `parent`, or the top one.
    fn level(&self, parent: Option<NodeId>) -> Option<&Level<I>> {
        match parent {
            None => Some(&self.top),
            Some(node) => self.items.get(&node)?.children.as_ref(),
        }
    }

    /// The level under `parent`, or the top one; a node with an item gets its level.
    fn level_mut(&mut self, parent: Option<NodeId>) -> Option<&mut Level<I>> {
        match parent {
            None => Some(&mut self.top),
            Some(node) => Some(
                self.items
                    .get_mut(&node)?
                    .children
                    .get_or_insert_with(Level::default),
            ),
        }
    }
}

/// The folders that had their children in the control, each one after the folders it is
/// inside, and whether they were expanded: what [`TreeView::open`] reads from a tree and
/// [`TreeView::fill_open`] gives to one filled anew.
#[derive(Default)]
pub struct Open(Vec<(NodeId, bool)>);

/// The way to the node the focus goes to: for each level on it, the node of that level
/// the way goes through. The key is the level's parent.
type Way = HashMap<Option<NodeId>, NodeId>;

fn way_to(node: Option<NodeId>, source: &dyn Source) -> Way {
    let mut way = Way::new();
    if let Some(node) = node {
        let mut parent = None;
        for step in source.path(node).into_iter().chain([node]) {
            way.insert(parent, step);
            parent = Some(step);
        }
    }
    way
}

/// Which of `nodes` a level shows at first: all of them up to [`PAGE`], else the page
/// with `focus`, or the first one.
fn page_of(nodes: &[NodeId], focus: Option<NodeId>) -> Range<usize> {
    if nodes.len() <= PAGE {
        return 0..nodes.len();
    }
    let at = focus
        .and_then(|focus| nodes.iter().position(|&node| node == focus))
        .unwrap_or(0);
    let start = at / PAGE * PAGE;
    start..(start + PAGE).min(nodes.len())
}

/// The items of one tree control over the model's nodes.
pub struct TreeView<C: Control> {
    control: C,
    mirror: RefCell<Mirror<C::Item>>,
    busy: Cell<bool>,
    /// The item «Ще…» that has the focus, when one has it.
    more: Cell<Option<More>>,
}

impl<C: Control> TreeView<C> {
    pub fn new(control: C) -> Self {
        Self {
            control,
            mirror: RefCell::new(Mirror::default()),
            busy: Cell::new(false),
            more: Cell::new(None),
        }
    }

    pub fn control(&self) -> &C {
        &self.control
    }

    /// Whether the view is changing its control: the control's events are its own then.
    pub fn busy(&self) -> bool {
        self.busy.get()
    }

    /// Whether the control has any item.
    pub fn is_empty(&self) -> bool {
        self.mirror.borrow().items.is_empty()
    }

    /// Shows the top level of `source` in place of what the control showed, with the way
    /// to `current` expanded and `current` selected (#8).
    pub fn fill(&self, source: &dyn Source, current: Option<NodeId>) {
        self.while_busy(|| {
            self.control.clear();
            *self.mirror.borrow_mut() = Mirror::default();
            self.show(source, current);
        });
    }

    /// Whether the item selected in the control is that of `current`.
    pub fn shows(&self, current: Option<NodeId>) -> bool {
        match (self.control.selected(), current) {
            (Some(Entry::Node(node)), Some(current)) => node == current,
            (None, None) => true,
            _ => false,
        }
    }

    /// The folders that are open, for [`Self::fill_open`] to open them again in a tree
    /// filled anew (TASK-38).
    pub fn open(&self) -> Open {
        let mut open = Vec::new();
        {
            let mirror = self.mirror.borrow();
            let mut levels = vec![&mirror.top];
            while let Some(level) = levels.pop() {
                for node in &level.nodes {
                    let Some(item) = mirror.items.get(node) else {
                        continue;
                    };
                    if let Some(children) = &item.children {
                        open.push((*node, self.control.is_expanded(&item.item)));
                        levels.push(children);
                    }
                }
            }
        }
        Open(open)
    }

    /// Shows `source` as [`Self::fill`] does, with the folders of `open` that it still
    /// shows open again (TASK-38). A folder shown by pages shows one page, as after
    /// [`Self::refresh`], and a folder not on the page shown stays closed.
    pub fn fill_open(&self, source: &dyn Source, current: Option<NodeId>, open: &Open) {
        self.while_busy(|| {
            self.control.clear();
            *self.mirror.borrow_mut() = Mirror::default();
            self.more.set(None);
            let way = way_to(current, source);
            self.sync(None, source.roots(), source, &way);
            // The children come while their folder is closed, and the folders open from
            // the deepest one: an item put under an expanded one moves the scroll bar.
            for &(node, _) in &open.0 {
                if source.has_children(node) {
                    self.populate(node, source, &way);
                }
            }
            for &(node, expanded) in open.0.iter().rev() {
                if !expanded {
                    continue;
                }
                let filled = self
                    .mirror
                    .borrow()
                    .items
                    .get(&node)
                    .is_some_and(|item| item.children.is_some());
                if let (true, Some(item)) = (filled, self.item(node)) {
                    self.control.expand(&item);
                }
            }
            // Last, so the item selected is in view once the folders above it are open.
            if let Some(node) = current {
                self.reveal(node, source, &way);
            }
        });
    }

    /// Brings the items in line with `source`, whose filter or texts changed, and selects
    /// `current`, expanding the way to it (#4). A level shown by pages shows one page
    /// again: that of the node on the way to `current`, or the first (TASK-37 #8).
    pub fn refresh(&self, source: &dyn Source, current: Option<NodeId>) {
        self.while_busy(|| {
            self.control.freeze(true);
            self.show(source, current);
            self.control.freeze(false);
        });
    }

    fn show(&self, source: &dyn Source, current: Option<NodeId>) {
        self.more.set(None);
        let way = way_to(current, source);
        self.sync(None, source.roots(), source, &way);
        if let Some(node) = current {
            self.reveal(node, source, &way);
        }
    }

    /// Takes every item out.
    pub fn clear(&self) {
        self.while_busy(|| {
            self.control.clear();
            *self.mirror.borrow_mut() = Mirror::default();
            self.more.set(None);
        });
    }

    /// `node` is about to expand: its children come now, the first time (#2). Not while
    /// the view expands it itself, having put them there already.
    pub fn expanding(&self, node: NodeId, source: &dyn Source) {
        if !self.busy() {
            self.while_busy(|| self.populate(node, source, &Way::new()));
        }
    }

    /// The focus of the control has moved to the item of `entry`. Gives the node the
    /// model keeps as the one with the focus: the node itself, or for an item «Ще…» the
    /// node shown next to it, so the page with it shows when the filter changes.
    pub fn focus_moved(&self, entry: Entry) -> Option<NodeId> {
        match entry {
            Entry::Node(node) => {
                self.more.set(None);
                Some(node)
            }
            Entry::More(more) => {
                self.more.set(Some(more));
                let mirror = self.mirror.borrow();
                let level = mirror.level(more.parent)?;
                if more.above {
                    level.nodes.first().copied()
                } else {
                    level.nodes.last().copied()
                }
            }
        }
    }

    /// The item «Ще…» that has the focus, when one has it.
    pub fn more(&self) -> Option<More> {
        self.more.get()
    }

    /// Turns the page at the item `more` (TASK-37 #5, #6): the next page comes before the
    /// item «Ще…» after the pages shown, the previous one after the item before them, and
    /// the item stays with a smaller number, or goes with the last page. The focus goes
    /// to the node that was next to the item's nodes: the first one added, or the last.
    /// Gives that node, which the model keeps as the one with the focus.
    pub fn turn(&self, more: More, source: &dyn Source) -> Option<NodeId> {
        let (first, shown) = {
            let mirror = self.mirror.borrow();
            let level = mirror.level(more.parent)?;
            level.more(more.above)?;
            (level.first, level.nodes.len())
        };
        let wanted = match more.parent {
            None => source.roots(),
            Some(node) => source.children(node),
        };
        let end = first + shown;
        if end > wanted.len() {
            return None;
        }
        let (window, focus) = if more.above {
            (first.saturating_sub(PAGE)..end, first.checked_sub(1)?)
        } else {
            (first..(end + PAGE).min(wanted.len()), end)
        };
        let focus = *wanted.get(focus)?;
        self.while_busy(|| {
            self.control.freeze(true);
            self.sync_nodes(more.parent, &wanted, window.clone(), source, None);
            // Before the item «Ще…» with the focus goes, so the control does not move
            // the focus itself.
            if let Some(item) = self.item(focus) {
                self.control.select(&item);
            }
            self.sync_more(more.parent, wanted.len(), window, source);
            self.control.freeze(false);
        });
        self.more.set(None);
        Some(focus)
    }

    fn while_busy(&self, act: impl FnOnce()) {
        let outer = self.busy.replace(true);
        act();
        self.busy.set(outer);
    }

    /// Puts the children of `node` under its item, once.
    fn populate(&self, node: NodeId, source: &dyn Source, way: &Way) {
        let unfilled = self
            .mirror
            .borrow()
            .items
            .get(&node)
            .is_some_and(|item| item.children.is_none());
        if unfilled {
            self.sync(Some(node), source.children(node), source, way);
        }
    }

    /// Expands the way to `node`, whose children come as the folders open, and selects it.
    fn reveal(&self, node: NodeId, source: &dyn Source, way: &Way) {
        for above in source.path(node) {
            self.populate(above, source, way);
            let Some(item) = self.item(above) else {
                return;
            };
            self.control.expand(&item);
        }
        if let Some(item) = self.item(node) {
            self.control.select(&item);
        }
    }

    fn item(&self, node: NodeId) -> Option<C::Item> {
        self.mirror
            .borrow()
            .items
            .get(&node)
            .map(|item| item.item.clone())
    }

    /// Makes the items under `parent`, or of the top level, those of one page of
    /// `wanted`, or of all of it when it is no more than a page: the page of the node
    /// `way` goes through, else the first. The items «Ще…» stand for the rest.
    fn sync(&self, parent: Option<NodeId>, wanted: Vec<NodeId>, source: &dyn Source, way: &Way) {
        let window = page_of(&wanted, way.get(&parent).copied());
        self.sync_nodes(parent, &wanted, window.clone(), source, Some(way));
        self.sync_more(parent, wanted.len(), window, source);
    }

    /// Makes the items of the nodes under `parent`, or of the top level, those of
    /// `window` of `all`, in order: the items not wanted go, the missing ones come in
    /// their places, and, with a `way`, those that stay take their new texts, and their
    /// own children likewise once they have them. Without one, those that stay are left
    /// as they are.
    fn sync_nodes(
        &self,
        parent: Option<NodeId>,
        all: &[NodeId],
        window: Range<usize>,
        source: &dyn Source,
        way: Option<&Way>,
    ) {
        let wanted = &all[window.clone()];
        let (old, above) = {
            let mirror = self.mirror.borrow();
            mirror.level(parent).map_or_else(
                || (Vec::new(), None),
                |level| {
                    (
                        level.nodes.clone(),
                        level.above.as_ref().map(|more| more.item.clone()),
                    )
                },
            )
        };
        let keep: HashSet<NodeId> = wanted.iter().copied().collect();
        // From the last one: the control moves its selection from an item taken out to
        // the next one, and would do so again for every item of a page.
        for &node in old.iter().rev().filter(|node| !keep.contains(node)) {
            if let Some(item) = self.item(node) {
                self.control.delete(&item);
            }
            self.forget(node);
        }
        let parent_item = parent.and_then(|node| self.item(node));
        // The item «Ще…» before the nodes, when there is one, stays the first.
        let mut after = above;
        for &node in wanted {
            let item = match self.item(node) {
                Some(item) => {
                    if let Some(way) = way {
                        self.update(node, &item, source, way);
                    }
                    item
                }
                None => {
                    let text = source.label(node);
                    let has_children = source.has_children(node);
                    let item = self.control.insert(
                        parent_item.as_ref(),
                        after.as_ref(),
                        Entry::Node(node),
                        &text,
                        has_children,
                    );
                    let Some(item) = item else {
                        continue;
                    };
                    self.mirror.borrow_mut().items.insert(
                        node,
                        Item {
                            item: item.clone(),
                            text,
                            has_children,
                            children: None,
                        },
                    );
                    item
                }
            };
            after = Some(item);
        }
        let mut mirror = self.mirror.borrow_mut();
        if let Some(level) = mirror.level_mut(parent) {
            level.nodes = wanted.to_vec();
            level.first = window.start;
        }
    }

    /// Brings the items «Ще…» under `parent`, or of the top level, in line with `window`,
    /// the nodes shown of the `total` of the level: one before them for the nodes above,
    /// one after them for the rest.
    fn sync_more(
        &self,
        parent: Option<NodeId>,
        total: usize,
        window: Range<usize>,
        source: &dyn Source,
    ) {
        let parent_item = parent.and_then(|node| self.item(node));
        let counts = [(true, window.start), (false, total - window.end)];
        for (above, count) in counts {
            let wanted = (count > 0).then(|| source.more_label(count, above));
            let (old, last) = {
                let mirror = self.mirror.borrow();
                let Some(level) = mirror.level(parent) else {
                    return;
                };
                let old = level
                    .more(above)
                    .map(|more| (more.item.clone(), more.text.clone()));
                // The item after the nodes comes after the last of them.
                let last = level
                    .nodes
                    .last()
                    .and_then(|node| mirror.items.get(node))
                    .map(|item| item.item.clone())
                    .or_else(|| level.above.as_ref().map(|more| more.item.clone()));
                (old, last)
            };
            let new = match (old, wanted) {
                (None, None) => None,
                (Some((item, _)), None) => {
                    self.control.delete(&item);
                    None
                }
                (Some((item, text)), Some(wanted)) => {
                    if text != wanted {
                        self.control.set_text(&item, &wanted);
                    }
                    Some(MoreItem { item, text: wanted })
                }
                (None, Some(wanted)) => {
                    let after = if above { None } else { last };
                    self.control
                        .insert(
                            parent_item.as_ref(),
                            after.as_ref(),
                            Entry::More(More { parent, above }),
                            &wanted,
                            false,
                        )
                        .map(|item| MoreItem { item, text: wanted })
                }
            };
            let mut mirror = self.mirror.borrow_mut();
            if let Some(level) = mirror.level_mut(parent) {
                if above {
                    level.above = new;
                } else {
                    level.below = new;
                }
            }
        }
    }

    /// Brings the item of `node`, which stays, in line: its text, its button, and its
    /// children once it has them.
    fn update(&self, node: NodeId, item: &C::Item, source: &dyn Source, way: &Way) {
        let text = source.label(node);
        let has_children = source.has_children(node);
        let (old_text, had_children, filled) = {
            let mirror = self.mirror.borrow();
            let known = &mirror.items[&node];
            (
                known.text.clone(),
                known.has_children,
                known.children.is_some(),
            )
        };
        if old_text != text {
            self.control.set_text(item, &text);
        }
        if filled {
            let children = if has_children {
                source.children(node)
            } else {
                Vec::new()
            };
            self.sync(Some(node), children, source, way);
        }
        if had_children != has_children {
            self.control.set_has_children(item, has_children);
        }
        let mut mirror = self.mirror.borrow_mut();
        if let Some(known) = mirror.items.get_mut(&node) {
            known.text = text;
            known.has_children = has_children;
            // A folder whose children all went is filled again when it comes back.
            if !has_children {
                known.children = None;
            }
        }
    }

    /// Forgets the item of `node` and those under it, which the control has taken out.
    fn forget(&self, node: NodeId) {
        let mut mirror = self.mirror.borrow_mut();
        let mut gone = vec![node];
        while let Some(node) = gone.pop() {
            if let Some(item) = mirror.items.remove(&node) {
                gone.extend(item.children.into_iter().flat_map(|level| level.nodes));
            }
        }
    }
}

/// The tree control of the difference in the main window: a native tree with buttons and
/// lines, one item selected, its root hidden, so the folders of the difference are its top
/// level. Each item keeps what it stands for.
pub struct Native {
    tree: TreeCtrl,
    root: TreeItemId,
}

impl Native {
    pub fn new(parent: &dyn WxWidget) -> Self {
        let tree = TreeCtrl::builder(parent)
            .with_style(
                TreeCtrlStyle::HasButtons
                    | TreeCtrlStyle::LinesAtRoot
                    | TreeCtrlStyle::Single
                    | TreeCtrlStyle::HideRoot,
            )
            .build();
        let root = tree
            .add_root("", None, None)
            .expect("a new tree takes its root");
        Self { tree, root }
    }

    /// The control, for the window's sizers and events.
    pub fn tree(&self) -> &TreeCtrl {
        &self.tree
    }

    /// What `item` stands for.
    pub fn entry_of(&self, item: &TreeItemId) -> Option<Entry> {
        let data = self.tree.get_custom_data_direct(item)?;
        data.downcast_ref::<Entry>().copied()
    }

    /// Where a context menu opened from the keyboard goes: the screen point at the bottom
    /// left of the item with the focus, or the tree's top left corner without one.
    pub fn context_menu_point(&self) -> (i32, i32) {
        let point = self
            .tree
            .get_focused_item()
            .and_then(|item| self.tree.get_bounding_rect(&item, true))
            .map_or(Point::new(0, 0), |rect| {
                Point::new(rect.x, rect.y + rect.height)
            });
        let point = self.tree.client_to_screen(point);
        (point.x, point.y)
    }
}

impl Control for Native {
    type Item = TreeItemId;

    fn clear(&self) {
        self.tree.delete_children(&self.root);
    }

    fn insert(
        &self,
        parent: Option<&TreeItemId>,
        after: Option<&TreeItemId>,
        entry: Entry,
        text: &str,
        has_children: bool,
    ) -> Option<TreeItemId> {
        let parent = parent.unwrap_or(&self.root);
        let item = match after {
            Some(after) => self.tree.insert_item(parent, after, text, None, None),
            None => self.tree.prepend_item(parent, text, None, None),
        }?;
        self.tree.set_custom_data_direct(&item, entry);
        if has_children {
            self.tree.set_item_has_children(&item, true);
        }
        Some(item)
    }

    fn delete(&self, item: &TreeItemId) {
        self.tree.delete(item);
    }

    fn set_text(&self, item: &TreeItemId, text: &str) {
        self.tree.set_item_text(item, text);
    }

    fn set_has_children(&self, item: &TreeItemId, has_children: bool) {
        self.tree.set_item_has_children(item, has_children);
    }

    fn expand(&self, item: &TreeItemId) {
        self.tree.expand(item);
    }

    fn is_expanded(&self, item: &TreeItemId) -> bool {
        self.tree.is_expanded(item)
    }

    fn selected(&self) -> Option<Entry> {
        self.entry_of(&self.tree.get_selection()?)
    }

    fn select(&self, item: &TreeItemId) {
        self.tree.select_item(item);
        self.tree.ensure_visible(item);
    }

    fn freeze(&self, on: bool) {
        if on {
            self.tree.freeze();
        } else {
            self.tree.thaw();
        }
    }
}

#[cfg(test)]
mod tests;
