extern crate udev;

use std::path::PathBuf;
use std::os::unix::ffi::OsStrExt;
use std::io;
use Ddc;

/// Enumerate all currently attached displays on the system. Implements an
/// `Iterator` that generates I2C device paths to be passed to `Ddc::from_path`.
///
/// The current detection approach only checks that a monitor is on the I2C bus
/// with a reachable EDID/EEPROM. DDC/CI communication may not be available if
/// the display does not support it, or if the active input is controlled by
/// another host device.
///
/// # Example
///
/// ```rust,no_run
/// use ddc::{Enumerator, Ddc, commands};
///
/// let displays = Enumerator::new().unwrap();
/// for display_path in displays {
///     let mut ddc = Ddc::from_path(display_path).unwrap();
///     let mccs_version = ddc.execute(commands::GetVcpFeature::new(0xdf)).unwrap();
///     println!("MCCS version: {:04x}", mccs_version.maximum());
/// }
/// ```
///
/// # udev dependency
///
/// Requires the `udev` feature enabled to use.
pub struct Enumerator {
    inner: udev::Devices,
}

impl Enumerator {
    /// Create a new enumerator for available displays.
    pub fn new() -> io::Result<Self> {
        let udev = udev::Context::new()?;
        let mut en = udev::Enumerator::new(&udev)?;
        en.match_subsystem("i2c-dev")?;

        Ok(Enumerator {
            inner: en.scan_devices()?,
        })
    }
}

impl Iterator for Enumerator {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(dev) = self.inner.next() {
            let (devnode, name) = match dev.devnode().and_then(|devnode| dev.attribute_value("name").map(|name| (devnode, name))) {
                Some(v) => v,
                None => continue,
            };

            let skip_prefix = [ // list stolen from ddcutil's ignorable_i2c_device_sysfs_name
                "SMBus",
                "soc:i2cdsi",
                "smu",
                "mac-io",
                "u4",
            ];

            if skip_prefix.iter().any(|p| name.as_bytes().starts_with(p.as_bytes())) {
                continue
            }

            if Ddc::from_path(devnode).and_then(|mut ddc| ddc.read_edid(0, &mut [0u8])).is_err() {
                continue
            }

            return Some(devnode.into())
        }

        None
    }
}
