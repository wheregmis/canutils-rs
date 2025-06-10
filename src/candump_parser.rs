use nom::{
    bytes::complete::tag,
    character::complete::{alphanumeric1, digit1, hex_digit1, space0},
    combinator::map_res,
    sequence::tuple,
    IResult,
};

#[cfg(test)]
mod tests {
    use crate::candump_parser::*;

    #[test]
    fn it_works() {
        let exp = DumpEntry {
            timestamp: Timestamp {
                seconds: 1547046014,
                nanos: 597158,
            },
            can_interface: "vcan0".to_string(),
            can_frame: CanFrame {
                frame_id: 123,
                frame_body: 455,
            },
        };
        assert_eq!(
            dump_entry("(1547046014.597158) vcan0 7B#1C7"),
            Ok(("", exp))
        );
    }
}

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Timestamp {
    pub seconds: u64,
    pub nanos: u64,
}

fn timestamp(input: &str) -> IResult<&str, Timestamp> {
    let (input, (_, seconds, _, nanos, _)) = tuple((
        tag("("),
        map_res(digit1, |d: &str| d.parse()),
        tag("."),
        map_res(digit1, |d: &str| d.parse()),
        tag(")"),
    ))(input)?;
    
    Ok((input, Timestamp { seconds, nanos }))
}

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CanFrame {
    pub frame_id: u32,
    pub frame_body: u64,
}

fn can_frame(input: &str) -> IResult<&str, CanFrame> {
    let (input, (frame_id, _, frame_body)) = tuple((
        map_res(hex_digit1, |d| u32::from_str_radix(d, 16)),
        tag("#"),
        map_res(hex_digit1, |d| u64::from_str_radix(d, 16)),
    ))(input)?;
    
    Ok((input, CanFrame { frame_id, frame_body }))
}

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DumpEntry {
    timestamp: Timestamp,
    can_interface: String,
    can_frame: CanFrame,
}

impl DumpEntry {
    pub fn timestamp(&self) -> &Timestamp {
        &self.timestamp
    }

    pub fn can_interface(&self) -> &str {
        &self.can_interface
    }

    pub fn can_frame(&self) -> &CanFrame {
        &self.can_frame
    }
}

pub fn dump_entry(input: &str) -> IResult<&str, DumpEntry> {
    let (input, (timestamp, _, can_interface, _, can_frame)) = tuple((
        timestamp,
        space0,
        alphanumeric1,
        space0,
        can_frame,
    ))(input)?;
    
    Ok((input, DumpEntry { 
        timestamp, 
        can_interface: can_interface.to_string(), 
        can_frame 
    }))
}
