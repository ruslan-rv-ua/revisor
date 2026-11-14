//! The rows of the difference as a tree of their folders (TASK-33), apart from the tree
//! control: which nodes there are and in which order, and which of them the filter shows,
//! with how many changes each holds.
//!
//! Each row is one node, so the list and the tree show the same entries and the focus
//! goes from one to the other on the same entry; a folder with rows inside it that is no
//! row itself is a node of its own, and a folder that is a row holds what is inside it.
//! The folders of the difference are the top level, named by their whole paths. The
//! children of a node come in the order of the rows, the order the comparison walks the
//! trees (TASK-13 #20): by name regardless of case, folders and files alike, an entry
//! whose kind changed deleted before added — the order «Шлях» of the list (decided
//! 2026-11-07). The nodes are numbered in that order, depth first, so the subtree of a
//! node is the run of numbers from it to its end.

use revisor_core::settings::ChangeType;

use super::{Index, NONE, Row};

/// A node of the difference tree, as the window knows it: its number, which its control's
/// item keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

#[derive(Clone, Copy, Debug)]
struct Node {
    /// The node above, or [`NONE`] at the top level.
    parent: u32,
    /// The row the node is, or [`NONE`] for a folder that is no row itself.
    row: u32,
    /// For a folder that is no row: the folder of the index whose path it is.
    folder: u32,
    first_child: u32,
    next_sibling: u32,
    /// The first node after the node's subtree.
    end: u32,
}

/// How many of the rows the filter lets through are in the subtree of a node, its own row
/// included: the changes, and the inaccessible entries, which are no changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shown {
    pub changes: u32,
    pub inaccessible: u32,
}

impl Shown {
    fn any(self) -> bool {
        self.changes + self.inaccessible > 0
    }

    fn add(&mut self, other: Shown) {
        self.changes += other.changes;
        self.inaccessible += other.inaccessible;
    }

    fn of(row: &Row) -> Self {
        match row.kind {
            ChangeType::Inaccessible => Self {
                changes: 0,
                inaccessible: 1,
            },
            _ => Self {
                changes: 1,
                inaccessible: 0,
            },
        }
    }
}

pub struct Tree {
    nodes: Vec<Node>,
    /// The nodes of the top level, in order.
    roots: Vec<u32>,
    /// The node of each row.
    of_row: Vec<u32>,
    /// What the filter shows of each node's subtree.
    shown: Vec<Shown>,
}

impl Tree {
    /// The tree of `rows`, in tree order, whose folders `index` knows.
    pub(super) fn new(rows: &[Row], index: &Index) -> Self {
        let folders = &index.folders;
        let rank = |folder: u32| index.folder_rank[folder as usize] as usize;
        let ranks = index
            .folder_rank
            .iter()
            .max()
            .map_or(0, |&r| r as usize + 1);
        let mut tree = Builder {
            nodes: Vec::with_capacity(rows.len() + rows.len() / 8),
            last_child: Vec::with_capacity(rows.len() + rows.len() / 8),
            roots: Vec::new(),
            last_root: NONE,
        };
        // The node that holds what is inside a folder, by the folder's path: the row of
        // the folder met last, or the folder's own node.
        let mut holder = vec![NONE; ranks];
        let mut of_row = Vec::with_capacity(rows.len());
        let mut chain = Vec::new();
        for r in 0..rows.len() as u32 {
            let parent = if index.is_root(r) {
                NONE
            } else {
                let folder = index.parent[r as usize];
                match holder[rank(folder)] {
                    NONE => {
                        // The folders on the way that have no node yet, from the inside out.
                        chain.clear();
                        let mut at = folder;
                        let mut above = NONE;
                        while at != NONE {
                            match holder[rank(at)] {
                                NONE => {
                                    chain.push(at);
                                    at = folders.parents[at as usize];
                                }
                                node => {
                                    above = node;
                                    break;
                                }
                            }
                        }
                        for &folder in chain.iter().rev() {
                            above = tree.push(above, NONE, folder);
                            holder[rank(folder)] = above;
                        }
                        above
                    }
                    node => node,
                }
            };
            let node = tree.push(parent, r, NONE);
            of_row.push(node);
            let own = index.own_folder[r as usize];
            if own != NONE {
                holder[rank(own)] = node;
            }
        }
        let Builder {
            mut nodes, roots, ..
        } = tree;
        // A subtree ends where the last of its nodes does: the nodes come depth first.
        for n in (0..nodes.len()).rev() {
            let (parent, end) = (nodes[n].parent, nodes[n].end);
            if parent != NONE {
                let parent = &mut nodes[parent as usize];
                parent.end = parent.end.max(end);
            }
        }
        let shown = vec![Shown::default(); nodes.len()];
        Self {
            nodes,
            roots,
            of_row,
            shown,
        }
    }

    /// Lets through the rows `passed` says, by row: a node is shown when a row of its
    /// subtree is.
    pub(super) fn filter(&mut self, rows: &[Row], passed: &[bool]) {
        for (n, node) in self.nodes.iter().enumerate() {
            self.shown[n] = match node.row {
                NONE => Shown::default(),
                r if passed[r as usize] => Shown::of(&rows[r as usize]),
                _ => Shown::default(),
            };
        }
        for n in (0..self.nodes.len()).rev() {
            let parent = self.nodes[n].parent;
            if parent != NONE {
                let below = self.shown[n];
                self.shown[parent as usize].add(below);
            }
        }
    }

