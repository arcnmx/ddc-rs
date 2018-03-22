#![deny(missing_docs)]
#![doc(html_root_url = "http://arcnmx.github.io/ddc-rs/")]

//! Control displays using the DDC/CI protocol.
//!
//! # Example
//!
//! ```rust,no_run
//! use ddc::{Ddc, commands};
//!
//! # #[cfg(feature = "i2c-linux")] fn ddc() {
//! let mut ddc = Ddc::from_path("/dev/i2c-4").unwrap();
//! let mccs_version = ddc.execute(commands::GetVcpFeature::new(0xdf)).unwrap();
//! println!("MCCS version: {:04x}", mccs_version.maximum());
//! # }
//! ```

extern crate resize_slice;
#[cfg(feature = "i2c-linux")]
extern crate i2c_linux;
extern crate i2c;

use std::thread::sleep;
use std::time::{Instant, Duration};
use std::{iter, cmp};
use resize_slice::ResizeSlice;

/// EDID EEPROM I2C address
pub const I2C_ADDRESS_EDID: u16 = 0x50;

/// E-DDC EDID segment register I2C address
pub const I2C_ADDRESS_EDID_SEGMENT: u16 = 0x30;

/// DDC/CI command and control I2C address
pub const I2C_ADDRESS_DDC_CI: u16 = 0x37;

/// DDC sub-address command prefix
pub const SUB_ADDRESS_DDC_CI: u8 = 0x51;

const DELAY_COMMAND_FAILED_MS: u64 = 40;

/// DDC/CI command request and response types.
pub mod commands;
pub use commands::{Command, CommandResult, VcpValue};

#[cfg(all(feature = "udev", feature = "i2c-linux"))]
mod enumerate;

#[cfg(all(feature = "udev", feature = "i2c-linux"))]
pub use enumerate::Enumerator;

mod error;
pub use error::{Error, ErrorCode};

/// A handle to provide DDC/CI operations on an I2C device.
#[derive(Clone, Debug)]
pub struct Ddc<I> {
    inner: I,
    delay: Delay,
}

#[cfg(feature = "i2c-linux")]
impl Ddc<i2c_linux::I2c<::std::fs::File>> {
    /// Open a new DDC/CI handle with the specified I2C device node path
    pub fn from_path<P: AsRef<::std::path::Path>>(p: P) -> ::std::io::Result<Self> {
        Ok(Ddc::new(i2c_linux::I2c::from_path(p)?))
    }
}

