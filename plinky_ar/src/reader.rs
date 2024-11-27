// Unfortunately there is no specification for the ar format anywhere. This is based on the
// Wikipedia page for the format [1] and the FreeBSD man page [2]
//
// [1]: https://en.wikipedia.org/wiki/Ar_(Unix)
// [2]: https://man.freebsd.org/cgi/man.cgi?query=ar&sektion=5

use crate::utils::{RawString, RawStringAsU64};
use crate::{ArFile, ArMemberId, ArSymbolTable};
use plinky_macros::{Display, Error, RawType};
use plinky_utils::raw_types::{RawReadError, RawType};
use plinky_utils::{Bits, Endian};
use std::collections::HashMap;
use std::io::{BufRead, Seek, SeekFrom};
use std::sync::atomic::{AtomicU64, Ordering};

const GNU_SYMBOL_TABLE_NAME: &str = "/";
const GNU_FILE_NAMES_NAME: &str = "//";

static NEXT_READER_SERIAL: AtomicU64 = AtomicU64::new(0);

pub struct ArReader<R: BufRead + Seek> {
    read: R,
    symbol_table: Option<ArSymbolTable>,
    gnu_file_names: Option<HashMap<u64, String>>,
    serial: u64,
}

impl<R: BufRead + Seek> ArReader<R> {
    pub fn new(read: R) -> Result<Self, ArReadError> {
        let mut reader = Self {
            read,
            symbol_table: None,
            gnu_file_names: None,
            serial: NEXT_READER_SERIAL.fetch_add(1, Ordering::Relaxed),
        };

        let magic: [u8; 8] = reader.read_raw()?;
        if &magic != b"!<arch>\n" {
            return Err(ArReadError::UnexpectedMagic(String::from_utf8_lossy(&magic).into()));
        }

        // The symbol table and the file names table must be located at the start of the file. We
        // read them at the constructor otherwise we risk them not being read when jumping to files
        // with read_member_by_id.
        if reader.peek_next_file_name()?.as_deref() == Some(GNU_SYMBOL_TABLE_NAME) {
            let (_header, content) = reader.read_raw_file()?;
            reader.symbol_table = Some(reader.read_gnu_symbol_table(&content)?);
        }
        if reader.peek_next_file_name()?.as_deref() == Some(GNU_FILE_NAMES_NAME) {
            let (_header, content) = reader.read_raw_file()?;
            reader.gnu_file_names = Some(reader.read_gnu_file_names(&content)?);
        }

        Ok(reader)
    }

    pub fn symbol_table(&self) -> Option<&ArSymbolTable> {
        self.symbol_table.as_ref()
    }

    pub fn read_member_by_id(&mut self, id: &ArMemberId) -> Result<ArFile, ArReadError> {
        if id.reader_serial != self.serial {
            panic!("passed an ArMemberId to a different ArReader than the one that generated it");
        }

        let old_position =
            self.read.stream_position().map_err(ArReadError::GetCurrentPositionFailed)?;
        self.seek(id.header_offset)?;
        let result = self.read_file();
        self.seek(old_position)?;

        result?.ok_or(ArReadError::InvalidReferenceInSymbolTable)
    }

