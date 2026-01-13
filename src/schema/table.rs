use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Cursor, Read, Write};

use super::record::{RangeRecord, Record};
use super::value::{Value, ValueType};
use crate::error::{DbError, Result};

/// Table definition
#[derive(Debug, Clone, PartialEq)]
pub struct TableDef {
    pub name: String,
    pub types: Vec<ValueType>,
    pub cols: Vec<String>,
    pub pkeys: u8,  // Number of columns that form the primary key
    pub prefix: u8, // Prefix for encoding keys
}

impl TableDef {
    pub fn new(name: impl Into<String>, prefix: u8) -> Self {
        Self {
            name: name.into(),
            types: Vec::new(),
            cols: Vec::new(),
            pkeys: 0,
            prefix,
        }
    }

    /// Add a column to the table definition
    pub fn add_column(mut self, name: impl Into<String>, typ: ValueType) -> Self {
        self.cols.push(name.into());
        self.types.push(typ);
        self
    }

    /// Set the number of primary key columns
    pub fn with_pkeys(mut self, pkeys: u8) -> Self {
        self.pkeys = pkeys;
        self
    }

    /// Encode key from values
    pub fn encode_key(&self, vals: &[Value]) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();

        // Write prefix
        buffer.write_u8(self.prefix)?;

        // Write number of values
        buffer.write_u8(vals.len() as u8)?;

        // Write each value
        for val in vals {
            val.write_to(&mut buffer)?;
        }

        Ok(buffer)
    }

    /// Decode values from key bytes
    pub fn decode_values(&self, data: &[u8]) -> Result<Vec<Value>> {
        let mut cursor = Cursor::new(data);

        // Read prefix (discard)
        let _prefix = cursor.read_u8()?;

        // Read number of values
        let n = cursor.read_u8()?;

        let mut values = Vec::new();
        for _ in 0..n {
            values.push(Value::read_from(&mut cursor)?);
        }

        Ok(values)
    }

    /// Check and reorder record to match table definition
    pub fn check_record(&self, rec: &mut Record) -> Result<()> {
        // Verify all primary key columns are present
        for i in 0..self.pkeys as usize {
            let col = &self.cols[i];
            if !rec.cols.contains(col) {
                return Err(DbError::InvalidRecord(format!(
                    "Missing primary key column: {}",
                    col
                )));
            }
        }

        // Reorder record to match table definition
        let mut new_cols = Vec::new();
        let mut new_vals = Vec::new();

        for col in &self.cols {
            if let Some(val) = rec.get(col).cloned() {
                new_cols.push(col.clone());
                new_vals.push(val);
            }
        }

        rec.cols = new_cols;
        rec.vals = new_vals;

        Ok(())
    }

    /// Serialize table definition
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();

        // Write name
        buffer.write_u8(self.name.len() as u8)?;
        buffer.write_all(self.name.as_bytes())?;

        // Write prefix
        buffer.write_u8(self.prefix)?;

        // Write pkeys
        buffer.write_u8(self.pkeys)?;

        // Write number of columns
        buffer.write_u8(self.cols.len() as u8)?;

        // Write columns and types
        for (col, typ) in self.cols.iter().zip(&self.types) {
            buffer.write_u8(col.len() as u8)?;
            buffer.write_all(col.as_bytes())?;
            buffer.write_u8(*typ as u8)?;
        }

        Ok(buffer)
    }

    /// Deserialize table definition
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);

        // Read name
        let name_len = cursor.read_u8()?;
        let mut name_bytes = vec![0u8; name_len as usize];
        cursor.read_exact(&mut name_bytes)?;
        let name = String::from_utf8(name_bytes)
            .map_err(|e| DbError::InvalidFormat(format!("Invalid UTF-8: {}", e)))?;

        // Read prefix
        let prefix = cursor.read_u8()?;

        // Read pkeys
        let pkeys = cursor.read_u8()?;

        // Read number of columns
        let n_cols = cursor.read_u8()?;

        let mut cols = Vec::new();
        let mut types = Vec::new();

        // Read columns and types
        for _ in 0..n_cols {
            let col_len = cursor.read_u8()?;
            let mut col_bytes = vec![0u8; col_len as usize];
            cursor.read_exact(&mut col_bytes)?;
            let col = String::from_utf8(col_bytes)
                .map_err(|e| DbError::InvalidFormat(format!("Invalid UTF-8: {}", e)))?;

            let typ = ValueType::try_from(cursor.read_u8()?)?;

            cols.push(col);
            types.push(typ);
        }

        Ok(Self {
            name,
            types,
            cols,
            pkeys,
            prefix,
        })
    }
}

/// Internal metadata table definition
pub const TDEF_META: TableDef = TableDef {
    name: String::new(), // Will be set to "@meta" in runtime
    types: Vec::new(),
    cols: Vec::new(),
    pkeys: 2,
    prefix: 1,
};

/// Internal table catalog definition  
pub const TDEF_TABLE: TableDef = TableDef {
    name: String::new(), // Will be set to "@table" in runtime
    types: Vec::new(),
    cols: Vec::new(),
    pkeys: 2,
    prefix: 2,
};

/// Get metadata table definition
pub fn get_meta_tdef() -> TableDef {
    TableDef::new("@meta", 1)
        .add_column("key", ValueType::Bytes)
        .add_column("value", ValueType::Bytes)
        .with_pkeys(2)
}

/// Get table catalog definition
pub fn get_table_tdef() -> TableDef {
    TableDef::new("@table", 2)
        .add_column("name", ValueType::Bytes)
        .add_column("def", ValueType::Bytes)
        .with_pkeys(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_def_creation() {
        let tdef = TableDef::new("users", 10)
            .add_column("name", ValueType::Bytes)
            .add_column("age", ValueType::Int64)
            .with_pkeys(1);

        assert_eq!(tdef.name, "users");
        assert_eq!(tdef.cols.len(), 2);
        assert_eq!(tdef.pkeys, 1);
        assert_eq!(tdef.prefix, 10);
    }

    #[test]
    fn test_encode_decode_key() {
        let tdef = TableDef::new("test", 5)
            .add_column("id", ValueType::Int64)
            .add_column("name", ValueType::Bytes);

        let vals = vec![Value::Int64(42), Value::Bytes(b"test".to_vec())];
        let encoded = tdef.encode_key(&vals).unwrap();

        let decoded = tdef.decode_values(&encoded).unwrap();
        assert_eq!(vals, decoded);
    }

    #[test]
    fn test_serialize_deserialize() {
        let tdef = TableDef::new("users", 10)
            .add_column("name", ValueType::Bytes)
            .add_column("age", ValueType::Int64)
            .with_pkeys(1);

        let serialized = tdef.serialize().unwrap();
        let deserialized = TableDef::deserialize(&serialized).unwrap();

        assert_eq!(tdef, deserialized);
    }

    #[test]
    fn test_check_record() {
        let mut tdef = TableDef::new("users", 10)
            .add_column("name", ValueType::Bytes)
            .add_column("age", ValueType::Int64)
            .with_pkeys(1);

        let mut rec = Record::new()
            .add_int64("age", 30)
            .add_str("name", b"Alice".to_vec());

        tdef.check_record(&mut rec).unwrap();

        // After reordering, name should come first
        assert_eq!(rec.cols[0], "name");
        assert_eq!(rec.cols[1], "age");
    }
}