impl<I> Ddc<I> {
    /// Create a new DDC/CI handle with an existing open device.
    pub fn new(i2c: I) -> Self {
        Ddc {
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

    /// Wait for any previous commands to complete.
    ///
    /// The DDC specification defines delay intervals that must occur between
    /// execution of two subsequent commands, this waits for the amount of time
    /// remaining since the last command was executed. This is normally done
    /// internally and shouldn't need to be called manually unless synchronizing
    /// with an external process or another handle to the same device. It may
    /// however be desireable to run this before program exit.
    pub fn sleep(&mut self) {
        self.delay.sleep()
    }
}

impl<I: i2c::Address + i2c::BlockTransfer> Ddc<I> {
    /// Read up to 256 bytes of the monitor's EDID.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use ddc::Ddc;
    ///
    /// # #[cfg(feature = "i2c-linux")] fn ddc() {
    /// let mut ddc = Ddc::from_path("/dev/i2c-4").unwrap();
    /// let mut edid = [0u8; 0x100];
    /// ddc.read_edid(0, &mut edid[..]).unwrap();
    ///
    /// println!("EDID: {:?}", &edid[..]);
    /// # }
    /// ```
    pub fn read_edid(&mut self, mut offset: u8, mut data: &mut [u8]) -> Result<usize, I::Error> {
        self.inner.set_slave_address(I2C_ADDRESS_EDID, false)?;

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

impl<I: i2c::BulkTransfer> Ddc<I> {
    /// Read part of the EDID using the segments added in the Enhanced Display
    /// Data Channel (E-DDC) protocol.
    pub fn read_eddc_edid(&mut self, segment: u8, offset: u8, data: &mut [u8]) -> Result<usize, I::Error> {
        let len = {
            let mut msgs = [
                i2c::Message::Write {
                    address: I2C_ADDRESS_EDID_SEGMENT,
                    data: &[segment],
                    flags: Default::default(),
                },
                i2c::Message::Write {
                    address: I2C_ADDRESS_EDID,
                    data: &[offset],
                    flags: Default::default(),
                },
                i2c::Message::Read {
                    address: I2C_ADDRESS_EDID,
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

impl<I: i2c::Address + i2c::ReadWrite> Ddc<I> {
    /// Retrieve the capability string from the device.
    ///
    /// This executes multiple `CapabilitiesRequest` commands to construct the entire string.
    pub fn capabilities_string(&mut self) -> Result<Vec<u8>, Error<I::Error>> {
        let mut string = Vec::new();
        let mut offset = 0;
        loop {
            let caps = self.execute(commands::CapabilitiesRequest::new(offset))?;
            if caps.offset != offset {
                return Err(Error::Ddc(ErrorCode::InvalidOffset))
            } else if caps.data.is_empty() {
                break
            }

            string.extend(caps.data.iter());

            offset += caps.data.len() as u16;
        }

        Ok(string)
    }

    /// Read a table value from the device.
    pub fn table_read(&mut self, code: commands::FeatureCode) -> Result<Vec<u8>, Error<I::Error>> {
        let mut value = Vec::new();
        let mut offset = 0;
        loop {
            let table = self.execute(commands::TableRead::new(code, offset))?;
            if table.offset != offset {
                return Err(Error::Ddc(ErrorCode::InvalidOffset))
            } else if table.bytes().is_empty() {
                break
            }

            value.extend(table.bytes().iter());

            offset += table.bytes().len() as u16;
        }

        Ok(value)
    }

    /// Write a table value to the device.
    pub fn table_write(&mut self, code: commands::FeatureCode, value: &[u8]) -> Result<(), Error<I::Error>> {
        let mut offset = 0;
        for chunk in value.chunks(32) {
            self.execute(commands::TableWrite::new(code, offset, chunk))?;
            offset += chunk.len() as u16;
        }

        Ok(())
    }

    fn checksum<II: IntoIterator<Item=u8>>(iter: II) -> u8 {
        iter.into_iter().fold(0u8, |sum, v| sum ^ v)
    }

    /// Execute a DDC/CI command. See the `commands` module for all available
    /// commands. The return type is dependent on the executed command.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use ddc::{Ddc, commands};
    ///
    /// # #[cfg(feature = "i2c-linux")] fn ddc() {
    /// let mut ddc = Ddc::from_path("/dev/i2c-4").unwrap();
    /// let input = ddc.execute(commands::GetVcpFeature::new(0x60)).unwrap();
    /// println!("Monitor input: {:?}", input.value());
    /// # }
    ///
    /// ```
    pub fn execute<C: Command>(&mut self, command: C) -> Result<C::Ok, Error<I::Error>> {
        //let mut data = [0u8; C::MAX_LEN]; // TODO: once associated consts work...
        let mut data = [0u8; 36];
        command.encode(&mut data).map_err(Error::Ddc)?;

        //let mut out = [0u8; C::Ok::MAX_LEN + 3]; // TODO: once associated consts work...
        let mut out = [0u8; 36 + 3]; let out = &mut out[..C::Ok::MAX_LEN + 3];
        let res = self.execute_raw(
            &data[..command.len()],
            out,
            Duration::from_millis(C::DELAY_RESPONSE_MS as _)
        );
        let res = match res {
            Ok(res) => {
                self.delay = Delay::new(Duration::from_millis(C::DELAY_COMMAND_MS));
                res
            },
            Err(e) => {
                self.delay = Delay::new(Duration::from_millis(DELAY_COMMAND_FAILED_MS));
                return Err(e)
            },
        };

        let res = C::Ok::decode(res);

        if res.is_err() {
            self.delay = Delay::new(Duration::from_millis(DELAY_COMMAND_FAILED_MS));
        }

        res.map_err(Error::Ddc)
    }

    fn encode_command<'a>(data: &[u8], packet: &'a mut [u8]) -> &'a [u8] {
        packet[0] = SUB_ADDRESS_DDC_CI;
        packet[1] = 0x80 | data.len() as u8;
        packet[2..2 + data.len()].copy_from_slice(data);
        packet[2 + data.len()] = Self::checksum(
            iter::once((I2C_ADDRESS_DDC_CI as u8) << 1)
            .chain(packet[..2 + data.len()].iter().cloned())
        );

        &packet[..3 + data.len()]
    }

    fn execute_raw<'a>(&mut self, data: &[u8], out: &'a mut [u8], response_delay: Duration) -> Result<&'a mut [u8], Error<I::Error>> {
        assert!(data.len() <= 36);

        let mut packet = [0u8; 36 + 3];
        let packet = Self::encode_command(data, &mut packet);
        self.inner.set_slave_address(I2C_ADDRESS_DDC_CI, false)?;

        let full_len = {
            self.sleep();
            self.inner.i2c_write(packet)?;
            if !out.is_empty() {
                sleep(response_delay);
                self.inner.i2c_read(out)?
            } else {
                return Ok(out)
            }
        };

        if full_len < 2 {
            return Err(Error::Ddc(ErrorCode::InvalidLength.into()))
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
            iter::once(((I2C_ADDRESS_DDC_CI as u8) << 1) | 1)
            .chain(iter::once(SUB_ADDRESS_DDC_CI))
            .chain(out[1..2 + len].iter().cloned())
        );

        if out[2 + len] != checksum {
            return Err(Error::Ddc(ErrorCode::InvalidChecksum))
        }

        Ok(&mut out[2..2 + len])
    }
}

#[derive(Clone, Debug)]
struct Delay {
    time: Option<Instant>,
    delay: Duration,
}

impl Delay {
    fn new(delay: Duration) -> Self {
        Delay {
            time: Some(Instant::now()),
            delay: delay,
        }
    }

    fn sleep(&mut self) {
        if let Some(delay) = self.time.take().and_then(|time| self.delay.checked_sub(time.elapsed())) {
            sleep(delay);
        }
    }
}

impl Default for Delay {
    fn default() -> Self {
        Delay {
            time: None,
            delay: Default::default(),
        }
    }
}
