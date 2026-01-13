use super::value::Value;
use crate::error::{DbError, Result};

/// A record represents a row in a table
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub cols: Vec<String>,
    pub vals: Vec<Value>,
}

impl Record {
    pub fn new() -> Self {
        Self {
            cols: Vec::new(),
            vals: Vec::new(),
        }
    }

    /// Add a string column
    pub fn add_str(mut self, col: impl Into<String>, val: impl Into<Vec<u8>>) -> Self {
        self.cols.push(col.into());
        self.vals.push(Value::Bytes(val.into()));
        self
    }

    /// Add an int64 column
    pub fn add_int64(mut self, col: impl Into<String>, val: i64) -> Self {
        self.cols.push(col.into());
        self.vals.push(Value::Int64(val));
        self
    }

    /// Add a value directly
    pub fn add(mut self, col: impl Into<String>, val: Value) -> Self {
        self.cols.push(col.into());
        self.vals.push(val);
        self
    }

    /// Get value by column name
    pub fn get(&self, col: &str) -> Option<&Value> {
        self.cols
            .iter()
            .position(|c| c == col)
            .and_then(|idx| self.vals.get(idx))
    }

    /// Get mutable value by column name
    pub fn get_mut(&mut self, col: &str) -> Option<&mut Value> {
        self.cols
            .iter()
            .position(|c| c == col)
            .and_then(|idx| self.vals.get_mut(idx))
    }

    /// Check if record is valid (same number of columns and values)
    pub fn is_valid(&self) -> bool {
        self.cols.len() == self.vals.len()
    }

    /// Get number of columns
    pub fn len(&self) -> usize {
        self.cols.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.cols.is_empty()
    }
}

impl Default for Record {
    fn default() -> Self {
        Self::new()
    }
}

/// A range record for range queries
#[derive(Debug, Clone, PartialEq)]
pub struct RangeRecord {
    pub cols: Vec<String>,
    pub val_starts: Vec<Value>,
    pub val_ends: Vec<Value>,
}

impl RangeRecord {
    pub fn new() -> Self {
        Self {
            cols: Vec::new(),
            val_starts: Vec::new(),
            val_ends: Vec::new(),
        }
    }

    /// Add a range for a column
    pub fn add_range(mut self, col: impl Into<String>, start: Value, end: Value) -> Self {
        self.cols.push(col.into());
        self.val_starts.push(start);
        self.val_ends.push(end);
        self
    }

    /// Add an int64 range
    pub fn add_int64_range(self, col: impl Into<String>, start: i64, end: i64) -> Self {
        self.add_range(col, Value::Int64(start), Value::Int64(end))
    }

    /// Check if range record is valid
    pub fn is_valid(&self) -> bool {
        self.cols.len() == self.val_starts.len() && self.cols.len() == self.val_ends.len()
    }
}

impl Default for RangeRecord {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_creation() {
        let rec = Record::new()
            .add_str("name", b"Alice".to_vec())
            .add_int64("age", 30);

        assert_eq!(rec.len(), 2);
        assert!(rec.is_valid());
        assert_eq!(rec.get("name"), Some(&Value::Bytes(b"Alice".to_vec())));
        assert_eq!(rec.get("age"), Some(&Value::Int64(30)));
    }

    #[test]
    fn test_record_get() {
        let rec = Record::new()
            .add_str("key", b"value".to_vec())
            .add_int64("count", 42);

        assert_eq!(rec.get("key"), Some(&Value::Bytes(b"value".to_vec())));
        assert_eq!(rec.get("count"), Some(&Value::Int64(42)));
        assert_eq!(rec.get("nonexistent"), None);
    }

    #[test]
    fn test_range_record() {
        let range = RangeRecord::new().add_int64_range("age", 20, 30);

        assert!(range.is_valid());
        assert_eq!(range.cols, vec!["age"]);
        assert_eq!(range.val_starts, vec![Value::Int64(20)]);
        assert_eq!(range.val_ends, vec![Value::Int64(30)]);
    }
}
