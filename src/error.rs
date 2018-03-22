use std::{io, fmt, error};

/// An error that can occur during DDC/CI communication.
///
/// This error is generic over the underlying I2C communication.
#[derive(Debug, Clone)]
pub enum Error<I> {
    /// Internal I2C communication error
    I2c(I),
    /// DDC/CI protocol error or transmission corruption
    Ddc(ErrorCode),
}

/// DDC/CI protocol errors
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ErrorCode {
    /// Expected matching offset from DDC/CI
    InvalidOffset,
    /// DDC/CI invalid packet length
    InvalidLength,
    /// Checksum mismatch
    InvalidChecksum,
    /// Expected opcode mismatch
    InvalidOpcode,
    /// Expected data mismatch
    InvalidData,
    /// Custom unspecified error
    Invalid(String),
}

impl<I> From<I> for Error<I> {
    fn from(e: I) -> Self {
        Error::I2c(e)
    }
}

impl<I: error::Error + Send + Sync + 'static> From<Error<I>> for io::Error {
    fn from(e: Error<I>) -> io::Error {
        match e {
            Error::I2c(e) => io::Error::new(io::ErrorKind::Other, e),
            Error::Ddc(e) => io::Error::new(io::ErrorKind::InvalidData, e),
        }
    }
}

impl<I: error::Error> error::Error for Error<I> {
    fn description(&self) -> &str {
        match *self {
            Error::I2c(ref e) => error::Error::description(e),
            Error::Ddc(ref e) => error::Error::description(e),
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            Error::I2c(ref e) => error::Error::cause(e),
            Error::Ddc(ref e) => error::Error::cause(e),
        }
    }
}

impl<I: fmt::Display> fmt::Display for Error<I> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::I2c(ref e) => write!(f, "DDC/CI I2C error: {}", e),
            Error::Ddc(ref e) => write!(f, "DDC/CI error: {}", e),
        }
    }
}

impl error::Error for ErrorCode {
    fn description(&self) -> &str {
        match *self {
            ErrorCode::InvalidOffset => "invalid offset returned from DDC/CI",
            ErrorCode::InvalidLength => "invalid DDC/CI length",
            ErrorCode::InvalidChecksum => "DDC/CI checksum mismatch",
            ErrorCode::InvalidOpcode => "DDC/CI VCP opcode mismatch",
            ErrorCode::InvalidData => "invalid DDC/CI data",
            ErrorCode::Invalid(ref s) => s,
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", error::Error::description(self))
    }
}
