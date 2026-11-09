use super::*;
use std::collections::BTreeMap;

/// A model of nodes: each node's text and children, the filter as a set of hidden nodes.
#[derive(Default)]
struct Model {
    texts: BTreeMap<u32, String>,
    children: BTreeMap<u32, Vec<u32>>,
    roots: Vec<u32>,
    hidden: HashSet<u32>,
}

impl Model {
    /// A model of `(node, parent, text)`, in order; the parent [`u32::MAX`] is the top level.
    fn of(nodes: &[(u32, u32, &str)]) -> Self {
        let mut model = Self::default();
        for &(node, parent, text) in nodes {
            model.texts.insert(node, text.to_string());
            if parent == u32::MAX {
                model.roots.push(node);
            } else {
                model.children.entry(parent).or_default().push(node);
            }
        }
        model
    }

    fn shown(&self, nodes: &[u32]) -> Vec<NodeId> {
        nodes
            .iter()
            .filter(|node| !self.hidden.contains(node))
            .map(|&node| NodeId(node))
            .collect()
    }
}

impl Source for Model {
    fn roots(&self) -> Vec<NodeId> {
        self.shown(&self.roots)
    }

    fn children(&self, node: NodeId) -> Vec<NodeId> {
        self.shown(self.children.get(&node.0).map_or(&[][..], Vec::as_slice))
    }

    fn has_children(&self, node: NodeId) -> bool {
        !self.children(node).is_empty()
    }

    fn label(&self, node: NodeId) -> String {
        self.texts[&node.0].clone()
    }

    fn path(&self, node: NodeId) -> Vec<NodeId> {
        let mut path = Vec::new();
        let mut at = node.0;
        while let Some((&parent, _)) = self.children.iter().find(|(_, c)| c.contains(&at)) {
            path.push(NodeId(parent));
            at = parent;
        }
        path.reverse();
        path
    }

    fn more_label(&self, count: usize, above: bool) -> String {
        if above {
            format!("ще {count} вище")
        } else {
            format!("ще {count}")
        }
    }
}

/// An item of the mock control.
#[derive(Clone, Debug)]
struct MockItem {
    entry: Entry,
    text: String,
    has_children: bool,
    expanded: bool,
    parent: Option<usize>,
}

/// A control that keeps its items and notes the calls that change them.
#[derive(Default)]
struct Mock {
    /// The items by their number; `None` once deleted.
    items: RefCell<Vec<Option<MockItem>>>,
    /// The children of each item and of the top level (`usize::MAX`), in order.
    order: RefCell<HashMap<usize, Vec<usize>>>,
    selected: Cell<Option<usize>>,
    calls: RefCell<Vec<String>>,
}

impl Mock {
    fn note(&self, call: String) {
        self.calls.borrow_mut().push(call);
    }

    fn take_calls(&self) -> Vec<String> {
        std::mem::take(&mut *self.calls.borrow_mut())
    }

    /// The texts of the items as an outline: two spaces per level, `+` for a collapsed
    /// item with a button, `-` for an expanded one.
    fn outline(&self) -> Vec<String> {
        let mut lines = Vec::new();
        self.walk(usize::MAX, 0, &mut lines);
        lines
    }

    fn walk(&self, parent: usize, depth: usize, lines: &mut Vec<String>) {
        let order = self
            .order
            .borrow()
            .get(&parent)
            .cloned()
            .unwrap_or_default();
        for i in order {
            let item = self.items.borrow()[i]
                .clone()
                .expect("a deleted item is out of the order");
            let mark = match (item.has_children, item.expanded) {
                (false, _) => " ",
                (true, false) => "+",
                (true, true) => "-",
            };
            let selected = if self.selected.get() == Some(i) {
                " *"
            } else {
                ""
            };
            lines.push(format!(
                "{}{mark}{}{selected}",
                "  ".repeat(depth),
                item.text
            ));
            if item.expanded {
                self.walk(i, depth + 1, lines);
            }
        }
    }

    fn selected_node(&self) -> Option<NodeId> {
        let i = self.selected.get()?;
        match self.items.borrow()[i].as_ref()?.entry {
            Entry::Node(node) => Some(node),
            Entry::More(_) => None,
        }
    }

