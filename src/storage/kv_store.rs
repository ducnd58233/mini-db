use crate::{
    btree::{BPTreeDisk, BTreeIterator},
    error::Result,
};
use std::path::Path;

/// Iterator over key-value pairs
pub struct KVIterator<'a> {
    inner: BTreeIterator,
    tree: &'a mut BPTreeDisk,
}

impl<'a> KVIterator<'a> {
    /// Get current key-value pair
    pub fn current(&self) -> Option<(Vec<u8>, Vec<u8>)> {
        self.inner
            .deref()
            .map(|kv| (kv.key_bytes().to_vec(), kv.val_bytes().to_vec()))
    }

    /// Move to next entry
    pub fn next(&mut self) -> Result<bool> {
        self.inner.next(self.tree)
    }

    /// Check if exhausted
    pub fn is_exhausted(&self) -> bool {
        self.inner.is_exhausted()
    }
}

pub struct KVStore {
    tree: BPTreeDisk,
}

impl KVStore {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let tree = BPTreeDisk::new(path)?;
        Ok(Self { tree })
    }

    pub fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        match self.tree.find(key)? {
            Some(kv) => {
                let val_bytes = kv.val_bytes().to_vec();
                Ok(Some(val_bytes))
            }
            None => Ok(None),
        }
    }

    /// Get range of values from start_key to end_key (inclusive)
    pub fn get_range(&mut self, start_key: &[u8], end_key: &[u8]) -> Result<Vec<Vec<u8>>> {
        let mut results = Vec::new();
        let mut iter = self.tree.seek_ge(start_key)?;

        while let Some(kv) = iter.deref() {
            if kv.key_bytes() > end_key {
                break;
            }

            results.push(kv.val_bytes().to_vec());

            if !iter.next(&mut self.tree)? {
                break;
            }
        }

        Ok(results)
    }

    /// Set a key-value pair (insert or update)
    pub fn set(&mut self, key: &[u8], val: &[u8]) -> Result<()> {
        // Try to update first
        if !self.tree.set(key, val)? {
            // If update failed, insert
            self.tree.insert(key, val)?;
        }
        Ok(())
    }

    /// Delete a key
    pub fn del(&mut self, key: &[u8]) -> Result<bool> {
        self.tree.del(key)
    }

    /// Create an iterator starting at or after the given key
    pub fn iter_from(&mut self, start_key: &[u8]) -> Result<KVIterator> {
        let inner = self.tree.seek_ge(start_key)?;
        Ok(KVIterator {
            inner,
            tree: &mut self.tree,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_kvstore_basic_operations() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut store = KVStore::open(temp_file.path()).unwrap();

        let key = b"hello";
        let val = b"world";

        // Test set
        store.set(key, val).unwrap();

        // Test get
        let result = store.get(key).unwrap();
        assert_eq!(result, Some(val.to_vec()));

        // Test update
        let new_val = b"rust";
        store.set(key, new_val).unwrap();
        let result = store.get(key).unwrap();
        assert_eq!(result, Some(new_val.to_vec()));

        // Test delete
        let deleted = store.del(key).unwrap();
        assert!(deleted);

        let result = store.get(key).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_kvstore_range_query() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut store = KVStore::open(temp_file.path()).unwrap();

        // Insert sequential keys
        for i in 1..=10 {
            let key = format!("key{:02}", i);
            let val = format!("val{}", i);
            store.set(key.as_bytes(), val.as_bytes()).unwrap();
        }

        // Range query from key03 to key07
        let results = store.get_range(b"key03", b"key07").unwrap();
        assert_eq!(results.len(), 5); // key03, key04, key05, key06, key07
    }

    #[test]
    fn test_kvstore_iterator() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut store = KVStore::open(temp_file.path()).unwrap();

        // Insert test data
        for i in 1..=5 {
            let key = format!("key{}", i);
            let val = format!("val{}", i);
            store.set(key.as_bytes(), val.as_bytes()).unwrap();
        }

        // Test iteration
        let mut iter = store.iter_from(b"key1").unwrap();
        let mut count = 0;

        while let Some((key, val)) = iter.current() {
            count += 1;
            if !iter.next().unwrap() {
                break;
            }
        }

        assert_eq!(count, 5);
    }
}
