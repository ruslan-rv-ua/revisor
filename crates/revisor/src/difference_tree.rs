//! The tree of the difference in the main window (TASK-33): the items of a tree control
//! kept in line with the nodes of the model, [`difference::List`]. The control has items
//! only for the nodes of the top level and of the folders opened so far: the children of
//! a node come when it is first expanded (#2), so a difference of a million rows opens at
//! once and UI Automation has no more than that to walk. When the filter or the setting
//! «Вузли дерева» changes, the items change in place — those hidden go, those shown come,
//! the texts that differ are written again —, so the item with the focus, when it stays,
//! is left as it is.
//!
//! The view reaches its control only through [`Control`]: [`Native`] over a `TreeCtrl` in
//! the window, a mock in the tests. It sets the selection itself while it changes the
//! control, and is [`TreeView::busy`] then: the control's events are its own, and the
//! window leaves them alone.
//!
//! [`difference::List`]: crate::difference::List

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

use wxdragon::prelude::*;

use crate::difference::{List, NodeId};

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
}

/// What the view needs of its control.
pub trait Control {
    /// An item of the control.
    type Item: Clone;
    /// Takes every item out.
    fn clear(&self);
    /// Puts an item for `node` with `text` under `parent`, or at the top level, right
    /// after `after`, or first; `has_children` gives it the button that expands it.
    fn insert(
        &self,
        parent: Option<&Self::Item>,
        after: Option<&Self::Item>,
        node: NodeId,
        text: &str,
        has_children: bool,
    ) -> Option<Self::Item>;
    /// Takes `item` out, with the items under it.
    fn delete(&self, item: &Self::Item);
    fn set_text(&self, item: &Self::Item, text: &str);
    fn set_has_children(&self, item: &Self::Item, has_children: bool);
    fn expand(&self, item: &Self::Item);
    /// Makes `item` the selected one, with the focus rectangle, scrolled into view; the
    /// keyboard focus stays where it is.
    fn select(&self, item: &Self::Item);
}

/// An item of the control as the view knows it.
struct Item<I> {
    item: I,
    text: String,
    has_children: bool,
    /// The nodes under it, once it has been expanded.
    children: Option<Vec<NodeId>>,
}

struct Mirror<I> {
    /// The nodes of the top level.
    top: Vec<NodeId>,
    items: HashMap<NodeId, Item<I>>,
}

impl<I> Default for Mirror<I> {
    fn default() -> Self {
        Self {
            top: Vec::new(),
            items: HashMap::new(),
        }
    }
}

/// The items of one tree control over the model's nodes.
pub struct TreeView<C: Control> {
    control: C,
    mirror: RefCell<Mirror<C::Item>>,
    busy: Cell<bool>,
}

impl<C: Control> TreeView<C> {
    pub fn new(control: C) -> Self {
        Self {
            control,
            mirror: RefCell::new(Mirror::default()),
            busy: Cell::new(false),
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
            self.sync(None, source.roots(), source);
            if let Some(node) = current {
                self.reveal(node, source);
            }
        });
    }

    /// Brings the items in line with `source`, whose filter or texts changed, and selects
    /// `current`, expanding the way to it (#4).
    pub fn refresh(&self, source: &dyn Source, current: Option<NodeId>) {
        self.while_busy(|| {
            self.sync(None, source.roots(), source);
            if let Some(node) = current {
                self.reveal(node, source);
            }
        });
    }

    /// Takes every item out.
    pub fn clear(&self) {
        self.while_busy(|| {
            self.control.clear();
            *self.mirror.borrow_mut() = Mirror::default();
        });
    }

    /// `node` is about to expand: its children come now, the first time (#2). Not while
    /// the view expands it itself, having put them there already.
    pub fn expanding(&self, node: NodeId, source: &dyn Source) {
        if !self.busy() {
            self.while_busy(|| self.populate(node, source));
        }
    }

    fn while_busy(&self, act: impl FnOnce()) {
        let outer = self.busy.replace(true);
        act();
        self.busy.set(outer);
    }

    /// Puts the children of `node` under its item, once.
    fn populate(&self, node: NodeId, source: &dyn Source) {
        let unfilled = self
            .mirror
            .borrow()
            .items
            .get(&node)
            .is_some_and(|item| item.children.is_none());
        if unfilled {
            self.sync(Some(node), source.children(node), source);
        }
    }

    /// Expands the way to `node`, whose children come as the folders open, and selects it.
    fn reveal(&self, node: NodeId, source: &dyn Source) {
        for above in source.path(node) {
            self.populate(above, source);
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

    /// Makes the items under `parent`, or of the top level, those of `wanted`, in order:
    /// the items not wanted go, the missing ones come in their places, and those that stay
    /// take their new texts, and their own children likewise once they have them.
    fn sync(&self, parent: Option<NodeId>, wanted: Vec<NodeId>, source: &dyn Source) {
        let old = {
            let mirror = self.mirror.borrow();
            match parent {
                None => mirror.top.clone(),
                Some(node) => mirror
                    .items
                    .get(&node)
                    .and_then(|item| item.children.clone())
                    .unwrap_or_default(),
            }
        };
        let keep: HashSet<NodeId> = wanted.iter().copied().collect();
        for &node in old.iter().filter(|node| !keep.contains(node)) {
            if let Some(item) = self.item(node) {
                self.control.delete(&item);
            }
            self.forget(node);
        }
        let parent_item = parent.and_then(|node| self.item(node));
        let mut after = None;
        for &node in &wanted {
            let text = source.label(node);
            let has_children = source.has_children(node);
            let item = match self.item(node) {
                Some(item) => {
                    self.update(node, &item, text, has_children, source);
                    item
                }
                None => {
                    let item = self.control.insert(
                        parent_item.as_ref(),
                        after.as_ref(),
                        node,
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
        match parent {
            None => mirror.top = wanted,
            Some(node) => {
                if let Some(item) = mirror.items.get_mut(&node) {
                    item.children = Some(wanted);
                }
            }
        }
    }

    /// Brings the item of `node`, which stays, in line: its text, its button, and its
    /// children once it has them.
    fn update(
        &self,
        node: NodeId,
        item: &C::Item,
        text: String,
        has_children: bool,
        source: &dyn Source,
    ) {
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
            self.sync(Some(node), children, source);
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
                gone.extend(item.children.into_iter().flatten());
            }
        }
    }
}

/// The tree control of the difference in the main window: a native tree with buttons and
/// lines, one item selected, its root hidden, so the folders of the difference are its top
/// level. Each item keeps the number of its node.
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

    /// The node of `item`.
    pub fn node_of(&self, item: &TreeItemId) -> Option<NodeId> {
        let data = self.tree.get_custom_data_direct(item)?;
        data.downcast_ref::<u32>().copied().map(NodeId)
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
        node: NodeId,
        text: &str,
        has_children: bool,
    ) -> Option<TreeItemId> {
        let parent = parent.unwrap_or(&self.root);
        let item = match after {
            Some(after) => self.tree.insert_item(parent, after, text, None, None),
            None => self.tree.prepend_item(parent, text, None, None),
        }?;
        self.tree.set_custom_data_direct(&item, node.0);
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

    fn select(&self, item: &TreeItemId) {
        self.tree.select_item(item);
        self.tree.ensure_visible(item);
    }
}

#[cfg(test)]
mod tests;
