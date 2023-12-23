// Unfortunately there is no specification for the ar format anywhere. This is based on the
// Wikipedia page for the format [1] and the FreeBSD man page [2]
//
// [1]: https://en.wikipedia.org/wiki/Ar_(Unix)
// [2]: https://man.freebsd.org/cgi/man.cgi?query=ar&sektion=5

use crate::ArchiveFile;
use plink_macros::{Display, Error, RawType};
use plink_rawutils::raw_types::{RawReadError, RawType, RawWriteError};
use plink_rawutils::Bits;
use std::collections::HashMap;
use std::io::{BufRead, Read};

pub struct ArReader<R: BufRead> {
    read: CountingRead<R>,
    gnu_file_names: Option<HashMap<u64, String>>,
}

impl<R: BufRead> ArReader<R> {
    pub fn new(read: R) -> Result<Self, ArReadError> {
        let mut reader = Self { read: CountingRead::new(read), gnu_file_names: None };

        let magic: [u8; 8] = reader.read_raw()?;
        if &magic != b"!<arch>\n" {
            return Err(ArReadError::UnexpectedMagic(String::from_utf8_lossy(&magic).into()));
        }

        Ok(reader)
    }

    fn read_file(&mut self) -> Result<Option<ArchiveFile>, ArReadError> {
        loop {
            // Terminate if we reach the end of the file.
            if self.read.fill_buf().map(|buf| buf.is_empty()).unwrap_or(false) {
                return Ok(None);
            }

            let header: RawHeader = self.read_raw()?;
            if header.end_magic != [b'`', b'\n'] {
                return Err(ArReadError::InvalidEndMagic(header.end_magic));
            }

            let mut content = vec![0; header.size.value as _];
            self.read.read_exact(&mut content).map_err(ArReadError::ContentReadFailed)?;

            self.align()?;

            let raw_name = header.name.value.trim_end_matches(' ');
            let name = if raw_name == "/" {
                // GNU format, symbol table
                raw_name.to_string()
            } else if raw_name == "//" {
                // GNU format, file names table
                match &self.gnu_file_names {
                    Some(_) => return Err(ArReadError::DuplicateGnuFileNamesTable),
                    None => {
                        self.gnu_file_names = Some(self.read_gnu_file_names(&content)?);
                        continue;
                    }
                }
            } else if let Some(name) = raw_name.strip_suffix('/') {
                // GNU format, name.len() <= 15
                name.to_string()
            } else if let Some(offset) = raw_name.strip_prefix('/') {
                // GNU format, name.len() > 15, offset in the string table
                let offset: u64 = offset
                    .parse()
                    .map_err(|e| ArReadError::InvalidOffsetForGnuLongName(offset.into(), e))?;
                self.gnu_file_names
                    .as_ref()
                    .ok_or(ArReadError::LongNameWithoutGnuFileNamesTable)?
                    .get(&offset)
                    .ok_or(ArReadError::MissingNameInGnuFileNamesTable(offset))?
                    .clone()
            } else if let Some(_len) = raw_name.strip_prefix("#1/") {
                // BSD format, lame.len() > 16, len is the size of name at the start of content
                return Err(ArReadError::BsdFormatUnsupported);
            } else {
                // BSD format, name.len() <= 16
                return Err(ArReadError::BsdFormatUnsupported);
            };

            return Ok(Some(ArchiveFile {
                name,
                content,
                modification_time: header.mtime.value,
                owner_id: header.uid.value,
                group_id: header.gid.value,
                mode: header.mode.value,
            }));
        }
    }

    fn read_gnu_file_names(&self, mut raw: &[u8]) -> Result<HashMap<u64, String>, ArReadError> {
        const SEPARATOR: &[u8] = b"/\n";

        let mut result = HashMap::new();
        let mut pos = 0;
        while !raw.is_empty() && raw != b"\n" {
            dbg!(std::str::from_utf8(raw).unwrap());
            match raw.windows(SEPARATOR.len()).position(|v| v == SEPARATOR) {
                Some(end) => {
                    let name = std::str::from_utf8(&raw[..end])
                        .map_err(|_| ArReadError::NonUtf8GnuFileNamesTable)?;
                    raw = &raw[end + SEPARATOR.len()..];
                    result.insert(pos, name.to_string());
                    pos += (name.len() + SEPARATOR.len()) as u64;
                }
                None => return Err(ArReadError::UnterminatedNameInGnuFileNamesTable),
            };
        }
        Ok(result)
    }

