use std::io::{Read, Write};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

use crate::error::{DbError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ValueType {
    Bytes = 1,
    Int64 = 2,
}

impl TryFrom<u8> for ValueType {
    type Error = DbError;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(ValueType::Bytes),
            2 => Ok(ValueType::Int64),
            _ => Err(DbError::InvalidFormat(format!(
                "Invalid value type: {}",
                value
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Bytes(Vec<u8>),
    Int64(i64),
}

impl Value {
    pub fn value_type(&self) -> ValueType {
        match self {
            Value::Bytes(_) => ValueType::Bytes,
            Value::Int64(_) => ValueType::Int64,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Bytes(b) => Some(b),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int64(i) => Some(*i),
            _ => None,
        }
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u8(self.value_type() as u8)?;

        match self {
            Value::Int64(val) => {
                writer.write_i64::<BigEndian>(*val)?;
            }
            Value::Bytes(val) => {
                writer.write_u8(val.len() as u8)?;
                writer.write_all(val)?;
            }
        }

        Ok(())
    }

    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        let value_type = ValueType::try_from(reader.read_u8()?)?;

        match value_type {
            ValueType::Int64 => {
                let val = reader.read_i64::<BigEndian>()?;
                Ok(Value::Int64(val))
            }
            ValueType::Bytes => {
                let len = reader.read_u8()?;
                let mut data = vec![0u8; len as usize];
                reader.read_exact(&mut data)?;
                Ok(Value::Bytes(data))
            }
        }
    }
}

impl From<i64> for Value {
    fn from(val: i64) -> Self {
        Value::Int64(val)
    }
}

impl From<Vec<u8>> for Value {
    fn from(val: Vec<u8>) -> Self {
        Value::Bytes(val)
    }
}

impl From<&[u8]> for Value {
    fn from(val: &[u8]) -> Self {
        Value::Bytes(val.to_vec())
    }
}

impl From<String> for Value {
    fn from(val: String) -> Self {
        Value::Bytes(val.into_bytes())
    }
}

impl From<&str> for Value {
    fn from(val: &str) -> Self {
        Value::Bytes(val.as_bytes().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_value_int64() {
        let val = Value::Int64(42);
        assert_eq!(val.value_type(), ValueType::Int64);
        assert_eq!(val.as_i64(), Some(42));
    }

    #[test]
    fn test_value_bytes() {
        let val = Value::Bytes(b"hello".to_vec());
        assert_eq!(val.value_type(), ValueType::Bytes);
        assert_eq!(val.as_bytes(), Some(b"hello".as_ref()));
    }

    #[test]
    fn test_value_serialization() {
        let val = Value::Int64(12345);
        let mut buffer = Vec::new();
        val.write_to(&mut buffer).unwrap();

        let mut cursor = Cursor::new(buffer);
        let deserialized = Value::read_from(&mut cursor).unwrap();

        assert_eq!(val, deserialized);
    }

    #[test]
    fn test_value_from_conversions() {
        let v1: Value = 42i64.into();
        assert_eq!(v1, Value::Int64(42));

        let v2: Value = "hello".into();
        assert_eq!(v2, Value::Bytes(b"hello".to_vec()));
    }
}
