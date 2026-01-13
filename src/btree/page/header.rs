use crate::error::{DbError, Result};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageType {
    Meta = 0,
    Internal = 1,
    Leaf = 2,
}

impl TryFrom<u8> for PageType {
    type Error = crate::error::DbError;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(PageType::Meta),
            1 => Ok(PageType::Internal),
            2 => Ok(PageType::Leaf),
            _ => Err(DbError::InvalidPageType(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageHeader {
    pub page_type: PageType,
    pub next_page_pointer: u64,
    pub prev_page_pointer: u64,
}

impl PageHeader {
    pub const SIZE: usize = 1 + 8 + 8; // 17 bytes

    pub fn new(page_type: PageType) -> Self {
        Self {
            page_type,
            next_page_pointer: 0,
            prev_page_pointer: 0,
        }
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u8(self.page_type as u8)?;
        writer.write_u64::<BigEndian>(self.next_page_pointer)?;
        writer.write_u64::<BigEndian>(self.prev_page_pointer)?;
        Ok(())
    }

    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        let page_type = PageType::try_from(reader.read_u8()?)?;
        let next_page_pointer = reader.read_u64::<BigEndian>()?;
        let prev_page_pointer = reader.read_u64::<BigEndian>()?;

        Ok(Self {
            page_type,
            next_page_pointer,
            prev_page_pointer,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_page_header_serialization() {
        let header = PageHeader {
            page_type: PageType::Internal,
            next_page_pointer: 1024,
            prev_page_pointer: 512,
        };

        let mut buffer = Vec::new();
        header.write_to(&mut buffer).unwrap();

        let mut cursor = Cursor::new(buffer);
        let deserialized = PageHeader::read_from(&mut cursor).unwrap();

        assert_eq!(header, deserialized);
    }

    #[test]
    fn test_invalid_page_type() {
        let mut buffer = vec![0u8; 17];
        buffer[0] = 255;
        let mut cursor = Cursor::new(buffer);
        let result = PageHeader::read_from(&mut cursor);
        assert!(result.is_err());
    }
}
