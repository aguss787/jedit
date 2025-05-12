use std::{cell::RefCell, slice::Iter};

use crate::container::node::{Index, IndexKind, NodeKind, NodeMeta};

#[derive(Debug)]
pub struct WorkTreeNode {
    name: String,
    len: usize,
    meta: Option<NodeMeta>,
    child: Option<Vec<WorkTreeNode>>,
}

impl WorkTreeNode {
    pub fn new(name: String, meta: Option<NodeMeta>) -> Self {
        Self {
            name,
            len: 1,
            meta,
            child: None,
        }
    }

    pub fn new_empty(name: String) -> Self {
        Self {
            name,
            len: 1,
            meta: None,
            child: None,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn as_tree_string(&self) -> impl Iterator<Item = String> {
        std::iter::once(self.formatted_name(0))
            .chain(WorkTreeStringIter::new(self.child.as_deref()))
    }

    pub fn selector(&self, index: usize) -> Vec<&str> {
        let mut res = Vec::new();

        self.traverse_node(
            index,
            &mut |node| {
                if !std::ptr::eq(self, node) {
                    res.push(node.name.as_str());
                }
            },
            &mut |_| {},
            |_| {},
        );

        res
    }

    pub fn reindex(&mut self, index: usize, node_index: Index, force: bool) {
        let (len, child) = match node_index.kind {
            IndexKind::Terminal => (1, Vec::new()),
            IndexKind::Object(items) => (
                items.len() + 1,
                items.into_iter().map(WorkTreeNode::new_empty).collect(),
            ),
            IndexKind::Array(n) => (
                n + 1,
                (0..n)
                    .map(|i| WorkTreeNode::new_empty(i.to_string()))
                    .collect(),
            ),
        };

        let old_len = RefCell::new(None);

        self.traverse_node_mut(
            index,
            &mut |_| {},
            &mut |node: &mut WorkTreeNode, _| {
                if let Some(old_len) = *old_len.borrow() {
                    node.len -= old_len;
                    node.len += len;
                }
            },
            |node: &mut WorkTreeNode| {
                node.meta = Some(node_index.meta);
                if node.child.is_some() || force {
                    *old_len.borrow_mut() = Some(node.len);
                    node.child = Some(child);
                }
            },
        );
    }

    pub(crate) fn rename(&mut self, index: usize, new_key: String) {
        let new_key_len = new_key.len();
        let old_key_len = RefCell::new(0);
        self.traverse_node_mut(
            index,
            &mut |_| {},
            &mut |node: &mut WorkTreeNode, _| {
                if let Some(meta) = &mut node.meta {
                    meta.n_bytes -= *old_key_len.borrow();
                    meta.n_bytes += new_key_len;
                }
            },
            |node: &mut WorkTreeNode| {
                *old_key_len.borrow_mut() = node.name.len();
                node.name = new_key;
            },
        );
    }

    pub(crate) fn delete(&mut self, index: usize, mut parent_metas: Vec<NodeMeta>) {
        let should_delete = RefCell::new(true);
        self.traverse_node_mut(
            index,
            &mut |_| {},
            &mut |node: &mut WorkTreeNode, child_index| {
                if *should_delete.borrow() {
                    let (Some(child), Some(child_index)) = (&mut node.child, child_index) else {
                        return;
                    };
                    child.remove(child_index);
                    let Some(meta) = node.meta else {
                        return;
                    };

                    if matches!(meta.kind, NodeKind::Array) {
                        for (index, child) in child.iter_mut().enumerate() {
                            child.name = index.to_string();
                        }
                    }
                    *should_delete.borrow_mut() = false;
                }

                if !*should_delete.borrow() {
                    node.len -= 1;
                    node.meta = Some(parent_metas.pop().expect("missing parent meta"));
                }
            },
            |_| {},
        );
    }

    pub fn close(&mut self, index: usize) {
        let old_len = RefCell::new(1);
        self.traverse_node_mut(
            index,
            &mut |_| {},
            &mut |node: &mut WorkTreeNode, _| {
                node.len -= *old_len.borrow();
            },
            |node: &mut WorkTreeNode| {
                *old_len.borrow_mut() = node.len - 1;
                node.child = None;
            },
        );
    }

    pub fn meta(&self, index: usize) -> Option<NodeMeta> {
        self.traverse_node(index, &mut |_| {}, &mut |_| {}, |node| node.meta)
    }

    fn traverse_node<'a, B, A, F, R>(
        &'a self,
        mut index: usize,
        before_visit_hook: &mut B,
        after_visit_hook: &mut A,
        on_found_hook: F,
    ) -> R
    where
        B: FnMut(&'a WorkTreeNode),
        A: FnMut(&'a WorkTreeNode),
        F: FnOnce(&'a WorkTreeNode) -> R,
    {
        before_visit_hook(self);
        if index == 0 {
            let res = on_found_hook(self);
            after_visit_hook(self);
            return res;
        }

        if index >= self.len {
            panic!("unexpected index");
        }

        index -= 1;
        let child = self.child.as_deref().into_iter().flatten();
        for child in child {
            if index < child.len {
                let res =
                    child.traverse_node(index, before_visit_hook, after_visit_hook, on_found_hook);
                after_visit_hook(self);
                return res;
            }

            index -= child.len;
        }

        unreachable!()
    }

    fn traverse_node_mut<B, A, F>(
        &mut self,
        mut index: usize,
        before_visit_hook: &mut B,
        after_visit_hook: &mut A,
        on_found_hook: F,
    ) where
        B: FnMut(&mut WorkTreeNode),
        A: FnMut(&mut WorkTreeNode, Option<usize>),
        F: FnOnce(&mut WorkTreeNode),
    {
        before_visit_hook(self);
        if index == 0 {
            on_found_hook(self);
            after_visit_hook(self, None);
            return;
        }

        if index >= self.len {
            panic!("unexpected index");
        }

        index -= 1;
        let child = self.child.as_deref_mut().into_iter().flatten();
        for (child_index, child) in child.enumerate() {
            if index < child.len {
                child.traverse_node_mut(index, before_visit_hook, after_visit_hook, on_found_hook);
                after_visit_hook(self, Some(child_index));
                return;
            }

            index -= child.len;
        }

        unreachable!()
    }

    fn formatted_name(&self, indent: usize) -> String {
        prefix(indent).chain(self.name.chars()).collect()
    }
}

pub struct WorkTreeStringIter<'a> {
    stack: Vec<Iter<'a, WorkTreeNode>>,
}

impl<'a> WorkTreeStringIter<'a> {
    fn new(init: Option<&'a [WorkTreeNode]>) -> Self {
        Self {
            stack: if let Some(init) = init {
                vec![init.iter()]
            } else {
                Vec::new()
            },
        }
    }
}

impl<'a> Iterator for WorkTreeStringIter<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let mut next = None;
        while next.is_none() {
            let next_iter = self.stack.last_mut()?;
            next = next_iter.next();
            if next.is_none() {
                self.stack.pop();
            }
        }

        let next = next?;
        let depth = self.stack.len();
        if let Some(child) = &next.child {
            self.stack.push(child.iter());
        }
        Some(next.formatted_name(depth))
    }
}

fn prefix(depth: usize) -> impl Iterator<Item = char> {
    (0..(2 * depth)).map(|_| '-')
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn work_tree_formatting_test() {
        let mut node = WorkTreeNode::new_empty(String::from("root"));
        node.reindex(
            0,
            Index {
                meta: NodeMeta::null(),
                kind: IndexKind::Object(vec![
                    String::from("a"),
                    String::from("b"),
                    String::from("c"),
                    String::from("d"),
                ]),
            },
            true,
        );
        node.reindex(
            1,
            Index {
                meta: NodeMeta::null(),
                kind: IndexKind::Object(vec![String::from("aa"), String::from("ab")]),
            },
            true,
        );
        node.reindex(
            4,
            Index {
                meta: NodeMeta::null(),
                kind: IndexKind::Array(3),
            },
            true,
        );
        node.reindex(
            8,
            Index {
                meta: NodeMeta::null(),
                kind: IndexKind::Array(5),
            },
            true,
        );
        node.close(8);

        assert_eq!(
            node.as_tree_string().collect::<Vec<_>>(),
            vec![
                String::from("root"),
                String::from("--a"),
                String::from("----aa"),
                String::from("----ab"),
                String::from("--b"),
                String::from("----0"),
                String::from("----1"),
                String::from("----2"),
                String::from("--c"),
                String::from("--d"),
            ]
        );
    }

    #[test]
    fn work_tree_selector_test() {
        let mut node = WorkTreeNode::new_empty(String::from("root"));
        node.reindex(
            0,
            Index {
                meta: NodeMeta::null(),
                kind: IndexKind::Object(vec![
                    String::from("a"),
                    String::from("b"),
                    String::from("c"),
                    String::from("d"),
                ]),
            },
            true,
        );
        node.reindex(
            1,
            Index {
                meta: NodeMeta::null(),
                kind: IndexKind::Object(vec![String::from("aa"), String::from("ab")]),
            },
            true,
        );
        node.reindex(
            4,
            Index {
                meta: NodeMeta::null(),
                kind: IndexKind::Array(3),
            },
            true,
        );

        assert_eq!(node.len(), 10);
        assert_eq!(node.selector(0), Vec::<&str>::new());
        assert_eq!(node.selector(1), vec!["a"]);
        assert_eq!(node.selector(2), vec!["a", "aa"]);
        assert_eq!(node.selector(3), vec!["a", "ab"]);
        assert_eq!(node.selector(4), vec!["b"]);
        assert_eq!(node.selector(5), vec!["b", "0"]);
        assert_eq!(node.selector(8), vec!["c"]);
    }
}
