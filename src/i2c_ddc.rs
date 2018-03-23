use std::thread::sleep;
use std::time::Duration;
use std::{iter, cmp, io, fmt, error};
use resize_slice::ResizeSlice;
use delay::Delay;
use i2c;
use {
    Edid, Eddc,
    DdcHost, DdcCommand, DdcCommandRaw,
    DdcCommandRawMarker, DdcCommandMarker,
    ErrorCode,
};

/// A handle to provide DDC/CI operations on an I2C device.
#[derive(Clone, Debug)]
pub struct I2cDdc<I> {
    inner: I,
    delay: Delay,
}

/// DDC/CI on Linux i2c-dev
#[cfg(feature = "i2c-linux")]
pub type I2cDeviceDdc = I2cDdc<::i2c_linux::I2c<::std::fs::File>>;

/// Open a new DDC/CI handle with the specified I2C device node path
#[cfg(feature = "i2c-linux")]
pub fn from_i2c_device<P: AsRef<::std::path::Path>>(p: P) -> ::std::io::Result<I2cDeviceDdc> {
    Ok(I2cDdc::new(::i2c_linux::I2c::from_path(p)?))
}

impl<I> I2cDdc<I> {
    /// Create a new DDC/CI handle with an existing open device.
    pub fn new(i2c: I) -> Self {
        I2cDdc {
            inner: i2c,
            delay: Default::default(),
        }
    }

    /// Consume the handle to return the inner device.
    pub fn into_inner(self) -> I {
        self.inner
    }

    /// Borrow the inner device.
    pub fn inner_ref(&self) -> &I {
        &self.inner
    }

    /// Mutably borrow the inner device.
    pub fn inner_mut(&mut self) -> &mut I {
        &mut self.inner
    }
}

impl<I: i2c::Address + i2c::BlockTransfer> Edid for I2cDdc<I> {
    type EdidError = I::Error;

    fn read_edid(&mut self, mut offset: u8, mut data: &mut [u8]) -> Result<usize, I::Error> {
        self.inner.set_slave_address(::I2C_ADDRESS_EDID, false)?;

        let mut len = 0;
        while !data.is_empty() {
            let datalen = cmp::min(0x80, data.len());
            let read = self.inner.i2c_read_block_data(offset, &mut data[..datalen])?;
            if read == 0 {
                break
            }
            len += read;
            offset = if let Some(offset) = offset.checked_add(read as u8) {
                offset
            } else {
                break
            };
            data.resize_from(read);
        }

        Ok(len)
    }
}

// TODO: these address/block bounds shouldn't be necessary, but might need
// specialization to impl Edid with BulkTransfer :<
impl<I: i2c::Address + i2c::BlockTransfer + i2c::BulkTransfer> Eddc for I2cDdc<I> {
    fn read_eddc_edid(&mut self, segment: u8, offset: u8, data: &mut [u8]) -> Result<usize, I::Error> {
        let len = {
            let mut msgs = [
                i2c::Message::Write {
                    address: ::I2C_ADDRESS_EDID_SEGMENT,
                    data: &[segment],
                    flags: Default::default(),
                },
                i2c::Message::Write {
                    address: ::I2C_ADDRESS_EDID,
                    data: &[offset],
                    flags: Default::default(),
                },
                i2c::Message::Read {
                    address: ::I2C_ADDRESS_EDID,
                    data: data,
                    flags: Default::default(),
                },
            ];
            self.inner.i2c_transfer(&mut msgs)?;
            msgs[2].len()
        };

        Ok(len)
    }
}

impl<I: i2c::Master> ::DdcHost for I2cDdc<I> {
    type Error = Error<I::Error>;

    fn sleep(&mut self) {
        self.delay.sleep()
    }
}

impl<I: i2c::Address + i2c::ReadWrite> DdcCommandRaw for I2cDdc<I> {
    fn execute_raw<'a>(&mut self, data: &[u8], out: &'a mut [u8], response_delay: Duration) -> Result<&'a mut [u8], Error<I::Error>> {
        assert!(data.len() <= 36);

        let mut packet = [0u8; 36 + 3];
        let packet = Self::encode_command(data, &mut packet);
        self.inner.set_slave_address(::I2C_ADDRESS_DDC_CI, false).map_err(Error::I2c)?;

        let full_len = {
            self.sleep();
            self.inner.i2c_write(packet).map_err(Error::I2c)?;
            if !out.is_empty() {
                sleep(response_delay);
                self.inner.i2c_read(out).map_err(Error::I2c)?
            } else {
                return Ok(out)
            }
        };

        if full_len < 2 {
            return Err(Error::Ddc(ErrorCode::InvalidLength))
        }

        let len = (out[1] & 0x7f) as usize;

        if out[1] & 0x80 == 0 {
            // TODO: apparently sometimes this isn't true?
            return Err(Error::Ddc(ErrorCode::Invalid("Expected DDC/CI length bit".into())))
        }

        if full_len < len + 2 {
            return Err(Error::Ddc(ErrorCode::InvalidLength))
        }

        let checksum = Self::checksum(
            iter::once(((::I2C_ADDRESS_DDC_CI as u8) << 1) | 1)
            .chain(iter::once(::SUB_ADDRESS_DDC_CI))
            .chain(out[1..2 + len].iter().cloned())
        );

        if out[2 + len] != checksum {
            return Err(Error::Ddc(ErrorCode::InvalidChecksum))
        }

        Ok(&mut out[2..2 + len])
    }
}

impl<I: i2c::Address + i2c::ReadWrite> DdcCommandMarker for I2cDdc<I> { }

impl<I: i2c::Address + i2c::ReadWrite> DdcCommandRawMarker for I2cDdc<I> {
    fn set_sleep_delay(&mut self, delay: Delay) {
        self.delay = delay;
    }
}

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

impl<I> From<ErrorCode> for Error<I> {
    fn from(e: ErrorCode) -> Self {
        Error::Ddc(e)
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