    /// The texts of the items under the item of `parent`, or of the top level, in order.
    fn level(&self, parent: Option<u32>) -> Vec<String> {
        let parent = parent.map_or(usize::MAX, |node| self.item_of(node).expect("an item"));
        let order = self.order.borrow();
        let items = self.items.borrow();
        order.get(&parent).map_or_else(Vec::new, |level| {
            level
                .iter()
                .map(|&i| items[i].as_ref().expect("an item").text.clone())
                .collect()
        })
    }

    /// The number of the item of `node`.
    fn item_of(&self, node: u32) -> Option<usize> {
        self.items.borrow().iter().position(|item| {
            item.as_ref()
                .is_some_and(|item| item.entry == Entry::Node(NodeId(node)))
        })
    }
}

impl Control for Mock {
    type Item = usize;

    fn clear(&self) {
        self.note("clear".into());
        self.items
            .borrow_mut()
            .iter_mut()
            .for_each(|item| *item = None);
        self.order.borrow_mut().clear();
        self.selected.set(None);
    }

    fn insert(
        &self,
        parent: Option<&usize>,
        after: Option<&usize>,
        entry: Entry,
        text: &str,
        has_children: bool,
    ) -> Option<usize> {
        self.note(format!("insert {text}"));
        let i = self.items.borrow().len();
        self.items.borrow_mut().push(Some(MockItem {
            entry,
            text: text.into(),
            has_children,
            expanded: false,
            parent: parent.copied(),
        }));
        let mut order = self.order.borrow_mut();
        let siblings = order
            .entry(parent.copied().unwrap_or(usize::MAX))
            .or_default();
        let at = after.map_or(0, |after| {
            siblings
                .iter()
                .position(|s| s == after)
                .expect("after a sibling")
                + 1
        });
        siblings.insert(at, i);
        Some(i)
    }

    fn delete(&self, item: &usize) {
        let text = self.items.borrow()[*item].as_ref().unwrap().text.clone();
        self.note(format!("delete {text}"));
        let parent = self.items.borrow()[*item].as_ref().unwrap().parent;
        let mut gone = vec![*item];
        while let Some(i) = gone.pop() {
            gone.extend(self.order.borrow_mut().remove(&i).unwrap_or_default());
            if self.selected.get() == Some(i) {
                self.selected.set(None);
            }
            self.items.borrow_mut()[i] = None;
        }
        if let Some(siblings) = self
            .order
            .borrow_mut()
            .get_mut(&parent.unwrap_or(usize::MAX))
        {
            siblings.retain(|s| s != item);
        }
    }

    fn set_text(&self, item: &usize, text: &str) {
        self.note(format!("text {text}"));
        self.items.borrow_mut()[*item].as_mut().unwrap().text = text.into();
    }

    fn set_has_children(&self, item: &usize, has_children: bool) {
        let mut items = self.items.borrow_mut();
        let item = items[*item].as_mut().unwrap();
        item.has_children = has_children;
        if !has_children {
            item.expanded = false;
        }
    }

    fn expand(&self, item: &usize) {
        self.items.borrow_mut()[*item].as_mut().unwrap().expanded = true;
    }

    fn select(&self, item: &usize) {
        self.selected.set(Some(*item));
    }

    fn freeze(&self, _on: bool) {}
}

/// A folder `docs` with a file and a folder `img` of two files, and a file at the top.
fn example() -> Model {
    Model::of(&[
        (0, u32::MAX, "docs — 3 зміни"),
        (1, 0, "a.txt — додано"),
        (2, 0, "img — 2 зміни"),
        (3, 2, "b.png — видалено"),
        (4, 2, "c.png — змінено: розмір"),
        (5, u32::MAX, "x.txt — додано"),
    ])
}

#[test]
fn the_tree_opens_with_the_top_level_and_the_way_to_the_first_change() {
    let model = example();
    let view = TreeView::new(Mock::default());
    view.fill(&model, Some(NodeId(1)));
    assert_eq!(
        view.control().outline(),
        [
            "-docs — 3 зміни",
            "   a.txt — додано *",
            "  +img — 2 зміни",
            " x.txt — додано"
        ]
    );
    // The folder not opened has no items yet (#2).
    assert_eq!(view.control().item_of(3), None);
    assert!(!view.busy());
}