    fn read_file(&mut self) -> Result<Option<ArFile>, ArReadError> {
        // Terminate if we reach the end of the file.
        if self.is_eof() {
            return Ok(None);
        }

        let (header, content) = self.read_raw_file()?;

        let raw_name = header.name.value.trim_end_matches(' ');
        let name = if raw_name == GNU_SYMBOL_TABLE_NAME {
            // GNU format, symbol table
            return Err(ArReadError::SymbolTableNotAtBeginning);
        } else if raw_name == GNU_FILE_NAMES_NAME {
            // GNU format, file names table
            return Err(ArReadError::FileNamesTableNotAtBeginning);
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

        Ok(Some(ArFile {
            name,
            content,
            modification_time: header.mtime.value,
            owner_id: header.uid.value,
            group_id: header.gid.value,
            mode: header.mode.value,
        }))
    }

    fn peek_next_file_name(&mut self) -> Result<Option<String>, ArReadError> {
        if self.is_eof() {
            return Ok(None);
        }

        let old_position =
            self.read.stream_position().map_err(ArReadError::GetCurrentPositionFailed)?;
        let header: RawHeader = self.read_raw()?;
        self.seek(old_position)?;

        Ok(Some(header.name.value.trim_end_matches(' ').into()))
    }

    fn read_gnu_file_names(&self, mut raw: &[u8]) -> Result<HashMap<u64, String>, ArReadError> {
        const SEPARATOR: &[u8] = b"/\n";

        let mut result = HashMap::new();
        let mut pos = 0;
        while !raw.is_empty() && raw != b"\n" {
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

    fn read_gnu_symbol_table(
        &self,
        mut raw: &[u8],
    ) -> Result<ArSymbolTable, ArSymbolTableReadError> {
        let count = u32::read(Bits::Bits64, Endian::Big, &mut raw)?;

        let mut offsets = Vec::new();
        for _ in 0..count {
            offsets.push(u32::read(Bits::Bits64, Endian::Big, &mut raw)?);
        }

        let mut symbols = HashMap::new();
        for offset in offsets {
            let Some(end) = raw.iter().position(|&c| c == 0) else {
                return Err(ArSymbolTableReadError::UnterminatedString);
            };

            let name = std::str::from_utf8(&raw[..end])
                .map_err(|_| ArSymbolTableReadError::NonUtf8SymbolName)?;
            raw = &raw[end + 1..];
            symbols.insert(
                name.to_string(),
                ArMemberId { reader_serial: self.serial, header_offset: offset as _ },
            );
        }

        if !raw.is_empty() && raw.iter().any(|&byte| byte != 0) {
            return Err(ArSymbolTableReadError::ExtraDataAtEnd);
        }

        Ok(ArSymbolTable { symbols })
    }

    fn read_raw_file(&mut self) -> Result<(RawHeader, Vec<u8>), ArReadError> {
        let header: RawHeader = self.read_raw()?;
        if header.end_magic != [b'`', b'\n'] {
            return Err(ArReadError::InvalidEndMagic(header.end_magic));
        }

        let mut content = vec![0; header.size.value as _];
        self.read.read_exact(&mut content).map_err(ArReadError::ContentReadFailed)?;

        self.align()?;

        Ok((header, content))
    }

    fn read_raw<T: RawType>(&mut self) -> Result<T, ArReadError> {
        // There are no types we need to read that depend on the bits of the processor, so we just
        // pick any of them to parse the raw types. The binary part of AR archives is also encoded
        // in big endian, so treat all raw data as big.
        Ok(T::read(Bits::Bits64, Endian::Big, &mut self.read)?)
    }

    fn align(&mut self) -> Result<(), ArReadError> {
        let pos = self.read.stream_position().map_err(ArReadError::GetCurrentPositionFailed)?;
        if pos % 2 == 1 {
            let mut buf = [0; 1];
            self.read.read_exact(&mut buf).map_err(ArReadError::AlignFailed)?;
            if buf[0] != b'\n' {
                return Err(ArReadError::InvalidAlignByte(buf[0]));
            }
        }

        Ok(())
    }

    fn seek(&mut self, position: u64) -> Result<(), ArReadError> {
        self.read
            .seek(SeekFrom::Start(position))
            .map_err(|e| ArReadError::SeekFailed(position, e))?;
        Ok(())
    }

    fn is_eof(&mut self) -> bool {
        self.read.fill_buf().map(|buf| buf.is_empty()).unwrap_or(false)
    }
}

impl<R: BufRead + Seek> Iterator for ArReader<R> {
    type Item = Result<ArFile, ArReadError>;

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

#[derive(Debug, Error, Display)]
pub enum ArReadError {
    #[transparent]
    Raw(RawReadError),
    #[display("failed to determine the current position in the file")]
    GetCurrentPositionFailed(#[source] std::io::Error),
    #[display("failed to seek to position {f0:#x} in the file")]
    SeekFailed(u64, #[source] std::io::Error),
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
    #[display("failed to read the symbol table")]
    SymbolTableReadFailed(#[from] ArSymbolTableReadError),
    #[display("invalid reference in symbol table")]
    InvalidReferenceInSymbolTable,
    #[display("the BSD ar format is not supported")]
    BsdFormatUnsupported,
    #[display("the symbol table is not at the beginning of the file")]
    SymbolTableNotAtBeginning,
    #[display("the file names table is not at the beginning of the file")]
    FileNamesTableNotAtBeginning,
}

#[derive(Debug, Error, Display)]
pub enum ArSymbolTableReadError {
    #[transparent]
    Raw(RawReadError),
    #[display("unterminated zero-terminated string")]
    UnterminatedString,
    #[display("non-utf-8 symbol name")]
    NonUtf8SymbolName,
    #[display("extra data was found at the end of the symbol table")]
    ExtraDataAtEnd,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    macro_rules! parse {
        ($name:expr_2021) => {
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
        assert_eq!((None, Vec::new()), parse!("empty.a").unwrap());
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
            (
                None,
                vec![ArFile {
                    name: "example.txt".into(),
                    content: b"hello\n".into(),
                    modification_time: 0,
                    owner_id: 0,
                    group_id: 0,
                    mode: 0o644,
                }]
            ),
            parse!("gnu-one-file.a").unwrap()
        );
    }

    #[test]
    fn test_gnu_multiple_files() {
        assert_eq!(
            (
                None,
                vec![
                    ArFile {
                        name: "unaligned-with-very-very-long-file-name.txt".into(),
                        content: b"unaligned body\n".into(),
                        modification_time: 0,
                        owner_id: 0,
                        group_id: 0,
                        mode: 0o644,
                    },
                    ArFile {
                        name: "aligned.txt".into(),
                        content: b"hello\n".into(),
                        modification_time: 0,
                        owner_id: 0,
                        group_id: 0,
                        mode: 0o644,
                    },
                    ArFile {
                        name: "also-aligned.txt".into(),
                        content: b"aligned\n".into(),
                        modification_time: 0,
                        owner_id: 0,
                        group_id: 0,
                        mode: 0o644,
                    },
                ]
            ),
            parse!("gnu-multiple-files.a").unwrap()
        );
    }

    #[test]
    fn test_metadata() {
        assert_eq!(
            (
                None,
                vec![ArFile {
                    name: "hello.txt".into(),
                    content: b"data\n".into(),
                    modification_time: 1703532181,
                    owner_id: 1000,
                    group_id: 1000,
                    mode: 0o100664
                }]
            ),
            parse!("metadata.a").unwrap()
        );
    }

    #[test]
    fn test_objects() {
        let mut content = Cursor::new(include_bytes!("../sample-archives/gnu-objects.a"));
        let mut reader = ArReader::new(&mut content).unwrap();

        let Some(table) = reader.symbol_table().cloned() else {
            panic!("first element is not the symbol table");
        };

        assert_eq!(3, table.symbols.len());
        assert_eq!(
            "foo.o",
            reader.read_member_by_id(table.symbols.get("hello").unwrap()).unwrap().name
        );
        assert_eq!(
            "bar.o",
            reader.read_member_by_id(table.symbols.get("goodbye").unwrap()).unwrap().name
        );
        assert_eq!(
            "bar.o",
            reader.read_member_by_id(table.symbols.get("world").unwrap()).unwrap().name
        );

        // Ensure the iterator continues to work after calling read_member_by_id.
        for expected_name in ["foo.o", "bar.o"] {
            let file = reader.next().unwrap().unwrap();
            assert_eq!(expected_name, file.name);
        }
        assert!(reader.next().is_none());
    }

    #[test]
    fn test_gnu_file_names_table_at_end() {
        // This tests both various errors that could occur with GNU-formatted archives, and that we
        // can continue the parsing if any archive member is invalid.

        let mut content =
            Cursor::new(include_bytes!("../sample-archives/gnu-file-names-table-at-end.a"));
        let mut reader = ArReader::new(&mut content).unwrap();

        match reader.next().unwrap().unwrap_err() {
            ArReadError::LongNameWithoutGnuFileNamesTable => {}
            other => panic!("expected LongNameWithoutGnuFileNamesTable error, found {other:?}"),
        }

        match reader.next().unwrap().unwrap_err() {
            ArReadError::FileNamesTableNotAtBeginning => {}
            other => panic!("expected DuplicateGnuFileNamesTable error, found {other:?}"),
        }

        assert!(reader.next().is_none());
    }

    #[test]
    fn test_gnu_wrong_file_name_refs() {
        // This tests both various errors that could occur with GNU-formatted archives, and that we
        // can continue the parsing if any archive member is invalid.

        let mut content =
            Cursor::new(include_bytes!("../sample-archives/gnu-wrong-file-name-refs.a"));
        let mut reader = ArReader::new(&mut content).unwrap();

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

    #[test]
    #[should_panic = "passed an ArMemberId to a different ArReader than the one that generated it"]
    fn test_mixing_armemberid_from_multiple_readers() {
        let mut content1 = Cursor::new(include_bytes!("../sample-archives/gnu-objects.a"));
        let mut content2 = Cursor::new(include_bytes!("../sample-archives/gnu-objects.a"));
        let reader1 = ArReader::new(&mut content1).unwrap();
        let mut reader2 = ArReader::new(&mut content2).unwrap();

        let Some(table1) = reader1.symbol_table() else {
            panic!("expected symbol table");
        };
        reader2.read_member_by_id(table1.symbols.get("hello").unwrap()).unwrap();
    }

    fn parse_archive(content: &[u8]) -> Result<(Option<ArSymbolTable>, Vec<ArFile>), ArReadError> {
        let mut cursor = Cursor::new(content);
        let reader = ArReader::new(&mut cursor)?;
        Ok((reader.symbol_table().cloned(), reader.collect::<Result<Vec<_>, ArReadError>>()?))
    }
}