    fn read_raw<T: RawType>(&mut self) -> Result<T, ArReadError> {
        // There are no types we need to read that depend on the bits of the processor, so we just
        // pick any of them to parse the raw types.
        Ok(T::read(Bits::Bits64, &mut self.read)?)
    }

    fn align(&mut self) -> Result<(), ArReadError> {
        if self.read.count % 2 == 1 {
            let mut buf = [0; 1];
            self.read.read_exact(&mut buf).map_err(ArReadError::AlignFailed)?;
            if buf[0] != b'\n' {
                return Err(ArReadError::InvalidAlignByte(buf[0]));
            }
        }

        Ok(())
    }
}

impl<R: BufRead> Iterator for ArReader<R> {
    type Item = Result<ArchiveFile, ArReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.read_file().transpose()
    }
}

#[derive(RawType)]
struct RawHeader {
    name: RawString<16>,
    mtime: RawStringAsU64<12, 10>,
    uid: RawStringAsU64<6, 10>,
    gid: RawStringAsU64<6, 10>,
    mode: RawStringAsU64<8, 8>,
    size: RawStringAsU64<10, 10>,
    end_magic: [u8; 2],
}

struct RawString<const LEN: usize> {
    value: String,
}

impl<const LEN: usize> RawType for RawString<LEN> {
    fn zero() -> Self {
        Self { value: std::iter::repeat(' ').take(LEN).collect() }
    }

    fn size(_bits: impl Into<Bits>) -> usize {
        LEN
    }

    fn read(_bits: impl Into<Bits>, reader: &mut dyn std::io::Read) -> Result<Self, RawReadError> {
        let mut buf = [0; LEN];
        reader.read_exact(&mut buf).map_err(RawReadError::io::<Self>)?;
        Ok(Self {
            value: std::str::from_utf8(&buf)
                .map_err(|_| RawReadError::custom::<Self>("failed to decode string".into()))?
                .to_string(),
        })
    }

    fn write(
        &self,
        _bits: impl Into<Bits>,
        _writer: &mut dyn std::io::Write,
    ) -> Result<(), RawWriteError> {
        unimplemented!();
    }
}

struct RawStringAsU64<const LEN: usize, const RADIX: u32> {
    value: u64,
}

impl<const LEN: usize, const RADIX: u32> RawType for RawStringAsU64<LEN, RADIX> {
    fn zero() -> Self {
        Self { value: 0 }
    }

    fn size(bits: impl Into<Bits>) -> usize {
        RawString::<LEN>::size(bits)
    }

    fn read(bits: impl Into<Bits>, reader: &mut dyn std::io::Read) -> Result<Self, RawReadError> {
        let string = RawReadError::wrap_type::<Self, _>(RawString::<LEN>::read(bits, reader))?;
        let string = string.value.trim_end_matches(' ');
        if string.is_empty() {
            Ok(Self { value: 0 })
        } else {
            Ok(Self {
                value: u64::from_str_radix(string, RADIX).map_err(|_| {
                    RawReadError::custom::<Self>(format!("failed to parse number from {string:?}",))
                })?,
            })
        }
    }

    fn write(
        &self,
        _bits: impl Into<Bits>,
        _writer: &mut dyn std::io::Write,
    ) -> Result<(), RawWriteError> {
        unimplemented!();
    }
}

struct CountingRead<R: BufRead> {
    inner: R,
    count: usize,
}

impl<R: BufRead> CountingRead<R> {
    fn new(inner: R) -> Self {
        Self { inner, count: 0 }
    }
}

impl<R: BufRead> Read for CountingRead<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        match self.inner.read(buf) {
            Ok(len) => {
                self.count += len;
                Ok(len)
            }
            Err(err) => Err(err),
        }
    }
}

impl<R: BufRead> BufRead for CountingRead<R> {
    fn fill_buf(&mut self) -> Result<&[u8], std::io::Error> {
        self.inner.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.count += amt;
        self.inner.consume(amt);
    }
}

