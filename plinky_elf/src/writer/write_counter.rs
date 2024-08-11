use std::io::Write;

pub(super) struct WriteCounter<'a> {
    inner: &'a mut dyn Write,
    pub(super) counter: usize,
}

impl<'a> WriteCounter<'a> {
    pub(super) fn new(inner: &'a mut dyn Write) -> Self {
        Self { inner, counter: 0 }
    }
}

impl Write for WriteCounter<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let written = self.inner.write(buf)?;
        self.counter += written;
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
