use core::{
    mem::{ManuallyDrop, MaybeUninit},
    ptr,
};

use super::DEFAULT_BUF_SIZE;
use crate::{BufWrite, Result, Write};

/// The `BufWriter<W>` struct adds buffering to any writer.
pub struct BufWriter<W: Write> {
    inner: W,
    pos: usize,
    buf: [MaybeUninit<u8>; DEFAULT_BUF_SIZE],
}

impl<W: Write> BufWriter<W> {
    /// Creates a new `BufWriter<W>` with a default buffer capacity (1 KB).
    pub const fn new(inner: W) -> BufWriter<W> {
        Self {
            inner,
            pos: 0,
            buf: [const { MaybeUninit::uninit() }; DEFAULT_BUF_SIZE],
        }
    }

    /// Gets a reference to the underlying writer.
    pub const fn get_ref(&self) -> &W {
        &self.inner
    }

    /// Gets a mutable reference to the underlying writer.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }

    /// Returns a reference to the internally buffered data.
    pub fn buffer(&self) -> &[u8] {
        unsafe { self.buf[..self.pos].assume_init_ref() }
    }

    /// Returns the number of bytes the internal buffer can hold at once.
    pub const fn capacity(&self) -> usize {
        DEFAULT_BUF_SIZE
    }

    /// Returns the remaining spare capacity in the internal buffer.
    pub const fn spare_capacity(&self) -> usize {
        self.capacity() - self.pos
    }

    /// Unwraps this `BufWriter<W>`, returning the underlying writer.
    ///
    /// Any buffered data will be flushed before returning.
    pub fn into_inner(self) -> Result<W> {
        // Prevent Drop from running while we manually extract the inner writer.
        let mut this = ManuallyDrop::new(self);
        this.flush_buf()?;
        Ok(unsafe { ptr::read(&this.inner) })
    }
}

impl<W: Write> Write for BufWriter<W> {
    /// Writes a buffer into this writer, returning how many bytes were written.
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if self.spare_capacity() < buf.len() {
            self.flush_buf()?;
        }
        let written = buf.len().min(self.spare_capacity());
        unsafe {
            self.buf[self.pos..self.pos + written]
                .assume_init_mut()
                .copy_from_slice(&buf[..written]);
        }
        self.pos += written;
        Ok(written)
    }

    /// Flushes this writer, ensuring that all intermediately buffered contents reach their destination.
    fn flush(&mut self) -> Result<()> {
        self.flush_buf()?;
        self.inner.flush()
    }
}

impl<W: Write> BufWrite for BufWriter<W> {
    /// Flushes the internal buffer to the underlying writer.
    fn flush_buf(&mut self) -> Result<()> {
        if self.pos > 0 {
            self.inner
                .write_all(unsafe { self.buf[..self.pos].assume_init_ref() })?;
            self.pos = 0;
        }
        Ok(())
    }

    /// Skips a number of bytes in the internal buffer, flushing if necessary.
    fn skip_some(&mut self, len: usize) -> Result<()> {
        let mut sparce = self.spare_capacity();
        if sparce < len {
            self.flush_buf()?;
            sparce = self.spare_capacity();
        }
        self.pos += len.min(sparce);
        Ok(())
    }
}

/// Drops the `BufWriter`, flushing the internal buffer.
impl<W: Write> Drop for BufWriter<W> {
    fn drop(&mut self) {
        let _ = self.flush_buf();
    }
}
