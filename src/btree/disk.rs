use super::page::{
    BTreeInternalPage, BTreeLeafPage, KeyEntry, KeyVal, PAGE_SIZE, PageHeader, PageType,
};
use crate::error::{DbError, Result};
use std::{
    fs::{File, OpenOptions},
    io::{Cursor, Read, Seek, SeekFrom, Write},
    path::Path,
};

#[derive(Debug, Clone)]
pub enum PageNode {
    Internal(BTreeInternalPage),
    Leaf(BTreeLeafPage),
}

trait PageWriter {
    fn write_to_buffer(&self, buffer: &mut Vec<u8>) -> Result<()>;
}

impl PageWriter for BTreeInternalPage {
    fn write_to_buffer(&self, buffer: &mut Vec<u8>) -> Result<()> {
        self.write_to(buffer)
    }
}

impl PageWriter for BTreeLeafPage {
    fn write_to_buffer(&self, buffer: &mut Vec<u8>) -> Result<()> {
        self.write_to(buffer)
    }
}

impl PageWriter for PageNode {
    fn write_to_buffer(&self, buffer: &mut Vec<u8>) -> Result<()> {
        match self {
            PageNode::Internal(page) => page.write_to_buffer(buffer),
            PageNode::Leaf(page) => page.write_to_buffer(buffer),
        }
    }
}

pub struct BPTreeDisk {
    file_path: String,
    file: File,
    next_free_block: u64,
}