#[test]
fn a_folder_gets_its_children_when_it_is_first_expanded() {
    let model = example();
    let view = TreeView::new(Mock::default());
    view.fill(&model, Some(NodeId(0)));
    assert_eq!(
        view.control().take_calls(),
        ["clear", "insert docs — 3 зміни", "insert x.txt — додано"]
    );
    view.expanding(NodeId(0), &model);
    assert_eq!(
        view.control().take_calls(),
        ["insert a.txt — додано", "insert img — 2 зміни"]
    );
    view.expanding(NodeId(2), &model);
    assert_eq!(
        view.control().take_calls(),
        ["insert b.png — видалено", "insert c.png — змінено: розмір"]
    );
    // Once only.
    view.expanding(NodeId(2), &model);
    assert!(view.control().take_calls().is_empty());
}

#[test]
fn the_view_ignores_the_expanding_it_makes_itself() {
    let model = example();
    let view = TreeView::new(Mock::default());
    view.while_busy(|| view.expanding(NodeId(0), &model));
    assert!(view.control().take_calls().is_empty());
}

#[test]
fn a_filter_change_leaves_the_items_that_stay_as_they_are() {
    let mut model = example();
    let view = TreeView::new(Mock::default());
    view.fill(&model, Some(NodeId(4)));
    let selected = view.control().item_of(4);
    view.control().take_calls();
    // The deleted file goes, the counts of its folders change.
    model.hidden.insert(3);
    model.texts.insert(0, "docs — 2 зміни".into());
    model.texts.insert(2, "img — 1 зміна".into());
    view.refresh(&model, Some(NodeId(4)));
    assert_eq!(
        view.control().take_calls(),
        [
            "text docs — 2 зміни",
            "text img — 1 зміна",
            "delete b.png — видалено"
        ]
    );
    assert_eq!(
        view.control().item_of(4),
        selected,
        "the item with the focus stays"
    );
    assert_eq!(
        view.control().outline(),
        [
            "-docs — 2 зміни",
            "   a.txt — додано",
            "  -img — 1 зміна",
            "     c.png — змінено: розмір *",
            " x.txt — додано"
        ]
    );
    // It comes back in its place.
    model.hidden.clear();
    model.texts.insert(0, "docs — 3 зміни".into());
    model.texts.insert(2, "img — 2 зміни".into());
    view.refresh(&model, Some(NodeId(4)));
    assert_eq!(
        view.control().outline(),
        [
            "-docs — 3 зміни",
            "   a.txt — додано",
            "  -img — 2 зміни",
            "     b.png — видалено",
            "     c.png — змінено: розмір *",
            " x.txt — додано"
        ]
    );
}

#[test]
fn a_hidden_branch_goes_and_comes_back_closed() {
    let mut model = example();
    let view = TreeView::new(Mock::default());
    view.fill(&model, Some(NodeId(3)));
    // Everything in `img` hidden: the folder goes with it.
    model.hidden.extend([2, 3, 4]);
    model.texts.insert(0, "docs — 1 зміна".into());
    view.refresh(&model, Some(NodeId(1)));
    assert_eq!(
        view.control().outline(),
        ["-docs — 1 зміна", "   a.txt — додано *", " x.txt — додано"]
    );
    assert_eq!(view.control().selected_node(), Some(NodeId(1)));
    model.hidden.clear();
    model.texts.insert(0, "docs — 3 зміни".into());
    view.refresh(&model, Some(NodeId(1)));
    assert_eq!(
        view.control().outline(),
        [
            "-docs — 3 зміни",
            "   a.txt — додано *",
            "  +img — 2 зміни",
            " x.txt — додано"
        ]
    );
}

#[test]
fn a_folder_whose_children_all_go_loses_its_button() {
    let mut model = example();
    let view = TreeView::new(Mock::default());
    view.fill(&model, Some(NodeId(4)));
    // The folder `img` is a change of its own and stays, without its files.
    model.hidden.extend([3, 4]);
    view.refresh(&model, Some(NodeId(2)));
    assert_eq!(
        view.control().outline(),
        [
            "-docs — 3 зміни",
            "   a.txt — додано",
            "   img — 2 зміни *",
            " x.txt — додано"
        ]
    );
    // Its files come back: it is filled again when it opens.
    model.hidden.clear();
    view.refresh(&model, Some(NodeId(2)));
    assert_eq!(view.control().item_of(3), None);
    view.expanding(NodeId(2), &model);
    assert!(view.control().item_of(3).is_some());
}

