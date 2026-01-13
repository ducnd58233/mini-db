use std::cmp::Ordering;
use std::fmt::Debug;

mod btree;
mod error;
mod schema;
mod storage;

const INTERNAL_NODE_MAX_KEYS: usize = 8;

#[derive(Debug, Clone)]
enum Node<K, V> {
    Internal(InternalNode<K, V>),
    Leaf(LeafNode<K, V>),
}

struct InsertResult<K, V> {
    left_node: Box<Node<K, V>>,
    split_info: Option<SplitInfo<K, V>>,
}

struct SplitInfo<K, V> {
    promoted_key: K,
    right_node: Box<Node<K, V>>,
}

#[derive(Debug, Clone)]
struct InternalNode<K, V> {
    len: usize,
    keys: [K; INTERNAL_NODE_MAX_KEYS],
    children: [Option<Box<Node<K, V>>>; INTERNAL_NODE_MAX_KEYS],
}

impl<K: Ord + Clone + Debug + Default, V: Clone + Debug + Default> InternalNode<K, V> {
    fn new() -> Self {
        Self {
            len: 0,
            keys: core::array::from_fn(|_| Default::default()),
            children: core::array::from_fn(|_| None),
        }
    }

    fn find_key_pos(&self, key: &K) -> usize {
        let mut left = 0;
        let mut right = self.len;

        while left < right {
            let mid = left + (right - left) / 2;
            match key.cmp(&self.keys[mid]) {
                Ordering::Less => right = mid,
                Ordering::Equal | Ordering::Greater => left = mid + 1,
            }
        }
        left
    }

    fn insert(&mut self, key: K, node: Box<Node<K, V>>) -> bool {
        let pos = self.find_key_pos(&key);

        for i in (pos..self.len).rev() {
            self.keys[i + 1] = std::mem::take(&mut self.keys[i]);
            self.children[i + 1] = self.children[i].take();
        }

        self.keys[pos] = key;
        self.children[pos] = Some(node);
        self.len += 1;

        self.len >= INTERNAL_NODE_MAX_KEYS
    }

    fn split(&mut self) -> (K, Box<Node<K, V>>) {
        let split_point = self.len / 2;
        let key_to_promote = self.keys[split_point].clone();
        let right_len = self.len - split_point;

        let mut right_keys: [K; INTERNAL_NODE_MAX_KEYS] =
            core::array::from_fn(|_| Default::default());
        let mut right_children: [Option<Box<Node<K, V>>>; INTERNAL_NODE_MAX_KEYS] =
            core::array::from_fn(|_| None);

        for i in 0..right_len {
            right_keys[i] = std::mem::take(&mut self.keys[split_point + i]);
            right_children[i] = self.children[split_point + i].take();
        }

        self.len = split_point;

        let right_internal = Box::new(Node::Internal(InternalNode {
            keys: right_keys,
            children: right_children,
            len: right_len,
        }));

        (key_to_promote, right_internal)
    }
}

#[derive(Debug, Clone)]
struct LeafNode<K, V> {
    len: usize,
    keys: [K; INTERNAL_NODE_MAX_KEYS],
    values: [V; INTERNAL_NODE_MAX_KEYS],
    next: Option<Box<LeafNode<K, V>>>,
}

impl<K: Ord + Clone + Debug + Default, V: Clone + Debug + Default> LeafNode<K, V> {
    fn new() -> Self {
        Self {
            len: 0,
            keys: core::array::from_fn(|_| Default::default()),
            values: core::array::from_fn(|_| Default::default()),
            next: None,
        }
    }

    fn find_key_pos(&self, key: &K) -> usize {
        let mut left = 0;
        let mut right = self.len;

        while left < right {
            let mid = left + (right - left) / 2;
            match key.cmp(&self.keys[mid]) {
                Ordering::Less => right = mid,
                Ordering::Equal | Ordering::Greater => left = mid + 1,
            }
        }

        left
    }

    fn insert(&mut self, key: K, value: V) -> bool {
        let pos = self.find_key_pos(&key);

        for i in (pos..self.len).rev() {
            self.keys[i + 1] = std::mem::take(&mut self.keys[i]);
            self.values[i + 1] = std::mem::take(&mut self.values[i]);
        }

        self.keys[pos] = key;
        self.values[pos] = value;
        self.len += 1;

        self.len >= INTERNAL_NODE_MAX_KEYS
    }

    fn split(&mut self) -> (K, Box<Node<K, V>>) {
        let split_point = self.len / 2;
        let key_to_promote = self.keys[split_point].clone();
        let right_len = self.len - split_point;

        let mut right_keys: [K; INTERNAL_NODE_MAX_KEYS] =
            core::array::from_fn(|_| Default::default());
        let mut right_values: [V; INTERNAL_NODE_MAX_KEYS] =
            core::array::from_fn(|_| Default::default());

        for i in 0..right_len {
            right_keys[i] = std::mem::take(&mut self.keys[split_point + i]);
            right_values[i] = std::mem::take(&mut self.values[split_point + i]);
        }

        self.len = split_point;

        let right_leaf = Box::new(Node::Leaf(LeafNode {
            keys: right_keys,
            values: right_values,
            next: self.next.take(),
            len: right_len,
        }));

        (key_to_promote, right_leaf)
    }
}