impl BPTreeDisk {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file_path = path.as_ref().to_string_lossy().into_owned();

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&file_path)?;

        let file_size = file.metadata()?.len();
        let next_free_block = if file_size == 0 {
            1
        } else {
            (file_size / PAGE_SIZE as u64) + 1
        };

        let mut tree = Self {
            file_path,
            file,
            next_free_block,
        };

        if file_size == 0 {
            tree.initialize_root()?;
        }

        Ok(tree)
    }

    fn initialize_root(&mut self) -> Result<()> {
        let root = BTreeLeafPage::new();
        self.write_page_at_pointer(0, &root)?;
        Ok(())
    }

    fn read_block_at_pointer(&mut self, pointer: u64) -> Result<Vec<u8>> {
        let offset = pointer * PAGE_SIZE as u64;
        self.file.seek(SeekFrom::Start(offset))?;

        let mut buffer = vec![0u8; PAGE_SIZE];
        self.file.read_exact(&mut buffer)?;

        Ok(buffer)
    }

    fn write_block_at_pointer(&mut self, pointer: u64, data: &[u8]) -> Result<()> {
        if data.len() > PAGE_SIZE {
            return Err(DbError::InvalidFormat(format!(
                "Data size {} exceeds page size {}",
                data.len(),
                PAGE_SIZE,
            )));
        }
        let offset = pointer * PAGE_SIZE as u64;
        self.file.seek(SeekFrom::Start(offset))?;

        let mut buffer = vec![0u8; PAGE_SIZE];
        buffer[..data.len()].copy_from_slice(data);

        self.file.write_all(&buffer)?;
        self.file.sync_all()?;

        Ok(())
    }

    fn allocate_block(&mut self) -> u64 {
        let block = self.next_free_block;
        self.next_free_block += 1;
        block
    }

    fn write_page_at_pointer<P>(&mut self, pointer: u64, page: &P) -> Result<()>
    where
        P: PageWriter,
    {
        let mut buffer = Vec::new();
        page.write_to_buffer(&mut buffer)?;
        self.write_block_at_pointer(pointer, &buffer)
    }

    pub(crate) fn read_page_at_pointer(&mut self, pointer: u64) -> Result<PageNode> {
        let buffer = self.read_block_at_pointer(pointer)?;
        let mut cursor = Cursor::new(buffer);

        let header = PageHeader::read_from(&mut cursor)?;

        match header.page_type {
            PageType::Internal => {
                let page = BTreeInternalPage::read_from(&mut cursor, header)?;
                Ok(PageNode::Internal(page))
            }
            PageType::Leaf => {
                let page = BTreeLeafPage::read_from(&mut cursor, header)?;
                Ok(PageNode::Leaf(page))
            }
            PageType::Meta => Err(DbError::InvalidFormat(
                "Meta page not supported".to_string(),
            )),
        }
    }

    pub fn find(&mut self, key: &[u8]) -> Result<Option<KeyVal>> {
        self.find_recursive(0, key)
    }

    fn find_recursive(&mut self, pointer: u64, key: &[u8]) -> Result<Option<KeyVal>> {
        let node = self.read_page_at_pointer(pointer)?;

        match node {
            PageNode::Internal(ipage) => {
                let key_entry = KeyEntry::new(key)?;
                let pos = ipage.find_last_le(&key_entry).unwrap();

                if pos < ipage.children.len() {
                    self.find_recursive(ipage.children[pos], key)
                } else {
                    Ok(None)
                }
            }
            PageNode::Leaf(lpage) => {
                let pos = lpage.find_exact(key);
                Ok(pos.map(|i| lpage.kv[i]))
            }
        }
    }

    pub fn insert(&mut self, key: &[u8], val: &[u8]) -> Result<()> {
        let kv = KeyVal::new(key, val)?;
        let insert_result = self.insert_recursive(0, kv)?;

        if let Some(new_root_data) = insert_result {
            // Need to create new root
            let mut new_root = BTreeInternalPage::new();
            let old_root_pointer = 0u64;
            let new_node_pointer = self.allocate_block();

            // Write the new node
            self.write_page_at_pointer(new_node_pointer, &new_root_data)?;

            // Get first keys from both nodes
            let old_root = self.read_page_at_pointer(old_root_pointer)?;
            let old_key = match old_root {
                PageNode::Internal(ref page) => page.keys.first().cloned(),
                PageNode::Leaf(ref page) => page
                    .kv
                    .first()
                    .map(|kv| KeyEntry::new(kv.key_bytes()).expect("Key should be valid")),
            };

            let new_key = match new_root_data {
                PageNode::Internal(ref page) => page.keys.first().cloned(),
                PageNode::Leaf(ref page) => page
                    .kv
                    .first()
                    .map(|kv| KeyEntry::new(kv.key_bytes()).expect("Key should be valid")),
            };

            if let (Some(old_k), Some(new_k)) = (old_key, new_key) {
                new_root.insert_kv(old_k, old_root_pointer)?;
                new_root.insert_kv(new_k, new_node_pointer)?;

                // Move old root to new location
                let new_old_root_pointer = self.allocate_block();
                self.write_page_at_pointer(new_old_root_pointer, &old_root)?;
                new_root.children[0] = new_old_root_pointer;

                // Write new root at position 0
                self.write_page_at_pointer(0, &new_root)?;
            }
        }

        Ok(())
    }

    fn insert_recursive(&mut self, pointer: u64, kv: KeyVal) -> Result<Option<PageNode>> {
        let mut node = self.read_page_at_pointer(pointer)?;

        match node {
            PageNode::Internal(ref mut ipage) => {
                let key_entry = KeyEntry::new(kv.key_bytes())?;
                let pos = if ipage.nkey == 0 {
                    // First insertion - create first leaf
                    let mut first_leaf = BTreeLeafPage::new();
                    first_leaf.insert_kv(kv)?;
                    let leaf_pointer = self.allocate_block();
                    self.write_page_at_pointer(leaf_pointer, &first_leaf)?;

                    let first_key = KeyEntry::new(kv.key_bytes())?;
                    ipage.insert_kv(first_key, leaf_pointer)?;
                    self.write_page_at_pointer(pointer, ipage)?;
                    return Ok(None);
                } else {
                    ipage.find_last_le(&key_entry).unwrap_or(0)
                };

                if pos >= ipage.children.len() {
                    return Err(DbError::InvalidFormat("Invalid child position".to_string()));
                }

                let child_pointer = ipage.children[pos];
                let insert_result = self.insert_recursive(child_pointer, kv)?;

                // Update key for this position
                let child = self.read_page_at_pointer(child_pointer)?;
                if let Some(first_key) = get_first_key(&child) {
                    ipage.keys[pos] = first_key;
                }

                // Handle split if needed
                if let Some(new_child) = insert_result {
                    let new_child_pointer = self.allocate_block();
                    self.write_page_at_pointer(new_child_pointer, &new_child)?;

                    if let Some(new_key) = get_first_key(&new_child) {
                        ipage.insert_kv(new_key, new_child_pointer)?;
                    }
                }

                // Write updated internal page
                self.write_page_at_pointer(pointer, ipage)?;

                // Check if current node needs split
                if ipage.is_full() {
                    let new_ipage = ipage.split();
                    self.write_page_at_pointer(pointer, ipage)?;
                    Ok(Some(PageNode::Internal(new_ipage)))
                } else {
                    Ok(None)
                }
            }
            PageNode::Leaf(ref mut lpage) => {
                lpage.insert_kv(kv)?;
                self.write_page_at_pointer(pointer, lpage)?;

                if lpage.is_full() {
                    let new_lpage = lpage.split();
                    self.write_page_at_pointer(pointer, lpage)?;
                    Ok(Some(PageNode::Leaf(new_lpage)))
                } else {
                    Ok(None)
                }
            }
        }
    }

    /// Update an existing key
    pub fn set(&mut self, key: &[u8], val: &[u8]) -> Result<bool> {
        let kv = KeyVal::new(key, val)?;
        self.set_recursive(0, kv)
    }

    fn set_recursive(&mut self, pointer: u64, kv: KeyVal) -> Result<bool> {
        let mut node = self.read_page_at_pointer(pointer)?;

        match node {
            PageNode::Internal(ref ipage) => {
                let key_entry = KeyEntry::new(kv.key_bytes())?;
                let pos = ipage.find_last_le(&key_entry).unwrap_or(0);

                if pos < ipage.children.len() {
                    self.set_recursive(ipage.children[pos], kv)
                } else {
                    Ok(false)
                }
            }
            PageNode::Leaf(ref mut lpage) => {
                if let Some(pos) = lpage.find_exact(kv.key_bytes()) {
                    lpage.kv[pos] = kv;
                    self.write_page_at_pointer(pointer, lpage)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }

    /// Delete a key
    pub fn del(&mut self, key: &[u8]) -> Result<bool> {
        self.del_recursive(0, key)
    }

    fn del_recursive(&mut self, pointer: u64, key: &[u8]) -> Result<bool> {
        let mut node = self.read_page_at_pointer(pointer)?;

        match node {
            PageNode::Internal(ref ipage) => {
                let key_entry = KeyEntry::new(key)?;
                let pos = ipage.find_last_le(&key_entry).unwrap_or(0);

                if pos < ipage.children.len() {
                    self.del_recursive(ipage.children[pos], key)
                } else {
                    Ok(false)
                }
            }
            PageNode::Leaf(ref mut lpage) => {
                let deleted = lpage.del_key(key);
                if deleted {
                    self.write_page_at_pointer(pointer, lpage)?;
                }
                Ok(deleted)
            }
        }
    }

    /// Create an iterator starting at or after the given key
    pub fn seek_ge(&mut self, key: &[u8]) -> Result<super::iterator::BTreeIterator> {
        super::iterator::BTreeIterator::new(self, key)
    }
}

fn get_first_key(node: &PageNode) -> Option<KeyEntry> {
    match node {
        PageNode::Internal(page) => page.keys.first().cloned(),
        PageNode::Leaf(page) => page
            .kv
            .first()
            .map(|kv| KeyEntry::new(kv.key_bytes()).expect("Key should be valid")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_btree_insert_find() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut tree = BPTreeDisk::new(temp_file.path()).unwrap();

        let key = 42i64.to_be_bytes();
        let val = 100i64.to_be_bytes();

        tree.insert(&key, &val).unwrap();

        let result = tree.find(&key).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().val_bytes(), val);
    }

    #[test]
    fn test_btree_update() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut tree = BPTreeDisk::new(temp_file.path()).unwrap();

        let key = 42i64.to_be_bytes();
        let val1 = 100i64.to_be_bytes();
        let val2 = 200i64.to_be_bytes();

        tree.insert(&key, &val1).unwrap();
        tree.set(&key, &val2).unwrap();

        let result = tree.find(&key).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().val_bytes(), val2);
    }

    #[test]
    fn test_btree_delete() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut tree = BPTreeDisk::new(temp_file.path()).unwrap();

        let key = 42i64.to_be_bytes();
        let val = 100i64.to_be_bytes();

        tree.insert(&key, &val).unwrap();
        let deleted = tree.del(&key).unwrap();
        assert!(deleted);

        let result = tree.find(&key).unwrap();
        assert!(result.is_none());
    }
}
