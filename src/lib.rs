#![deny(missing_docs)]
#![doc(html_root_url = "http://arcnmx.github.io/ddc-rs/")]

//! Control displays using the DDC/CI protocol.
//!
//! # Example
//!
//! ```rust,no_run
//! use ddc::{Ddc, commands};
//!
//! let mut ddc = Ddc::from_path("/dev/i2c-4").unwrap();
//! let mccs_version = ddc.execute(commands::GetVcpFeature::new(0xdf)).unwrap();
//! println!("MCCS version: {:04x}", mccs_version.value());
//! ```

extern crate i2c_linux as i2c;

use std::thread::sleep;
use std::time::{Instant, Duration};
use std::path::Path;
use std::os::unix::io::AsRawFd;
use std::fs::File;
use std::{io, iter};
use std::io::{Read, Write};
use i2c::I2c;

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

#[cfg(feature = "udev")]
mod enumerate;

#[cfg(feature = "udev")]
pub use enumerate::Enumerator;

/// A handle to provide DDC/CI operations on an I2C device.
#[derive(Clone, Debug)]
pub struct Ddc<I> {
    inner: I,
    delay: Delay,
}

impl Ddc<I2c<File>> {
    /// Open a new DDC/CI handle with the specified I2C device node path
    pub fn from_path<P: AsRef<Path>>(p: P) -> io::Result<Self> {
        Ok(Ddc::new(I2c::from_path(p)?))
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
}

impl<I: AsRawFd + Read + Write> Ddc<I2c<I>> {
    /// Read part of the EDID using the segments added in the Enhanced Display
    /// Data Channel (E-DDC) protocol.
    pub fn read_eddc_edid(&mut self, segment: u8, offset: u8, data: &mut [u8]) -> io::Result<usize> {
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

    /// Read part of the monitor's EDID. The full 256 bytes must be read in two
    /// segments.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use ddc::Ddc;
    ///
    /// let mut ddc = Ddc::from_path("/dev/i2c-4").unwrap();
    /// let mut edid = [0u8; 0x100];
    /// ddc.read_edid(0, &mut edid[..0x80]).unwrap();
    /// ddc.read_edid(0x80, &mut edid[0x80..]).unwrap();
    ///
    /// println!("EDID: {:?}", &edid[..]);
    /// ```
    pub fn read_edid(&mut self, offset: u8, data: &mut [u8]) -> io::Result<usize> {
        let len = {
            let mut msgs = [
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
            msgs[1].len()
        };

        Ok(len)
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

    /// Retrieve the capability string from the device.
    ///
    /// This executes multiple `CapabilitiesRequest` commands to construct the entire string.
    pub fn capabilities_string(&mut self) -> io::Result<Vec<u8>> {
        let mut string = Vec::new();
        let mut offset = 0;
        loop {
            let caps = self.execute(commands::CapabilitiesRequest::new(offset))?;
            if caps.offset != offset {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "expected matching offset"))
            } else if caps.data.is_empty() {
                break
            }

            string.extend(caps.data.iter());

            offset += caps.data.len() as u16;
        }

        Ok(string)
    }

    /// Read a table value from the device.
    pub fn table_read(&mut self, code: commands::FeatureCode) -> io::Result<Vec<u8>> {
        let mut value = Vec::new();
        let mut offset = 0;
        loop {
            let table = self.execute(commands::TableRead::new(code, offset))?;
            if table.offset != offset {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "expected matching offset"))
            } else if table.bytes().is_empty() {
                break
            }

            value.extend(table.bytes().iter());

            offset += table.bytes().len() as u16;
        }

        Ok(value)
    }

    /// Write a table value to the device.
    pub fn table_write(&mut self, code: commands::FeatureCode, value: &[u8]) -> io::Result<()> {
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
    /// let mut ddc = Ddc::from_path("/dev/i2c-4").unwrap();
    /// let input = ddc.execute(commands::GetVcpFeature::new(0x60)).unwrap();
    /// println!("Monitor input: {:?}", input.value());
    ///
    /// ```
    pub fn execute<C: Command>(&mut self, command: C) -> io::Result<C::Ok> {
        //let mut data = [0u8; C::MAX_LEN]; // TODO: once associated consts work...
        let mut data = [0u8; 36];
        command.encode(&mut data)?;

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

        res
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

    fn execute_raw<'a>(&mut self, data: &[u8], out: &'a mut [u8], response_delay: Duration) -> io::Result<&'a mut [u8]> {
        assert!(data.len() <= 36);

        let mut packet = [0u8; 36 + 3];
        let packet = Self::encode_command(data, &mut packet);

        let full_len = {
            self.sleep();
            self.inner.i2c_transfer(&mut [i2c::Message::Write {
                address: I2C_ADDRESS_DDC_CI,
                data: packet,
                flags: Default::default(),
            }])?;
            //self.inner.write(packet)?;
            if !out.is_empty() {
                sleep(response_delay);
                //self.inner.read(out)?;
                let mut msgs = [i2c::Message::Read {
                    address: I2C_ADDRESS_DDC_CI,
                    data: out,
                    flags: Default::default(),
                }];
                self.inner.i2c_transfer(&mut msgs)?;

                msgs[0].len()
            } else {
                return Ok(out)
            }
        };

        if full_len < 2 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "DDC/CI invalid length"))
        }

        let len = (out[1] & 0x7f) as usize;

        if out[1] & 0x80 == 0 {
            // TODO: apparently sometimes this isn't true?
            return Err(io::Error::new(io::ErrorKind::InvalidData, "DDC/CI length bit not set"))
        }

        if full_len < len + 2 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "DDC/CI length mismatch"))
        }

        let checksum = Self::checksum(
            iter::once(((I2C_ADDRESS_DDC_CI as u8) << 1) | 1)
            .chain(iter::once(SUB_ADDRESS_DDC_CI))
            .chain(out[1..2 + len].iter().cloned())
        );

        if out[2 + len] != checksum {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "DDC/CI checksum mismatch"))
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
