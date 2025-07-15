use core::convert::Infallible;

use axerrno::LinuxResult;

use crate::{Read, Write};

/// Read bytes from a buffer.
pub trait Buf {
    /// Returns the *minimum* number of bytes remaining in the buffer.
    fn remaining(&self) -> usize;

    /// Returns a slice starting at the current position and of length between
    /// `0` and `Buf::remaining()`.
    fn chunk(&self) -> &[u8];

    /// Advances the buffer by `n` bytes.
    fn advance(&mut self, n: usize);

    /// Reads the buffer with the given function, which is called repeatedly
    /// until it returns `0` or the buffer is exhausted.
    fn read_with<R>(&mut self, mut f: impl FnMut(&[u8]) -> Result<usize, R>) -> Result<usize, R> {
        let mut read = 0;
        loop {
            let d = self.chunk();
            if d.is_empty() {
                break;
            }

            let cnt = f(d)?;
            if cnt == 0 {
                break;
            }

            self.advance(cnt);
            read += cnt;
        }
        Ok(read)
    }

    /// Creates an adaptor which implements the `Read` trait for `self`.
    fn reader(self) -> Reader<Self>
    where
        Self: Sized,
    {
        Reader(self)
    }
}

/// A trait for values that provide sequential write access to bytes.
pub trait BufMut: Buf {
    /// Returns a mutable slice starting at the current position and of length
    /// between `0` and `Buf::remaining()`.
    fn chunk_mut(&mut self) -> &mut [u8];

    /// Fills the buffer with the given function, which is called repeatedly
    /// until it returns `0` or the buffer is exhausted.
    fn fill_with<R>(
        &mut self,
        mut f: impl FnMut(&mut [u8]) -> Result<usize, R>,
    ) -> Result<usize, R> {
        let mut written = 0;
        loop {
            let d = self.chunk_mut();
            if d.is_empty() {
                break;
            }

            let cnt = f(d)?;
            if cnt == 0 {
                break;
            }

            self.advance(cnt);
            written += cnt;
        }
        Ok(written)
    }

    /// Transfer bytes into self from src and advance the cursor by the number of bytes written.
    fn put(&mut self, src: &mut impl Buf) -> usize {
        self.fill_with::<Infallible>(|chunk| {
            let s = src.chunk();
            let cnt = usize::min(s.len(), chunk.len());
            if cnt == 0 {
                return Ok(0);
            }

            chunk[..cnt].copy_from_slice(&s[..cnt]);
            src.advance(cnt);
            Ok(cnt)
        })
        .unwrap_or_else(|err| match err {})
    }

    /// Creates an adaptor which implements the `Write` trait for `self`.
    fn writer(self) -> Writer<Self>
    where
        Self: Sized,
    {
        Writer(self)
    }
}

/// A `Buf` adapter which implements `Read` for the inner value.
pub struct Reader<B>(B);
impl<B: Buf> Read for Reader<B> {
    fn read(&mut self, mut buf: &mut [u8]) -> LinuxResult<usize> {
        Ok(buf.put(&mut self.0))
    }
}

/// A `BufMut` adapter which implements `Write` for the inner value.
pub struct Writer<B>(B);
impl<B: BufMut> Write for Writer<B> {
    fn write(&mut self, mut buf: &[u8]) -> LinuxResult<usize> {
        Ok(self.0.put(&mut buf))
    }

    fn flush(&mut self) -> LinuxResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::buf::{Buf, BufMut};

    #[test]
    fn test_buf() {
        let mut buf: &[u8] = b"hello world";
        assert_eq!(buf.remaining(), 11);
        assert_eq!(buf.chunk(), b"hello world");
        buf.advance(6);
        assert_eq!(buf.remaining(), 5);
        assert_eq!(buf.chunk(), b"world");
    }

    #[test]
    fn test_put() {
        let mut buf = [0; 5];
        let mut src: &[u8] = b"hello world";
        let written = buf.as_mut_slice().put(&mut src);
        assert_eq!(written, 5);
        assert_eq!(&buf[..written], b"hello");
        assert_eq!(src.remaining(), 6);
    }
}
