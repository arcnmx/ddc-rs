extern crate ddc;

use std::env::args;
use std::str;
use ddc::Ddc;

fn main() {
    //::env_logger::init();

    let path = args().nth(1).expect("argument: i2c device path");

    let mut ddc = Ddc::from_path(path).expect("failed to open i2c device");

    let caps = ddc.capabilities_string().expect("failed to read ddc capabilities");
    let caps = str::from_utf8(&caps).expect("caps was not a valid string");
    println!("got CAPS: {}", caps);
}
