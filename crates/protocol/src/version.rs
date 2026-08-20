use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProtocolVersion {
    major: u32,
    minor: u32,
}
impl ProtocolVersion {
    pub const V1_0: Self = Self::new(1, 0);

    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }
    pub const fn major(&self) -> u32 {
        self.major
    }
    pub const fn minor(&self) -> u32 {
        self.minor
    }
}

impl fmt::Display for ProtocolVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.major, self.minor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseProtocolVersionError;
impl fmt::Display for ParseProtocolVersionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("invalid protocol version; expected canonical MAJOR.MINOR")
    }
}
impl std::error::Error for ParseProtocolVersionError {}
