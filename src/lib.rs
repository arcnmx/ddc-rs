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
extern crate mccs;
#[cfg(feature = "i2c-linux")]
extern crate i2c_linux;
#[cfg(feature = "i2c")]
extern crate i2c;

use std::{iter, fmt, error};
use std::time::Duration;

pub use mccs::{FeatureCode, Value as VcpValue};

/// DDC/CI command request and response types.
pub mod commands;
pub use commands::{Command, CommandResult, TimingMessage};

#[cfg(all(feature = "udev", feature = "i2c-linux"))]
mod enumerate;

#[cfg(all(feature = "udev", feature = "i2c-linux"))]
pub use enumerate::Enumerator;

mod delay;
pub use delay::Delay;

#[cfg(feature = "i2c")]
mod i2c_ddc;
#[cfg(feature = "i2c")]
pub use i2c_ddc::I2cDdc;
#[cfg(feature = "i2c-linux")]
pub use i2c_ddc::{I2cDeviceDdc, from_i2c_device};

/// EDID EEPROM I2C address
pub const I2C_ADDRESS_EDID: u16 = 0x50;

/// E-DDC EDID segment register I2C address
pub const I2C_ADDRESS_EDID_SEGMENT: u16 = 0x30;

/// DDC/CI command and control I2C address
pub const I2C_ADDRESS_DDC_CI: u16 = 0x37;

/// DDC sub-address command prefix
pub const SUB_ADDRESS_DDC_CI: u8 = 0x51;

/// DDC delay required before retrying a request
pub const DELAY_COMMAND_FAILED_MS: u64 = 40;

/// A trait that allows retrieving Extended Display Identification Data (EDID)
/// from a device.
pub trait Edid {
    /// An error that can occur when reading the EDID from a device.
    type EdidError;

    /// Read up to 256 bytes of the monitor's EDID.
    fn read_edid(&mut self, offset: u8, data: &mut [u8]) -> Result<usize, Self::EdidError>;
}

/// E-DDC allows reading extensions of Enhanced EDID.
pub trait Eddc: Edid {
    /// Read part of the EDID using the segments added in the Enhanced Display
    /// Data Channel (E-DDC) protocol.
    fn read_eddc_edid(&mut self, segment: u8, offset: u8, data: &mut [u8]) -> Result<usize, Self::EdidError>;
}

/// A DDC host is able to communicate with a DDC device such as a display.
pub trait DdcHost {
    /// An error that can occur when communicating with a DDC device.
    ///
    /// Usually impls `From<ErrorCode>`.
    type Error;

    /// Wait for any previous commands to complete.
    ///
    /// The DDC specification defines delay intervals that must occur between
    /// execution of two subsequent commands, this waits for the amount of time
    /// remaining since the last command was executed. This is normally done
    /// internally and shouldn't need to be called manually unless synchronizing
    /// with an external process or another handle to the same device. It may
    /// however be desireable to run this before program exit.
    fn sleep(&mut self) { }
}

/// Allows the execution of arbitrary low level DDC commands.
pub trait DdcCommandRaw: DdcHost {
    /// Executes a raw DDC/CI command.
    ///
    /// A response should not be read unless `out` is not empty, and the delay
    /// should occur in between any write and read made to the device. A subslice
    /// of `out` excluding DDC packet headers should be returned.
    fn execute_raw<'a>(&mut self, data: &[u8], out: &'a mut [u8], response_delay: Duration) -> Result<&'a mut [u8], Self::Error>;
}

/// Using this marker trait will automatically implement the `DdcCommand` trait.
pub trait DdcCommandRawMarker: DdcCommandRaw where Self::Error: From<ErrorCode> {
    /// Sets an internal `Delay` that must expire before the next command is
    /// attempted.
    fn set_sleep_delay(&mut self, delay: Delay);
}

/// A (slightly) higher level interface to `DdcCommandRaw`.
///
/// Some DDC implementations only provide access to the higher level commands
/// exposed in the `Ddc` trait.
pub trait DdcCommand: DdcHost {
    /// Execute a DDC/CI command. See the `commands` module for all available
    /// commands. The return type is dependent on the executed command.
    fn execute<C: Command>(&mut self, command: C) -> Result<C::Ok, Self::Error>;

    /// Computes a DDC/CI packet checksum
    fn checksum<II: IntoIterator<Item=u8>>(iter: II) -> u8 {
        iter.into_iter().fold(0u8, |sum, v| sum ^ v)
    }

    /// Encodes a DDC/CI command into a packet.
    ///
    /// `packet.len()` must be 3 bytes larger than `data.len()`
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
}

