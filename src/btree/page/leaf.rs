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
pub const MAX_VAL_SIZE: usize = 12;
pub const LEAF_MAX_KV: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyVal {
    pub keylen: u16,
    pub vallen: u16,
    pub key: [u8; MAX_KEY_SIZE],
    pub val: [u8; MAX_VAL_SIZE],
}

impl KeyVal {
    pub fn new(key: &[u8], val: &[u8]) -> Result<Self> {
        if key.len() > MAX_KEY_SIZE {
            return Err(DbError::KeySizeExceeded(key.len(), MAX_KEY_SIZE));
        }

        if val.len() > MAX_VAL_SIZE {
            return Err(DbError::ValueSizeExceed(val.len(), MAX_VAL_SIZE));
        }

        let mut key_data = [0u8; MAX_KEY_SIZE];
        let mut val_data = [0u8; MAX_KEY_SIZE];

        let key_len = key.len();
        let val_len = val.len();

        key_data[MAX_KEY_SIZE - key_len..].copy_from_slice(key);
        val_data[MAX_VAL_SIZE - val_len..].copy_from_slice(val);

        Ok(Self {
            keylen: key_len as u16,
            vallen: val_len as u16,
            key: key_data,
            val: val_data,
        })
    }

    pub fn from_i64(key: i64, val: i64) -> Self {
        let key_bytes = key.to_be_bytes();
        let val_bytes = val.to_be_bytes();
        Self::new(&key_bytes, &val_bytes).expect("i64 should always fit")
    }

    pub fn key_bytes(&self) -> &[u8] {
        let start = MAX_KEY_SIZE - self.keylen as usize;
        &self.key[start..]
    }

    pub fn val_bytes(&self) -> &[u8] {
        let start = MAX_VAL_SIZE - self.vallen as usize;
        &self.val[start..]
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u16::<BigEndian>(self.keylen)?;
        writer.write_u16::<BigEndian>(self.vallen)?;

        let key_start = MAX_KEY_SIZE - self.keylen as usize;
        writer.write_all(&self.key[key_start..])?;

        let val_start = MAX_VAL_SIZE - self.vallen as usize;
        writer.write_all(&self.val[val_start..])?;

        Ok(())
    }

    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        let keylen = reader.read_u16::<BigEndian>()?;
        let vallen = reader.read_u16::<BigEndian>()?;

        if keylen as usize > MAX_KEY_SIZE {
            return Err(DbError::InvalidFormat(format!(
                "Key length {} exceeds maximum {}",
                keylen, MAX_KEY_SIZE
            )));
        }
        if vallen as usize > MAX_VAL_SIZE {
            return Err(DbError::InvalidFormat(format!(
                "Value length {} exceeds maximum {}",
                vallen, MAX_VAL_SIZE
            )));
        }

        let mut key = [0u8; MAX_KEY_SIZE];
        let mut val = [0u8; MAX_VAL_SIZE];

        let key_start = MAX_KEY_SIZE - keylen as usize;
        reader.read_exact(&mut key[key_start..])?;

        let val_start = MAX_VAL_SIZE - vallen as usize;
        reader.read_exact(&mut val[val_start..])?;

        Ok(Self {
            keylen,
            vallen,
            key,
            val,
        })
    }
}

impl PartialOrd for KeyVal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for KeyVal {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key.cmp(&other.key)
    }
}

/// Leaf page node for B+ tree
#[derive(Debug, Clone)]
pub struct BTreeLeafPage {
    pub header: PageHeader,
    pub nkv: u16,
    pub kv: Vec<KeyVal>,
}

impl BTreeLeafPage {
    pub fn new() -> Self {
        Self {
            header: PageHeader::new(PageType::Leaf),
            nkv: 0,
            kv: Vec::with_capacity(LEAF_MAX_KV),
        }
    }

    pub fn is_full(&self) -> bool {
        self.nkv as usize >= LEAF_MAX_KV
    }

    /// Find last position where key <= find_key
    pub fn find_last_le(&self, find_kv: &KeyVal) -> Option<usize> {
        (0..self.nkv as usize)
            .rev()
            .find(|&i| self.kv[i] <= *find_kv)
    }

    /// Find exact key match
    pub fn find_exact(&self, key: &[u8]) -> Option<usize> {
        (0..self.nkv as usize).find(|&i| self.kv[i].key_bytes() == key)
    }

    /// Insert a key-value pair into the leaf node
    pub fn insert_kv(&mut self, insert_kv: KeyVal) -> Result<()> {
        if self.is_full() {
            return Err(DbError::NodeFull);
        }

        let pos = self.find_last_le(&insert_kv).map(|p| p + 1).unwrap_or(0);
        self.kv.insert(pos, insert_kv);
        self.nkv += 1;

        Ok(())
    }

