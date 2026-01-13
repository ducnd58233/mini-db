use super::disk::{BPTreeDisk, PageNode};
use super::page::{KeyEntry, KeyVal};
use crate::error::Result;

/// Path element in the tree traversal
#[derive(Debug, Clone)]
struct PathData {
    pointer: u64,
    position: usize,
    node: PageNode,
}

/// Iterator over B+ tree entries
pub struct BTreeIterator {
    path: Vec<PathData>,
    exhausted: bool,
}

impl BTreeIterator {
    /// Create a new iterator starting at or after the given key
    pub fn new(tree: &mut BPTreeDisk, key: &[u8]) -> Result<Self> {
        let mut iterator = Self {
            path: Vec::new(),
            exhausted: false,
        };

        iterator.seek(tree, 0, key)?;

        Ok(iterator)
    }

    fn seek(&mut self, tree: &mut BPTreeDisk, pointer: u64, key: &[u8]) -> Result<()> {
        let node = tree.read_page_at_pointer(pointer)?;

        match &node {
            PageNode::Internal(ipage) => {
                let key_entry = KeyEntry::new(key)?;
                let pos = ipage.find_last_le(&key_entry).unwrap_or(0);

                self.path.push(PathData {
                    pointer,
                    position: pos,
                    node: node.clone(),
                });

                if pos < ipage.children.len() {
                    self.seek(tree, ipage.children[pos], key)?;
                }
            }
            PageNode::Leaf(lpage) => {
                // Find first key >= search key
                let pos = (0..lpage.nkv as usize)
                    .find(|&i| lpage.kv[i].key_bytes() >= key)
                    .unwrap_or(lpage.nkv as usize);

                self.path.push(PathData {
                    pointer,
                    position: pos,
                    node,
                });
            }
        }

        Ok(())
    }

    /// Get the current key-value pair
    pub fn deref(&self) -> Option<KeyVal> {
        if self.exhausted || self.path.is_empty() {
            return None;
        }

        let last = self.path.last()?;
        match &last.node {
            PageNode::Leaf(lpage) => {
                if last.position < lpage.kv.len() {
                    Some(lpage.kv[last.position])
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Move to the next key-value pair
    pub fn next(&mut self, tree: &mut BPTreeDisk) -> Result<bool> {
        if self.exhausted {
            return Ok(false);
        }

        // Pop up until we can move forward
        loop {
            if self.path.is_empty() {
                self.exhausted = true;
                return Ok(false);
            }

            let last_idx = self.path.len() - 1;
            let last = &self.path[last_idx];

            match &last.node {
                PageNode::Internal(ipage) => {
                    if ipage.nkey == 0 || last.position >= ipage.nkey as usize - 1 {
                        // Need to go up
                        self.path.pop();
                        continue;
                    } else {
                        // Can move forward at this level
                        break;
                    }
                }
                PageNode::Leaf(lpage) => {
                    if lpage.nkv == 0 || last.position >= lpage.nkv as usize - 1 {
                        // Need to go up
                        self.path.pop();
                        continue;
                    } else {
                        // Can move forward at this level
                        break;
                    }
                }
            }
        }

        // Move forward at current level
        let last_idx = self.path.len() - 1;
        self.path[last_idx].position += 1;

        // Navigate down if we're at an internal node
        let last = &self.path[last_idx];
        if let PageNode::Internal(ipage) = &last.node {
            let child_pointer = ipage.children[last.position];
            self.navigate_to_first_leaf(tree, child_pointer)?;
        }

        Ok(true)
    }

    fn navigate_to_first_leaf(&mut self, tree: &mut BPTreeDisk, pointer: u64) -> Result<()> {
        let node = tree.read_page_at_pointer(pointer)?;

        match &node {
            PageNode::Internal(ipage) => {
                self.path.push(PathData {
                    pointer,
                    position: 0,
                    node: node.clone(),
                });

                if !ipage.children.is_empty() {
                    self.navigate_to_first_leaf(tree, ipage.children[0])?;
                }
            }
            PageNode::Leaf(_) => {
                self.path.push(PathData {
                    pointer,
                    position: 0,
                    node,
                });
            }
        }

        Ok(())
    }

    /// Check if iterator is exhausted
    pub fn is_exhausted(&self) -> bool {
        self.exhausted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btree::disk::BPTreeDisk;
    use tempfile::NamedTempFile;

    #[test]
    fn test_iterator_basic() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut tree = BPTreeDisk::new(temp_file.path()).unwrap();

        // Insert test data
        for i in 1..=10 {
            let key = (i as i64).to_be_bytes();
            let val = (i as i64 * 10).to_be_bytes();
            tree.insert(&key, &val).unwrap();
        }

        // Test iteration
        let start_key = 1i64.to_be_bytes();
        let mut iter = BTreeIterator::new(&mut tree, &start_key).unwrap();

        let mut count = 0;
        while let Some(kv) = iter.deref() {
            count += 1;
            if !iter.next(&mut tree).unwrap() {
                break;
            }
        }

        assert_eq!(count, 10);
    }

    #[test]
    fn test_iterator_seek() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut tree = BPTreeDisk::new(temp_file.path()).unwrap();

        // Insert test data
        for i in 1..=10 {
            let key = (i as i64).to_be_bytes();
            let val = (i as i64 * 10).to_be_bytes();
            tree.insert(&key, &val).unwrap();
        }

        // Seek to middle
        let start_key = 5i64.to_be_bytes();
        let mut iter = BTreeIterator::new(&mut tree, &start_key).unwrap();

        let first = iter.deref().unwrap();
        assert_eq!(first.key_bytes(), start_key);

        let mut count = 1;
        while iter.next(&mut tree).unwrap() {
            count += 1;
        }

        assert_eq!(count, 6); // 5, 6, 7, 8, 9, 10
    }
}
