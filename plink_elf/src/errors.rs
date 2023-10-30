use crate::ABI;

#[derive(Debug)]
pub enum LoadError {
    IO(std::io::Error),
    BadMagic([u8; 4]),
    BadClass(u8),
    BadEndian(u8),
    BadVersion(u32),
    BadAbi(u8),
    BadAbiVersion(ABI, u8),
    BadType(u16),
    BadMachine(u16),
    UnterminatedString,
    NonUtf8String(std::string::FromUtf8Error),
    MissingSectionNamesTable,
    WrongSectionNamesTableType,
    MissingSectionName(u32),
    MisalignedFile { current: usize, expected: usize },
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::IO(_) => f.write_str("I/O error"),
            LoadError::BadMagic(magic) => write!(f, "bad ELF magic number: {magic:?}"),
            LoadError::BadClass(class) => write!(f, "bad ELF class: {class}"),
            LoadError::BadEndian(endian) => write!(f, "bad ELF endianness: {endian}"),
            LoadError::BadVersion(version) => write!(f, "bad ELF version: {version}"),
            LoadError::BadAbi(abi) => write!(f, "bad ELF ABI: {abi}"),
            LoadError::BadAbiVersion(abi, version) => {
                write!(f, "bad ELF ABI for {abi:?}: {version}")
            }
            LoadError::BadType(type_) => write!(f, "bad ELF type: {type_}"),
            LoadError::BadMachine(machine) => write!(f, "bad ELF machine: {machine}"),
            LoadError::UnterminatedString => write!(f, "unterminated string"),
            LoadError::NonUtf8String(..) => write!(f, "non-UTF-8 string"),
            LoadError::MissingSectionNamesTable => {
                write!(f, "the table of section names is missing")
            }
            LoadError::WrongSectionNamesTableType => {
                write!(f, "the type of the section names table is not correct")
            }
            LoadError::MissingSectionName(id) => write!(f, "missing section name with id {id:#x}"),
            LoadError::MisalignedFile { current, expected } => write!(
                f,
                "misaligned file: parsed until {current:#x}, expected to be at {expected:#x}"
            ),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LoadError::IO(err) => Some(err),
            LoadError::NonUtf8String(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for LoadError {
    fn from(value: std::io::Error) -> Self {
        LoadError::IO(value)
    }
}

impl From<std::string::FromUtf8Error> for LoadError {
    fn from(value: std::string::FromUtf8Error) -> Self {
        LoadError::NonUtf8String(value)
    }
}