/// Using this marker trait will automatically implement the `Ddc` and `DdcTable`
/// traits.
pub trait DdcCommandMarker: DdcCommand where Self::Error: From<ErrorCode> { }

/// A high level interface to DDC commands.
pub trait Ddc: DdcHost {
    /// Retrieve the capability string from the device.
    ///
    /// This executes multiple `CapabilitiesRequest` commands to construct the entire string.
    fn capabilities_string(&mut self) -> Result<Vec<u8>, Self::Error>;

    /// Gets the current value of an MCCS VCP feature.
    fn get_vcp_feature(&mut self, code: FeatureCode) -> Result<VcpValue, Self::Error>;

    /// Sets a VCP feature to the specified value.
    fn set_vcp_feature(&mut self, code: FeatureCode, value: u16) -> Result<(), Self::Error>;

    /// Instructs the device to save its current settings.
    fn save_current_settings(&mut self) -> Result<(), Self::Error>;

    /// Retrieves a timing report from the device.
    fn get_timing_report(&mut self) -> Result<TimingMessage, Self::Error>;
}

/// Table commands can read and write arbitrary binary data to a VCP feature.
///
/// Tables were introduced in MCCS specification versions 3.0 and 2.2.
pub trait DdcTable: DdcHost {
    /// Read a table value from the device.
    fn table_read(&mut self, code: FeatureCode) -> Result<Vec<u8>, Self::Error>;

    /// Write a table value to the device.
    fn table_write(&mut self, code: FeatureCode, offset: u16, value: &[u8]) -> Result<(), Self::Error>;
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

impl<D: DdcCommandMarker> Ddc for D where D::Error: From<ErrorCode> {
    fn capabilities_string(&mut self) -> Result<Vec<u8>, Self::Error> {
        let mut string = Vec::new();
        let mut offset = 0;
        loop {
            let caps = self.execute(commands::CapabilitiesRequest::new(offset))?;
            if caps.offset != offset {
                return Err(ErrorCode::InvalidOffset.into())
            } else if caps.data.is_empty() {
                break
            }

            string.extend(caps.data.iter());

            offset += caps.data.len() as u16;
        }

        Ok(string)
    }

    fn get_vcp_feature(&mut self, code: FeatureCode) -> Result<VcpValue, Self::Error> {
        self.execute(commands::GetVcpFeature::new(code))
    }

    fn set_vcp_feature(&mut self, code: FeatureCode, value: u16) -> Result<(), Self::Error> {
        self.execute(commands::SetVcpFeature::new(code, value))
    }

    fn save_current_settings(&mut self) -> Result<(), Self::Error> {
        self.execute(commands::SaveCurrentSettings)
    }

    fn get_timing_report(&mut self) -> Result<TimingMessage, Self::Error> {
        self.execute(commands::GetTimingReport)
    }
}

impl<D: DdcCommandMarker> DdcTable for D where D::Error: From<ErrorCode> {
    fn table_read(&mut self, code: FeatureCode) -> Result<Vec<u8>, Self::Error> {
        let mut value = Vec::new();
        let mut offset = 0;
        loop {
            let table = self.execute(commands::TableRead::new(code, offset))?;
            if table.offset != offset {
                return Err(ErrorCode::InvalidOffset.into())
            } else if table.bytes().is_empty() {
                break
            }

            value.extend(table.bytes().iter());

            offset += table.bytes().len() as u16;
        }

        Ok(value)
    }

    fn table_write(&mut self, code: FeatureCode, mut offset: u16, value: &[u8]) -> Result<(), Self::Error> {
        for chunk in value.chunks(32) {
            self.execute(commands::TableWrite::new(code, offset, chunk))?;
            offset += chunk.len() as u16;
        }

        Ok(())
    }
}

impl<D: DdcCommandRawMarker> DdcCommand for D where D::Error: From<ErrorCode> {
    fn execute<C: Command>(&mut self, command: C) -> Result<C::Ok, Self::Error> {
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
                self.set_sleep_delay(Delay::new(Duration::from_millis(C::DELAY_COMMAND_MS)));
                res
            },
            Err(e) => {
                self.set_sleep_delay(Delay::new(Duration::from_millis(DELAY_COMMAND_FAILED_MS)));
                return Err(e)
            },
        };

        let res = C::Ok::decode(res);

        if res.is_err() {
            self.set_sleep_delay(Delay::new(Duration::from_millis(DELAY_COMMAND_FAILED_MS)));
        }

        res.map_err(From::from)
    }
}
