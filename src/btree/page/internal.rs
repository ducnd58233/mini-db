use std::{
    cmp::Ordering,
    io::{Read, Write},
};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

use crate::{
    btree::header::{PageHeader, PageType},
    error::{DbError, Result},
};

pub const MAX_KEY_SIZE: usize = 12;
pub const INTERNAL_MAX_KEY: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEntry {
    len: u16,
    data: [u8; MAX_KEY_SIZE],
}

impl KeyEntry {
    pub fn new(input: &[u8]) -> Result<Self> {
        if input.len() > MAX_KEY_SIZE {
            return Err(DbError::KeySizeExceeded(input.len(), MAX_KEY_SIZE));
        }

        let mut data = [0u8; MAX_KEY_SIZE];
        let len = input.len();

        data[MAX_KEY_SIZE - len..].copy_from_slice(input);

        Ok(Self {
            len: len as u16,
            data,
        })
    }

    pub fn from_i64(value: i64) -> Self {
        let bytes = value.to_be_bytes();
        Self::new(&bytes).expect(&format!("i64 should always fit in {}", MAX_KEY_SIZE))
    }

    pub fn as_bytes(&self) -> &[u8] {
        let start = MAX_KEY_SIZE - self.len as usize;
        &self.data[start..]
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u16::<BigEndian>(self.len)?;
        let start = MAX_KEY_SIZE - self.len as usize;
        writer.write_all(&self.data[start..])?;
        Ok(())
    }

    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        let len = reader.read_u16::<BigEndian>()?;
        if len as usize > MAX_KEY_SIZE {
            return Err(DbError::InvalidFormat(format!(
                "Key length {} exceeds maximum {}",
                len, MAX_KEY_SIZE
            )));
        }

        let mut data = [0u8; MAX_KEY_SIZE];
        let start = MAX_KEY_SIZE - len as usize;
        reader.read_exact(&mut data[start..])?;

        Ok(Self { len, data })
    }
}

impl PartialOrd for KeyEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for KeyEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.data.cmp(&other.data)
    }
}

#[derive(Debug, Clone)]
pub struct BTreeInternalPage {
    pub header: PageHeader,
    pub nkey: u16,
    pub keys: Vec<KeyEntry>,
    pub children: Vec<u64>,
}

impl BTreeInternalPage {
    pub fn new() -> Self {
        Self {
            header: PageHeader::new(PageType::Internal),
            nkey: 0,
            keys: Vec::with_capacity(INTERNAL_MAX_KEY),
            children: Vec::with_capacity(INTERNAL_MAX_KEY),
        }
    }

    pub fn is_full(&self) -> bool {
        self.nkey as usize >= INTERNAL_MAX_KEY
    }

    /// Find last position where key <= find_key
    pub fn find_last_le(&self, find_key: &KeyEntry) -> Option<usize> {
        (0..self.nkey as usize)
            .rev()
            .find(|&i| self.keys[i] <= *find_key)
    }

    /// Insert a key-child pair into the internal node
    pub fn insert_kv(&mut self, insert_key: KeyEntry, insert_child_ptr: u64) -> Result<()> {
        if self.is_full() {
            return Err(DbError::NodeFull);
        }

        let pos = self.find_last_le(&insert_key).map(|p| p + 1).unwrap_or(0);

        self.keys.insert(pos, insert_key);
        self.children.insert(pos, insert_child_ptr);
        self.nkey += 1;

        Ok(())
    }

    /// Delete key-value at position
    pub fn del_kv_at_pos(&mut self, pos: usize) {
        if pos < self.keys.len() {
            self.keys.remove(pos);
            self.children.remove(pos);
            self.nkey -= 1;
        }
    }