    /// How many nodes the tree has, shown or not.
    #[cfg_attr(not(test), expect(dead_code))]
    pub(super) fn len(&self) -> usize {
        self.nodes.len()
    }

    pub(super) fn is_shown(&self, node: u32) -> bool {
        self.shown
            .get(node as usize)
            .is_some_and(|shown| shown.any())
    }

    /// What the filter shows of the subtree of `node`, its own row included.
    pub(super) fn shown(&self, node: u32) -> Shown {
        self.shown[node as usize]
    }

    /// What the filter shows inside `node`, without its own row: in its children's
    /// subtrees.
    pub(super) fn inside(&self, node: u32) -> Shown {
        let mut inside = Shown::default();
        let mut child = self.nodes[node as usize].first_child;
        while child != NONE {
            inside.add(self.shown[child as usize]);
            child = self.nodes[child as usize].next_sibling;
        }
        inside
    }

    /// The row a node is, if it is one.
    pub(super) fn row(&self, node: u32) -> Option<u32> {
        Some(self.nodes[node as usize].row).filter(|&r| r != NONE)
    }

    /// The folder of the index that a folder which is no row is.
    pub(super) fn folder(&self, node: u32) -> u32 {
        self.nodes[node as usize].folder
    }

    pub(super) fn node_of_row(&self, row: u32) -> u32 {
        self.of_row[row as usize]
    }

    /// The nodes of the top level that are shown, in order.
    pub(super) fn shown_roots(&self) -> impl Iterator<Item = u32> + '_ {
        self.roots.iter().copied().filter(|&n| self.is_shown(n))
    }

    /// The children of `node` that are shown, in order.
    pub(super) fn shown_children(&self, node: u32) -> impl Iterator<Item = u32> + '_ {
        let mut child = self
            .nodes
            .get(node as usize)
            .map_or(NONE, |node| node.first_child);
        std::iter::from_fn(move || {
            while child != NONE {
                let at = child;
                child = self.nodes[at as usize].next_sibling;
                if self.is_shown(at) {
                    return Some(at);
                }
            }
            None
        })
    }

    /// The nodes `node` is inside, from the top level down.
    pub(super) fn ancestors(&self, node: u32) -> impl Iterator<Item = u32> + use<> {
        let mut path = Vec::new();
        let mut at = self.nodes.get(node as usize).map_or(NONE, |n| n.parent);
        while at != NONE {
            path.push(at);
            at = self.nodes[at as usize].parent;
        }
        path.into_iter().rev()
    }

    /// The rows of the subtree of `node`, its own included, shown or not.
    pub(super) fn rows_in(&self, node: u32) -> impl Iterator<Item = u32> + '_ {
        let end = self.nodes[node as usize].end;
        self.nodes[node as usize..end as usize]
            .iter()
            .map(|n| n.row)
            .filter(|&r| r != NONE)
    }

    /// The first row the filter lets through in the subtree of `node`: its own, or the
    /// first inside the folder.
    pub(super) fn first_row_in(&self, node: u32, passed: &[bool]) -> Option<u32> {
        self.rows_in(node).find(|&r| passed[r as usize])
    }

    /// The node of the first row the filter lets through: the first change of the tree.
    pub(super) fn first_row_node(&self, passed: &[bool]) -> Option<u32> {
        let row = passed.iter().position(|&p| p)?;
        Some(self.of_row[row])
    }

    /// `node` when it is shown, else the next node shown after it, else the one before it
    /// (TASK-13 #27), in tree order.
    pub(super) fn nearest_shown(&self, node: u32) -> Option<u32> {
        if self.is_shown(node) {
            return Some(node);
        }
        let len = self.nodes.len() as u32;
        (node.saturating_add(1)..len)
            .find(|&n| self.is_shown(n))
            .or_else(|| (0..node.min(len)).rev().find(|&n| self.is_shown(n)))
    }
}

/// The tree as it is being made: nodes come in tree order, each after its parent.
struct Builder {
    nodes: Vec<Node>,
    /// The last child of each node so far.
    last_child: Vec<u32>,
    roots: Vec<u32>,
    last_root: u32,
}

impl Builder {
    /// Adds the last child of `parent`, or of the top level with [`NONE`]: the row `row`,
    /// or the folder `folder` that is no row.
    fn push(&mut self, parent: u32, row: u32, folder: u32) -> u32 {
        let node = self.nodes.len() as u32;
        self.nodes.push(Node {
            parent,
            row,
            folder,
            first_child: NONE,
            next_sibling: NONE,
            end: node + 1,
        });
        self.last_child.push(NONE);
        let before = if parent == NONE {
            self.roots.push(node);
            std::mem::replace(&mut self.last_root, node)
        } else {
            std::mem::replace(&mut self.last_child[parent as usize], node)
        };
        match before {
            NONE if parent != NONE => self.nodes[parent as usize].first_child = node,
            NONE => {}
            before => self.nodes[before as usize].next_sibling = node,
        }
        node
    }
}
