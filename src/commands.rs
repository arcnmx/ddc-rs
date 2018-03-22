#![allow(missing_docs)]
use std::{mem, fmt};
use ErrorCode;

pub trait Command {
    type Ok: CommandResult;
    const MIN_LEN: usize;
    const MAX_LEN: usize;
    const DELAY_RESPONSE_MS: u64;
    const DELAY_COMMAND_MS: u64;

    fn len(&self) -> usize;

    fn encode(&self, data: &mut [u8]) -> Result<usize, ErrorCode>;
}

pub trait CommandResult: Sized {
    const MAX_LEN: usize;
    fn decode(data: &[u8]) -> Result<Self, ErrorCode>;
}

#[derive(Copy, Clone, Debug)]
pub struct GetVcpFeature {
    pub code: FeatureCode,
}

impl GetVcpFeature {
    pub fn new(code: FeatureCode) -> Self {
        GetVcpFeature {
            code: code,
        }
    }
}

impl Command for GetVcpFeature {
    type Ok = VcpValue;
    const MIN_LEN: usize = 2;
    const MAX_LEN: usize = 2;
    const DELAY_RESPONSE_MS: u64 = 40;
    const DELAY_COMMAND_MS: u64 = 50; // the spec omits this, but 50 corresponds with what all other commands suggest

    fn len(&self) -> usize { 2 }

    fn encode(&self, data: &mut [u8]) -> Result<usize, ErrorCode> {
        assert!(data.len() >= 2);
        data[0] = 0x01;
        data[1] = self.code;

        Ok(2)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct SetVcpFeature {
    pub code: FeatureCode,
    pub value: u16,
}

impl SetVcpFeature {
    pub fn new(code: FeatureCode, value: u16) -> Self {
        SetVcpFeature {
            code: code,
            value: value,
        }
    }
}

impl Command for SetVcpFeature {
    type Ok = ();
    const MIN_LEN: usize = 4;
    const MAX_LEN: usize = 4;
    const DELAY_RESPONSE_MS: u64 = 0;
    const DELAY_COMMAND_MS: u64 = 50;

    fn len(&self) -> usize { 4 }

    fn encode(&self, data: &mut [u8]) -> Result<usize, ErrorCode> {
        assert!(data.len() >= 4);

        data[0] = 0x03;
        data[1] = self.code;
        data[2] = (self.value >> 8) as _;
        data[3] = self.value as _;

        Ok(4)
    }
}

pub type FeatureCode = u8;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VcpValue {
    pub kind: u8,
    pub mh: u8,
    pub ml: u8,
    pub sh: u8,
    pub sl: u8,
}

impl VcpValue {
    pub fn value(&self) -> u16 {
        ((self.sh as u16) << 8) | self.sl as u16
    }

    pub fn maximum(&self) -> u16 {
        ((self.mh as u16) << 8) | self.ml as u16
    }
}

impl fmt::Debug for VcpValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("VcpValue")
            .field("maximum", &self.maximum())
            .field("value", &self.value())
            .finish()
    }
}

impl CommandResult for VcpValue {
    const MAX_LEN: usize = 8;

    fn decode(data: &[u8]) -> Result<Self, ErrorCode> {
        if data.len() != 8 {
            return Err(ErrorCode::InvalidLength)
        }

        if data[0] != 0x02 {
            return Err(ErrorCode::InvalidOpcode)
        }

        match data[1] {
            0x00 => (), // NoError
            0x01 => return Err(ErrorCode::Invalid("Unsupported VCP code".into())),
            rc => return Err(ErrorCode::Invalid(format!("Unrecognized VCP error code 0x{:02x}", rc))),
        }

        // data[2] == vcp code from request

        Ok(VcpValue {
            kind: data[2],
            mh: data[6],
            ml: data[7],
            sh: data[4],
            sl: data[5],
        })
    }
}

#[derive(Copy, Clone, Debug)]
pub struct SaveCurrentSettings;

impl Command for SaveCurrentSettings {
    type Ok = ();
    const MIN_LEN: usize = 1;
    const MAX_LEN: usize = 1;
    const DELAY_RESPONSE_MS: u64 = 0;
    const DELAY_COMMAND_MS: u64 = 200;

    fn len(&self) -> usize { 1 }

    fn encode(&self, data: &mut [u8]) -> Result<usize, ErrorCode> {
        assert!(data.len() >= 1);
        data[0] = 0x0c;

        Ok(1)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct TableWrite<'a> {
    pub code: FeatureCode,
    pub offset: u16,
    pub data: &'a [u8],
}

impl<'a> TableWrite<'a> {
    pub fn new(code: FeatureCode, offset: u16, data: &'a [u8]) -> Self {
        TableWrite {
            code: code,
            offset: offset,
            data: data,
        }
    }
}

impl<'a> Command for TableWrite<'a> {
    type Ok = ();
    const MIN_LEN: usize = 4;
    const MAX_LEN: usize = 4 + 32; // Spec says this should be 3~35 but allows 32 bytes of data transfer?? how?? What does "P=1" mean?
    const DELAY_RESPONSE_MS: u64 = 0;
    const DELAY_COMMAND_MS: u64 = 50;

    fn len(&self) -> usize { 4 + self.data.len() }

