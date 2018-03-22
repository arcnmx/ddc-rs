extern crate i2c;
extern crate ddc;
extern crate edid;

use std::env::args;
use ddc::{Ddc, commands};

fn ddc<D: i2c::Address + i2c::ReadWrite>(mut ddc: Ddc<D>) where
    D::Error: ::std::fmt::Debug,
{
    let mccs_ver = ddc.execute(commands::GetVcpFeature::new(0xdf)).expect("failed to read VCP value");
    println!("MCCS version is {:04x}", mccs_ver.maximum());

    let input = ddc.execute(commands::GetVcpFeature::new(0x60)).expect("failed to read VCP value");
    println!("input is {:?}", input);
}

#[cfg(feature = "i2c-linux")]
fn main() {
    //::env_logger::init();

    use std::env::args;

    let path = args().nth(1).expect("argument: i2c device path");

    ddc(Ddc::from_path(path).expect("failed to open i2c device"))
}

#[cfg(not(feature = "i2c-linux"))]
fn main() {
    unimplemented!()
}