#[derive(Debug)]
struct BPTree<K, V> {
    root: Option<Box<Node<K, V>>>,
}

impl<K: Ord + Clone + Debug + Default, V: Clone + Debug + Default> BPTree<K, V> {
    fn new() -> Self {
        Self {
            root: Some(Box::new(Node::Leaf(LeafNode::new()))),
        }
    }

    fn get_promoted_key(node: &Node<K, V>) -> K {
        match node {
            Node::Leaf(leaf) => leaf.keys[0].clone(),
            Node::Internal(internal) => internal.keys[0].clone(),
        }
    }

    fn insert(&mut self, key: K, value: V) {
        let root = self.root.take().unwrap();
        let InsertResult {
            left_node: new_root,
            split_info,
        } = self.insert_recursive(root, key, value);

        if let Some(SplitInfo {
            promoted_key,
            right_node,
        }) = split_info
        {
            let mut internal = InternalNode::new();
            let left_min_key = Self::get_promoted_key(&new_root);
            internal.keys[0] = left_min_key;
            internal.keys[1] = promoted_key;
            internal.children[0] = Some(new_root);
            internal.children[1] = Some(right_node);
            internal.len = 2;
            self.root = Some(Box::new(Node::Internal(internal)));
        } else {
            self.root = Some(new_root);
        }
    }

    fn insert_recursive(&mut self, node: Box<Node<K, V>>, key: K, value: V) -> InsertResult<K, V> {
        match *node {
            Node::Leaf(mut leaf) => {
                let needs_split = leaf.insert(key, value);

                if needs_split {
                    let (promoted_key, right_node) = leaf.split();
                    return InsertResult {
                        left_node: Box::new(Node::Leaf(leaf)),
                        split_info: Some(SplitInfo {
                            promoted_key,
                            right_node,
                        }),
                    };
                }
                InsertResult {
                    left_node: Box::new(Node::Leaf(leaf)),
                    split_info: None,
                }
            }
            Node::Internal(mut internal) => {
                let child_idx = internal
                    .find_key_pos(&key)
                    .min(internal.len.saturating_sub(1));
                let child = internal.children[child_idx].take().unwrap();

                let InsertResult {
                    left_node: updated_child,
                    split_info,
                } = self.insert_recursive(child, key, value);
                internal.children[child_idx] = Some(updated_child);

                if let Some(SplitInfo {
                    promoted_key,
                    right_node,
                }) = split_info
                {
                    let needs_split = internal.insert(promoted_key, right_node);
                    if needs_split {
                        let (promoted_key, right_internal) = internal.split();
                        return InsertResult {
                            left_node: Box::new(Node::Internal(internal)),
                            split_info: Some(SplitInfo {
                                promoted_key,
                                right_node: right_internal,
                            }),
                        };
                    }
                }

                InsertResult {
                    left_node: Box::new(Node::Internal(internal)),
                    split_info: None,
                }
            }
        }
    }

    fn print_tree(&self) {
        if let Some(ref root) = self.root {
            self.print_node(root, 0);
        } else {
            println!("(empty tree)");
        }
    }

    fn print_node(&self, node: &Node<K, V>, depth: usize) {
        let indent = "  ".repeat(depth);

        match node {
            Node::Internal(internal) => {
                let keys: Vec<_> = internal.keys[..internal.len].iter().collect();
                println!("{}Internal: keys={:?}", indent, keys);

                for i in 0..internal.len {
                    if let Some(ref child) = internal.children[i] {
                        self.print_node(child, depth + 1);
                    }
                }
            }
            Node::Leaf(leaf) => {
                let keys: Vec<_> = leaf.keys[..leaf.len].iter().collect();
                println!("{}Leaf: keys={:?}", indent, keys);
            }
        }
    }
}

fn main() {
    println!("Hello, world!");

    let mut tree = BPTree::new();

    for i in 0..INTERNAL_NODE_MAX_KEYS * 20 {
        tree.insert(i, format!("value_{}", i));
    }

    tree.print_tree();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert() {
        let mut tree = BPTree::new();
        // Insert enough keys to force at least one split and create an internal root
        for i in 0..INTERNAL_NODE_MAX_KEYS * 2 {
            tree.insert(i, format!("value_{}", i));
        }

        let root = tree.root.as_ref().expect("root should exist");

        match root.as_ref() {
            Node::Internal(internal) => {
                assert_eq!(internal.len, 4);
                assert_eq!(internal.keys[..4], [0, 4, 8, 12]);
            }
            Node::Leaf(_) => {
                panic!("expected internal root after many inserts, but got leaf");
            }
        }
    }
}