    /// Split node into two equal parts
    pub fn split(&mut self) -> Self {
        let split_pos = self.nkey as usize / 2;

        let new_keys = self.keys.split_off(split_pos);
        let new_children = self.children.split_off(split_pos);
        let new_nkey = new_keys.len() as u16;

        self.nkey = split_pos as u16;

        Self {
            header: PageHeader::new(PageType::Internal),
            nkey: new_nkey,
            keys: new_keys,
            children: new_children,
        }
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        self.header.write_to(writer)?;
        writer.write_u16::<BigEndian>(self.nkey)?;

        for i in 0..self.nkey as usize {
            self.keys[i].write_to(writer)?;
        }

        for i in 0..self.nkey as usize {
            writer.write_u64::<BigEndian>(self.children[i])?;
        }

        Ok(())
    }

    pub fn read_from<R: Read>(reader: &mut R, header: PageHeader) -> Result<Self> {
        let nkey = reader.read_u16::<BigEndian>()?;

        let mut keys = Vec::with_capacity(nkey as usize);
        for _ in 0..nkey {
            keys.push(KeyEntry::read_from(reader)?);
        }

        let mut children = Vec::with_capacity(nkey as usize);
        for _ in 0..nkey {
            children.push(reader.read_u64::<BigEndian>()?);
        }

        Ok(Self {
            header,
            nkey,
            keys,
            children,
        })
    }
}

impl Default for BTreeInternalPage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_key_entry_ordering() {
        let key1 = KeyEntry::from_i64(3);
        let key2 = KeyEntry::from_i64(10);
        let key3 = KeyEntry::from_i64(5);

        assert!(key1 < key2);
        assert!(key3 < key2);
        assert!(key1 < key3);
    }

    #[test]
    fn test_internal_page_insert() {
        let mut node = BTreeInternalPage::new();

        node.insert_kv(KeyEntry::from_i64(3), 100).unwrap();
        assert_eq!(node.nkey, 1);
        assert_eq!(node.keys[0], KeyEntry::from_i64(3));

        node.insert_kv(KeyEntry::from_i64(10), 200).unwrap();
        assert_eq!(node.nkey, 2);
        assert_eq!(node.keys[1], KeyEntry::from_i64(10));

        node.insert_kv(KeyEntry::from_i64(5), 150).unwrap();
        assert_eq!(node.nkey, 3);
        assert_eq!(node.keys[0], KeyEntry::from_i64(3));
        assert_eq!(node.keys[1], KeyEntry::from_i64(5));
        assert_eq!(node.keys[2], KeyEntry::from_i64(10));
    }

    #[test]
    fn test_internal_page_split() {
        let mut node = BTreeInternalPage::new();

        node.insert_kv(KeyEntry::from_i64(3), 100).unwrap();
        node.insert_kv(KeyEntry::from_i64(5), 150).unwrap();
        node.insert_kv(KeyEntry::from_i64(10), 200).unwrap();
        node.insert_kv(KeyEntry::from_i64(12), 250).unwrap();

        let new_node = node.split();

        assert_eq!(node.nkey, 2);
        assert_eq!(new_node.nkey, 2);
        assert_eq!(node.keys[0], KeyEntry::from_i64(3));
        assert_eq!(node.keys[1], KeyEntry::from_i64(5));
        assert_eq!(new_node.keys[0], KeyEntry::from_i64(10));
        assert_eq!(new_node.keys[1], KeyEntry::from_i64(12));
    }

    #[test]
    fn test_internal_page_serialization() {
        let mut node = BTreeInternalPage::new();
        node.insert_kv(KeyEntry::from_i64(3), 100).unwrap();
        node.insert_kv(KeyEntry::from_i64(5), 150).unwrap();

        let mut buffer = Vec::new();
        node.write_to(&mut buffer).unwrap();

        let mut cursor = Cursor::new(buffer);
        let header = PageHeader::read_from(&mut cursor).unwrap();
        let deserialized = BTreeInternalPage::read_from(&mut cursor, header).unwrap();

        assert_eq!(node.nkey, deserialized.nkey);
        assert_eq!(node.keys, deserialized.keys);
        assert_eq!(node.children, deserialized.children);
    }
}