    /// Delete a key-value pair from leaf node
    pub fn del_kv(&mut self, del_kv: &KeyVal) -> bool {
        if let Some(pos) = self.find_last_le(del_kv) {
            if self.kv[pos] == *del_kv {
                self.kv.remove(pos);
                self.nkv -= 1;
                return true;
            }
        }
        false
    }

    /// Split node into two equal parts
    pub fn split(&mut self) -> Self {
        let split_pos = self.nkv as usize / 2;

        let new_kv = self.kv.split_off(split_pos);
        let new_nkv = new_kv.len() as u16;

        self.nkv = split_pos as u16;

        Self {
            header: PageHeader::new(PageType::Leaf),
            nkv: new_nkv,
            kv: new_kv,
        }
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        self.header.write_to(writer)?;
        writer.write_u16::<BigEndian>(self.nkv)?;

        for i in 0..self.nkv as usize {
            self.kv[i].write_to(writer)?;
        }

        Ok(())
    }

    pub fn read_from<R: Read>(reader: &mut R, header: PageHeader) -> Result<Self> {
        let nkv = reader.read_u16::<BigEndian>()?;

        let mut kv = Vec::with_capacity(nkv as usize);
        for _ in 0..nkv {
            kv.push(KeyVal::read_from(reader)?);
        }

        Ok(Self { header, nkv, kv })
    }
}

impl Default for BTreeLeafPage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_keyval_creation() {
        let kv = KeyVal::from_i64(42, 100);
        assert_eq!(kv.keylen, 8);
        assert_eq!(kv.vallen, 8);
    }

    #[test]
    fn test_keyval_ordering() {
        let kv1 = KeyVal::from_i64(3, 100);
        let kv2 = KeyVal::from_i64(10, 200);
        let kv3 = KeyVal::from_i64(5, 150);

        assert!(kv1 < kv2);
        assert!(kv3 < kv2);
        assert!(kv1 < kv3);
    }

    #[test]
    fn test_leaf_page_insert() {
        let mut node = BTreeLeafPage::new();

        node.insert_kv(KeyVal::from_i64(3, 300)).unwrap();
        assert_eq!(node.nkv, 1);

        node.insert_kv(KeyVal::from_i64(10, 1000)).unwrap();
        assert_eq!(node.nkv, 2);

        node.insert_kv(KeyVal::from_i64(5, 500)).unwrap();
        assert_eq!(node.nkv, 3);

        // Check ordering
        assert_eq!(node.kv[0], KeyVal::from_i64(3, 300));
        assert_eq!(node.kv[1], KeyVal::from_i64(5, 500));
        assert_eq!(node.kv[2], KeyVal::from_i64(10, 1000));
    }

    #[test]
    fn test_leaf_page_delete() {
        let mut node = BTreeLeafPage::new();

        node.insert_kv(KeyVal::from_i64(3, 300)).unwrap();
        node.insert_kv(KeyVal::from_i64(5, 500)).unwrap();
        node.insert_kv(KeyVal::from_i64(10, 1000)).unwrap();

        let del_kv = KeyVal::from_i64(5, 500);
        let deleted = node.del_kv(&del_kv);

        assert!(deleted);
        assert_eq!(node.nkv, 2);
        assert_eq!(node.kv[0], KeyVal::from_i64(3, 300));
        assert_eq!(node.kv[1], KeyVal::from_i64(10, 1000));
    }

    #[test]
    fn test_leaf_page_split() {
        let mut node = BTreeLeafPage::new();

        node.insert_kv(KeyVal::from_i64(1, 100)).unwrap();
        node.insert_kv(KeyVal::from_i64(2, 200)).unwrap();
        node.insert_kv(KeyVal::from_i64(3, 300)).unwrap();
        node.insert_kv(KeyVal::from_i64(4, 400)).unwrap();

        let new_node = node.split();

        assert_eq!(node.nkv, 2);
        assert_eq!(new_node.nkv, 2);
        assert_eq!(node.kv[0], KeyVal::from_i64(1, 100));
        assert_eq!(node.kv[1], KeyVal::from_i64(2, 200));
        assert_eq!(new_node.kv[0], KeyVal::from_i64(3, 300));
        assert_eq!(new_node.kv[1], KeyVal::from_i64(4, 400));
    }

    #[test]
    fn test_leaf_page_serialization() {
        let mut node = BTreeLeafPage::new();
        node.insert_kv(KeyVal::from_i64(3, 300)).unwrap();
        node.insert_kv(KeyVal::from_i64(5, 500)).unwrap();

        let mut buffer = Vec::new();
        node.write_to(&mut buffer).unwrap();

        let mut cursor = Cursor::new(buffer);
        let header = PageHeader::read_from(&mut cursor).unwrap();
        let deserialized = BTreeLeafPage::read_from(&mut cursor, header).unwrap();

        assert_eq!(node.nkv, deserialized.nkv);
        assert_eq!(node.kv, deserialized.kv);
    }
}
