extern crate ddc;
extern crate edid;

use std::env::args;
use ddc::{Ddc, commands};

fn main() {
    //::env_logger::init();

    let path = args().nth(1).expect("argument: i2c device path");

    let mut ddc = Ddc::from_path(path).expect("failed to open i2c device");

    let input = ddc.execute(commands::GetVcpFeature::new(0x60)).expect("failed to read VCP value");
    println!("input is {:?}", input);
}