#[derive(Debug, Error, Display)]
pub enum ArReadError {
    #[transparent]
    Raw(RawReadError),
    #[display("unexpected magic value {f0:?}, is this an ar archive?")]
    UnexpectedMagic(String),
    #[display("failed to align the reader in preparation for the next item")]
    AlignFailed(#[source] std::io::Error),
    #[display("failed to read the content of a file inside the archive")]
    ContentReadFailed(#[source] std::io::Error),
    #[display("invalid byte {f0:#x} used for alignment")]
    InvalidAlignByte(u8),
    #[display("invalid magic value at the end of a file header: {f0:#x?}")]
    InvalidEndMagic([u8; 2]),
    #[display("the GNU file names table is present more than one time")]
    DuplicateGnuFileNamesTable,
    #[display("the GNU file names table doesn't contain UTF-8 text")]
    NonUtf8GnuFileNamesTable,
    #[display("unterminated file name in GNU file names table")]
    UnterminatedNameInGnuFileNamesTable,
    #[display("missing name in GNU file names table (offset: {f0})")]
    MissingNameInGnuFileNamesTable(u64),
    #[display("file has a long name but no GNU file names table was found")]
    LongNameWithoutGnuFileNamesTable,
    #[display("invalid offset for long GNU file name: {f0:?}")]
    InvalidOffsetForGnuLongName(String, #[source] std::num::ParseIntError),
    #[display("the BSD ar format is not supported")]
    BsdFormatUnsupported,
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! parse {
        ($name:expr) => {
            parse_archive(include_bytes!(concat!("../sample-archives/", $name)))
        };
    }

    #[test]
    fn test_wrong_magic() {
        match parse_archive(b"FOOBARBAZ").unwrap_err() {
            ArReadError::UnexpectedMagic(magic) if magic == "FOOBARBA" => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn test_wrong_end_magic() {
        let content = b"!<arch>\nexample.txt     0           0     0     644     6         NO";
        match parse_archive(content).unwrap_err() {
            ArReadError::InvalidEndMagic([b'N', b'O']) => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn test_empty() {
        assert_eq!(Vec::<ArchiveFile>::new(), parse!("empty.a").unwrap());
    }

    #[test]
    fn test_bsd_multiple_files() {
        match parse!("bsd-multiple-files.a").unwrap_err() {
            ArReadError::BsdFormatUnsupported => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn test_gnu_one_file() {
        assert_eq!(
            vec![ArchiveFile {
                name: "example.txt".into(),
                content: b"hello\n".into(),
                modification_time: 0,
                owner_id: 0,
                group_id: 0,
                mode: 0o644,
            }],
            parse!("gnu-one-file.a").unwrap()
        );
    }

    #[test]
    fn test_gnu_multiple_files() {
        assert_eq!(
            vec![
                ArchiveFile {
                    name: "unaligned-with-very-very-long-file-name.txt".into(),
                    content: b"unaligned body\n".into(),
                    modification_time: 0,
                    owner_id: 0,
                    group_id: 0,
                    mode: 0o644,
                },
                ArchiveFile {
                    name: "aligned.txt".into(),
                    content: b"hello\n".into(),
                    modification_time: 0,
                    owner_id: 0,
                    group_id: 0,
                    mode: 0o644,
                },
                ArchiveFile {
                    name: "also-aligned.txt".into(),
                    content: b"aligned\n".into(),
                    modification_time: 0,
                    owner_id: 0,
                    group_id: 0,
                    mode: 0o644,
                },
            ],
            parse!("gnu-multiple-files.a").unwrap()
        );
    }

    #[test]
    fn test_gnu_various_errors() {
        // This tests both various errors that could occur with GNU-formatted archives, and that we
        // can continue the parsing if any archive member is invalid.

        let mut content = include_bytes!("../sample-archives/gnu-various-errors.a") as &[u8];
        let mut reader = ArReader::new(&mut content).unwrap();

        match reader.next().unwrap().unwrap_err() {
            ArReadError::LongNameWithoutGnuFileNamesTable => {}
            other => panic!("expected LongNameWithoutGnuFileNamesTable error, found {other:?}"),
        }

        match reader.next().unwrap().unwrap_err() {
            ArReadError::DuplicateGnuFileNamesTable => {}
            other => panic!("expected DuplicateGnuFileNamesTable error, found {other:?}"),
        }

        match reader.next().unwrap().unwrap_err() {
            ArReadError::MissingNameInGnuFileNamesTable(1024) => {}
            other => panic!("expected MissingNameInGnuFileNamesTable(1024) error, found {other:?}"),
        }

        match reader.next().unwrap().unwrap_err() {
            ArReadError::InvalidOffsetForGnuLongName(offset, _) if offset == "not-a-number" => {}
            other => panic!(
                "expected InvalidOffsetForGnuLongName(\"not-a-number\") error, found {other:?}"
            ),
        }

        assert!(reader.next().is_none());
    }

    fn parse_archive(mut content: &[u8]) -> Result<Vec<ArchiveFile>, ArReadError> {
        ArReader::new(&mut content)?.collect()
    }
}