#[test]
fn everything_hidden_leaves_the_tree_empty() {
    let mut model = example();
    let view = TreeView::new(Mock::default());
    view.fill(&model, Some(NodeId(1)));
    model.hidden.extend(0..6);
    view.refresh(&model, None);
    assert!(view.control().outline().is_empty());
    assert!(view.is_empty());
    assert_eq!(view.control().selected_node(), None);
}

#[test]
fn clearing_takes_every_item_out() {
    let model = example();
    let view = TreeView::new(Mock::default());
    view.fill(&model, Some(NodeId(4)));
    assert!(!view.is_empty());
    view.clear();
    assert!(view.is_empty());
    assert!(view.control().outline().is_empty());
    // A fill afterwards starts afresh.
    view.fill(&model, None);
    assert_eq!(view.control().outline().len(), 2);
}

// --- A level of more than a page (TASK-37) ---------------------------------------------

/// A folder `big`, the node 0, of `files` files, the nodes 1 and on, `f1` and on.
fn big(files: u32) -> Model {
    let names: Vec<String> = (1..=files).map(|i| format!("f{i}")).collect();
    let mut nodes = vec![(0, u32::MAX, "big")];
    nodes.extend((1..=files).map(|i| (i, 0, names[i as usize - 1].as_str())));
    Model::of(&nodes)
}

/// The first and the last of `texts`, and how many they are.
fn ends(texts: &[String]) -> (&str, &str, usize) {
    (
        texts.first().map_or("", String::as_str),
        texts.last().map_or("", String::as_str),
        texts.len(),
    )
}

const BELOW: More = More {
    parent: Some(NodeId(0)),
    above: false,
};
const ABOVE: More = More {
    parent: Some(NodeId(0)),
    above: true,
};

#[test]
fn a_folder_of_more_than_a_page_opens_with_its_first_page() {
    let model = big(2500);
    let view = TreeView::new(Mock::default());
    view.fill(&model, Some(NodeId(0)));
    view.control().take_calls();
    view.expanding(NodeId(0), &model);
    let level = view.control().level(Some(0));
    assert_eq!(ends(&level), ("f1", "ще 1500", PAGE + 1));
    assert_eq!(level[PAGE - 1], "f1000");
    assert_eq!(
        view.control().take_calls().len(),
        PAGE + 1,
        "no more is put"
    );
    // A page exactly: all of it, and no node «Ще…».
    let model = big(PAGE as u32);
    let view = TreeView::new(Mock::default());
    view.fill(&model, Some(NodeId(1)));
    assert_eq!(ends(&view.control().level(Some(0))), ("f1", "f1000", PAGE));
}

#[test]
fn the_next_page_comes_before_the_node_more_and_takes_the_focus() {
    let model = big(2500);
    let view = TreeView::new(Mock::default());
    view.fill(&model, Some(NodeId(1)));
    assert_eq!(view.more(), None);
    // The focus on the node «Ще…»: the model keeps the node above it.
    assert_eq!(
        view.focus_moved(Entry::More(BELOW)),
        Some(NodeId(PAGE as u32))
    );
    assert_eq!(view.more(), Some(BELOW));
    view.control().take_calls();
    assert_eq!(view.turn(BELOW, &model), Some(NodeId(1001)));
    assert_eq!(view.more(), None);
    assert!(!view.busy());
    let calls = view.control().take_calls();
    assert_eq!(calls.len(), PAGE + 1);
    assert_eq!(calls[0], "insert f1001");
    assert_eq!(calls[PAGE], "text ще 500", "the node stays, with less");
    let level = view.control().level(Some(0));
    assert_eq!(ends(&level), ("f1", "ще 500", 2 * PAGE + 1));
    assert_eq!(level[PAGE], "f1001");
    assert_eq!(view.control().selected_node(), Some(NodeId(1001)));
    // The last page: the node «Ще…» goes.
    assert_eq!(view.turn(BELOW, &model), Some(NodeId(2001)));
    assert_eq!(ends(&view.control().level(Some(0))), ("f1", "f2500", 2500));
    assert_eq!(view.control().selected_node(), Some(NodeId(2001)));
    assert_eq!(view.turn(BELOW, &model), None);
}

