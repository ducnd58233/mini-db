const INTERNAL_NODE_MAX_KEYS: usize = 8;

struct Node {
    is_leaf: bool,
}

#[derive(Debug)]
struct BTreeInternalNode {
    nkey: usize,
    keys: [u64; INTERNAL_NODE_MAX_KEYS],
    children: [*mut Node; INTERNAL_NODE_MAX_KEYS],
}

impl BTreeInternalNode {
    fn new() -> Self {
        Self {
            nkey: 0,
            keys: [0; INTERNAL_NODE_MAX_KEYS],
            children: [std::ptr::null_mut(); INTERNAL_NODE_MAX_KEYS],
        }
    }

    // Find last position so that the key <= find_key
    fn find_insert_pos(&self, find_key: u64) -> usize {
        let mut pos = 0;

        while pos < self.nkey && self.keys[pos] <= find_key {
            pos += 1;
        }

        pos
    }

    fn insert_kv(&mut self, key: u64, value: *mut Node) {
        let pos = self.find_insert_pos(key);

        /*
        [1,4,7] -> insert 3 -> shift right [1,3,4,7]
        */
        for i in (pos..self.nkey).rev() {
            self.keys[i+1] = self.keys[i];
            self.children[i+1] = self.children[i];
        }

        self.keys[pos] = key;
        self.children[pos] = value;
        self.nkey += 1;
    }

    fn split(&mut self) -> BTreeInternalNode {
        let mut new_node = BTreeInternalNode::new();

        let pos = self.nkey / 2;

        /*
        [1,3,4,7] -> split -> [1,3] [4,7]
        */
        for i in pos..self.nkey {
            new_node.keys[i - pos] = self.keys[i];
            new_node.children[i - pos] = self.children[i];
            self.keys[i] = 0;
            self.children[i] = std::ptr::null_mut();
        }

        new_node.nkey = self.nkey - pos;
        self.nkey = pos;
        new_node
    }
}

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_insert_pos() {
        let mut node = BTreeInternalNode::new();
        node.keys = [1, 4, 7, 0, 0, 0, 0, 0];
        node.children = [std::ptr::null_mut(); INTERNAL_NODE_MAX_KEYS];
        node.nkey = 3;

        assert_eq!(node.find_insert_pos(3), 1);
        assert_eq!(node.find_insert_pos(4), 2);
        assert_eq!(node.find_insert_pos(5), 2);
    }

    #[test]
    fn test_insert_kv() {
        let mut node = BTreeInternalNode::new();
        node.insert_kv(3, std::ptr::null_mut());
        assert_eq!(node.keys[0], 3);
        assert_eq!(node.nkey, 1);

        node.insert_kv(10, std::ptr::null_mut());
        assert_eq!(node.keys[0], 3);
        assert_eq!(node.keys[1], 10);
        assert_eq!(node.nkey, 2);

        node.insert_kv(5, std::ptr::null_mut());
        assert_eq!(node.keys[0], 3);
        assert_eq!(node.keys[1], 5);
        assert_eq!(node.keys[2], 10);
        assert_eq!(node.nkey, 3);
    }

    #[test]
    fn test_split() {
        let mut node = BTreeInternalNode::new();
        node.keys = [1, 3, 4, 7, 0, 0, 0, 0];
        node.children = [std::ptr::null_mut(); INTERNAL_NODE_MAX_KEYS];
        node.nkey = 4;

        let new_node = node.split();
        assert_eq!(new_node.keys[0], 4);
        assert_eq!(new_node.keys[1], 7);
        assert_eq!(new_node.nkey, 2);
    }
}