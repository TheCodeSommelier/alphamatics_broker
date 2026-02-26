use std::io::{Error, ErrorKind, Result};

// ===================
//       Cursor
// ===================

pub struct Cur<'a> {
    /// Backing buffer (borrowed).
    b: &'a [u8],
    /// Current cursor position (byte offset into `b`).
    i: usize,
}

impl<'a> Cur<'a> {
    /// Create a new cursor over `b`, starting at position 0.
    pub fn new(b: &'a [u8]) -> Self {
        Self { b, i: 0 }
    }

    /// Number of bytes remaining from the current position to the end.
    pub fn remaining(&self) -> usize {
        self.b.len().saturating_sub(self.i)
    }

    /// Ensure at least `n` bytes remain, otherwise return `UnexpectedEof`.
    pub fn need(&self, n: usize) -> Result<()> {
        if self.remaining() < n {
            return Err(Error::new(ErrorKind::UnexpectedEof, "not enough bytes"));
        }
        Ok(())
    }

    /// Take `n` bytes from the current position and advance the cursor by `n`.
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        self.need(n)?;
        let s = &self.b[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }

    /// Read a big-endian `u8` and advance by 1 byte.
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    /// Read a big-endian `u16` and advance by 2 bytes.
    pub fn u16(&mut self) -> Result<u16> {
        let s = self.take(2)?;
        Ok(u16::from_be_bytes([s[0], s[1]]))
    }

    /// Read a big-endian `u32` and advance by 4 bytes.
    pub fn u32(&mut self) -> Result<u32> {
        let s = self.take(4)?;
        Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }

    /// Read a big-endian `u64` and advance by 8 bytes.
    pub fn u64(&mut self) -> Result<u64> {
        let s = self.take(8)?;
        Ok(u64::from_be_bytes([
            s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
        ]))
    }

    /// Read a big-endian `i16` and advance by 2 bytes.
    pub fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }

    /// Read a big-endian `i32` and advance by 4 bytes.
    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
}