#[test]
fn a_focus_beyond_the_first_page_shows_its_page_under_a_node_more() {
    let model = big(2500);
    let view = TreeView::new(Mock::default());
    view.fill(&model, Some(NodeId(1500)));
    let level = view.control().level(Some(0));
    assert_eq!(ends(&level), ("ще 1000 вище", "ще 500", PAGE + 2));
    assert_eq!((&*level[1], &*level[PAGE]), ("f1001", "f2000"));
    assert_eq!(view.control().selected_node(), Some(NodeId(1500)));
    // The focus on the node «Ще… вище»: the model keeps the node under it.
    assert_eq!(view.focus_moved(Entry::More(ABOVE)), Some(NodeId(1001)));
    // The page before comes above, and the focus goes to its last node.
    assert_eq!(view.turn(ABOVE, &model), Some(NodeId(1000)));
    let level = view.control().level(Some(0));
    assert_eq!(ends(&level), ("f1", "ще 500", 2 * PAGE + 1));
    assert_eq!((&*level[PAGE - 1], &*level[PAGE]), ("f1000", "f1001"));
    assert_eq!(view.control().selected_node(), Some(NodeId(1000)));
    assert_eq!(view.turn(ABOVE, &model), None);
    // A page in the middle, both ways.
    let model = big(3500);
    let view = TreeView::new(Mock::default());
    view.fill(&model, Some(NodeId(2001)));
    assert_eq!(
        ends(&view.control().level(Some(0))),
        ("ще 2000 вище", "ще 500", PAGE + 2)
    );
    assert_eq!(view.turn(ABOVE, &model), Some(NodeId(2000)));
    let level = view.control().level(Some(0));
    assert_eq!(ends(&level), ("ще 1000 вище", "ще 500", 2 * PAGE + 2));
    assert_eq!(level[1], "f1001");
}

#[test]
fn a_filter_change_leaves_one_page_the_first_or_that_of_the_focus() {
    let mut model = big(2500);
    let view = TreeView::new(Mock::default());
    view.fill(&model, Some(NodeId(1)));
    view.turn(BELOW, &model);
    assert_eq!(view.control().level(Some(0)).len(), 2 * PAGE + 1);
    // Ten files of the first page go: the node with the focus is on the first page now.
    model.hidden.extend(1..=10);
    view.refresh(&model, Some(NodeId(1001)));
    let level = view.control().level(Some(0));
    assert_eq!(ends(&level), ("f11", "ще 1490", PAGE + 1));
    assert_eq!(level[PAGE - 1], "f1010");
    assert_eq!(view.control().selected_node(), Some(NodeId(1001)));
    // They come back: it is on the second page again, the only one shown.
    model.hidden.clear();
    view.refresh(&model, Some(NodeId(1001)));
    let level = view.control().level(Some(0));
    assert_eq!(ends(&level), ("ще 1000 вище", "ще 500", PAGE + 2));
    assert_eq!(level[1], "f1001");
    assert_eq!(view.control().selected_node(), Some(NodeId(1001)));
    // The focus elsewhere: the first page.
    view.refresh(&model, Some(NodeId(0)));
    assert_eq!(
        ends(&view.control().level(Some(0))),
        ("f1", "ще 1500", PAGE + 1)
    );
    // Fewer than a page left: all of them, without a node «Ще…».
    model.hidden.extend(501..=2500);
    view.refresh(&model, Some(NodeId(0)));
    assert_eq!(ends(&view.control().level(Some(0))), ("f1", "f500", 500));
}

#[test]
fn the_top_level_shows_by_pages_too() {
    let names: Vec<String> = (0..1200).map(|i| format!("r{i}")).collect();
    let nodes: Vec<(u32, u32, &str)> = (0..1200u32)
        .map(|i| (i, u32::MAX, names[i as usize].as_str()))
        .collect();
    let model = Model::of(&nodes);
    let view = TreeView::new(Mock::default());
    view.fill(&model, Some(NodeId(1100)));
    assert_eq!(
        ends(&view.control().level(None)),
        ("ще 1000 вище", "r1199", 201)
    );
    let top = More {
        parent: None,
        above: true,
    };
    assert_eq!(view.focus_moved(Entry::More(top)), Some(NodeId(1000)));
    assert_eq!(view.turn(top, &model), Some(NodeId(999)));
    assert_eq!(ends(&view.control().level(None)), ("r0", "r1199", 1200));
    // A new difference starts with the first page.
    view.fill(&model, Some(NodeId(0)));
    assert_eq!(
        ends(&view.control().level(None)),
        ("r0", "ще 200", PAGE + 1)
    );
}
