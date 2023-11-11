mod render_hex;

pub(crate) use self::render_hex::render_hex;

use std::io::{Read, Seek, Write};

pub trait ReadSeek: Read + Seek {}

impl<T: Read + Seek> ReadSeek for T {}

pub trait WriteSeek: Write + Seek {}

impl<T: Write + Seek> WriteSeek for T {}
