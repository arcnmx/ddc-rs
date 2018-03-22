extern crate i2c;
extern crate ddc;
extern crate edid;

use std::path::Path;
use std::env::args;
use std::io;
use ddc::Ddc;

#[cfg(feature = "i2c-linux")]
fn edid<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref();

    println!("Opening {}", path.display());

    ddc(Ddc::from_path(path)?)
}

#[cfg(not(feature = "i2c-linux"))]
fn edid<P: AsRef<Path>>(_path: P) -> io::Result<()> {
    unimplemented!()
}

fn ddc<D: i2c::Address + i2c::BlockTransfer>(mut ddc: Ddc<D>) -> Result<(), D::Error> where
    D::Error: ::std::fmt::Debug,
    D::Error: From<io::Error>,
{
    let mut edid = [0u8; 0x80];
    let len = ddc.read_edid(0, &mut edid)?;

    let edid = edid::parse(&edid[..len]).to_result()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    println!("EDID: {:#?}", edid);

    Ok(())
}

fn main() {
    //::env_logger::init();

    let path = args().nth(1);

    match path {
        Some(path) => edid(path).expect("failed to get EDID"),
        #[cfg(all(feature = "udev", feature = "i2c-linux"))]
        None => ddc::Enumerator::new().expect("failed to enumerate DDC devices").for_each(|p| match edid(p) {
            Ok(()) => (),
            Err(e) => println!("Failure: {:?}", e),
        }),
        #[cfg(not(all(feature = "udev", feature = "i2c-linux")))]
        None => panic!("argument: i2c device path"),
    }
}
