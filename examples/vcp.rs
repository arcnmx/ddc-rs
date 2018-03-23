extern crate ddc;

use std::env::args;
use ddc::{Ddc, commands};

fn ddc<D: ddc::Ddc>(mut ddc: D) where
    D::Error: ::std::fmt::Debug,
{
    let mccs_ver = ddc.get_vcp_feature(0xdf).expect("failed to read VCP value");
    println!("MCCS version is {:04x}", mccs_ver.maximum());

    let input = ddc.get_vcp_feature(0x60).expect("failed to read VCP value");
    println!("input is {:?}", input);
}

#[cfg(feature = "i2c-linux")]
fn main() {
    //::env_logger::init();

    use std::env::args;

    let path = args().nth(1).expect("argument: i2c device path");

    ddc(ddc::from_i2c_device(path).expect("failed to open i2c device"))
}

#[cfg(not(feature = "i2c-linux"))]
fn main() {
    unimplemented!()
}
