extern crate ddc;

use std::str;
use ddc::Ddc;

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

fn ddc<D: ddc::Ddc>(mut ddc: D) where
    D::Error: ::std::fmt::Debug,
{
    let caps = ddc.capabilities_string().expect("failed to read ddc capabilities");
    let caps = str::from_utf8(&caps).expect("caps was not a valid string");
    println!("got CAPS: {}", caps);
}
