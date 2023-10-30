mod render_hex;

pub(crate) use self::render_hex::render_hex;

use std::io::{Read, Seek};

pub trait ReadSeek: Read + Seek {}

impl<T: Read + Seek> ReadSeek for T {}