    fn encode(&self, data: &mut [u8]) -> Result<usize, ErrorCode> {
        assert!(data.len() >= 4 + self.data.len());
        assert!(self.data.len() <= 32);

        data[0] = 0xe7;
        data[1] = self.code;
        data[2] = (self.offset >> 8) as _;
        data[3] = self.offset as _;
        data[4..4 + self.data.len()].copy_from_slice(self.data);

        Ok(4 + self.data.len())
    }
}

#[derive(Copy, Clone, Debug)]
pub struct TableRead {
    pub code: FeatureCode,
    pub offset: u16,
}

impl TableRead {
    pub fn new(code: FeatureCode, offset: u16) -> Self {
        TableRead {
            code: code,
            offset: offset,
        }
    }
}

impl Command for TableRead {
    type Ok = TableResponse;
    const MIN_LEN: usize = 4;
    const MAX_LEN: usize = 4;
    const DELAY_RESPONSE_MS: u64 = 40;
    const DELAY_COMMAND_MS: u64 = 50;

    fn len(&self) -> usize { 4 }

    fn encode(&self, data: &mut [u8]) -> Result<usize, ErrorCode> {
        assert!(data.len() >= 4);

        data[0] = 0xe2;
        data[1] = self.code;
        data[2] = (self.offset >> 8) as _;
        data[3] = self.offset as _;

        Ok(4)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct CapabilitiesRequest {
    pub offset: u16,
}

impl CapabilitiesRequest {
    pub fn new(offset: u16) -> Self {
        CapabilitiesRequest {
            offset: offset,
        }
    }
}

impl Command for CapabilitiesRequest {
    type Ok = CapabilitiesReply;
    const MIN_LEN: usize = 3;
    const MAX_LEN: usize = 3;
    const DELAY_RESPONSE_MS: u64 = 40;
    const DELAY_COMMAND_MS: u64 = 50;

    fn len(&self) -> usize { 3 }

    fn encode(&self, data: &mut [u8]) -> Result<usize, ErrorCode> {
        assert!(data.len() >= 3);

        data[0] = 0xf3;
        data[1] = (self.offset >> 8) as _;
        data[2] = self.offset as _;

        Ok(3)
    }
}

#[derive(Copy, Clone)]
pub struct TableResponse {
    pub offset: u16,
    data: [u8; 32],
    len: u8,
}

impl TableResponse {
    pub fn bytes(&self) -> &[u8] {
        &self.data[..self.len as usize]
    }
}

impl fmt::Debug for TableResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TableResponse")
            .field("offset", &self.offset)
            .field("bytes", &self.bytes())
            .finish()
    }
}

impl Default for TableResponse {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

impl CommandResult for TableResponse {
    const MAX_LEN: usize = 36;

    fn decode(data: &[u8]) -> Result<Self, ErrorCode> {
        if data.len() < 4 || data.len() > 36  { // spec says 3 - 35???
            return Err(ErrorCode::InvalidLength)
        }

        if data[0] != 0xe4 {
            return Err(ErrorCode::InvalidOpcode)
        }

        let mut table = TableResponse::default();
        table.offset = ((data[1] as u16) << 8) | data[2] as u16;
        let data = &data[3..];
        table.len = data.len() as u8;
        table.data[..data.len()].copy_from_slice(data);
        Ok(table)
    }
}

#[derive(Clone, Debug)]
pub struct CapabilitiesReply {
    pub offset: u16,
    pub data: Box<[u8]>,
}

impl CommandResult for CapabilitiesReply {
    const MAX_LEN: usize = 35;

    fn decode(data: &[u8]) -> Result<Self, ErrorCode> {
        if data.len() < 3 || data.len() > 35  {
            return Err(ErrorCode::InvalidLength)
        }

        if data[0] != 0xe3 {
            return Err(ErrorCode::InvalidOpcode)
        }

        Ok(CapabilitiesReply {
            offset: ((data[1] as u16) << 8) | data[2] as u16,
            data: data[3..].to_owned().into_boxed_slice(),
        })
    }
}

#[derive(Copy, Clone, Debug)]
pub struct GetTimingReport;

impl Command for GetTimingReport {
    type Ok = TimingMessage;
    const MIN_LEN: usize = 1;
    const MAX_LEN: usize = 1;
    const DELAY_RESPONSE_MS: u64 = 40;
    const DELAY_COMMAND_MS: u64 = 50;

    fn len(&self) -> usize { 1 }

    fn encode(&self, data: &mut [u8]) -> Result<usize, ErrorCode> {
        assert!(data.len() >= 1);
        data[0] = 0x07;

        Ok(1)
    }
}

#[derive(Clone, Debug)]
pub struct TimingMessage {
    pub timing_status: u8,
    pub horizontal_frequency: u16,
    pub vertical_frequency: u16,
}

impl CommandResult for TimingMessage {
    const MAX_LEN: usize = 6;

    fn decode(data: &[u8]) -> Result<Self, ErrorCode> {
        if data.len() != 6 {
            return Err(ErrorCode::InvalidLength)
        }

        if data[0] != 0x4e {
            return Err(ErrorCode::InvalidOpcode)
        }

        Ok(TimingMessage {
            timing_status: data[1],
            horizontal_frequency: ((data[2] as u16) << 8) | data[3] as u16,
            vertical_frequency: ((data[4] as u16) << 8) | data[5] as u16,
        })
    }
}

impl CommandResult for () {
    const MAX_LEN: usize = 0;

    fn decode(_data: &[u8]) -> Result<Self, ErrorCode> {
        unreachable!()
    }
}

impl<'a, C: Command> Command for &'a C {
    type Ok = C::Ok;
    const MIN_LEN: usize = C::MIN_LEN;
    const MAX_LEN: usize = C::MAX_LEN;
    const DELAY_RESPONSE_MS: u64 = C::DELAY_RESPONSE_MS;
    const DELAY_COMMAND_MS: u64 = C::DELAY_COMMAND_MS;

    fn len(&self) -> usize { (*self).len() }

    fn encode(&self, data: &mut [u8]) -> Result<usize, ErrorCode> {
        (*self).encode(data)
    }
}
