use ansi_term::Color::{self, Cyan, Fixed, Green, Purple};
use anyhow::Result;
use can_dbc::{ByteOrder, Signal};
use futures::StreamExt;
use socketcan::{tokio::CanSocket, CanFrame, EmbeddedFrame};
use std::collections::HashMap;
use std::convert::TryInto;
use std::fmt::Write;
use std::path::PathBuf;
use structopt::StructOpt;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

// Standard CAN constants (missing from socketcan crate)
const EFF_FLAG: u32 = 0x80000000; // Extended Frame Format flag

const COLOR_CAN_ID: Color = Color::White;
const COLOR_CAN_SFF: Color = Color::Blue;
const COLOR_CAN_EFF: Color = Color::Red;

const COLOR_NULL: Color = Fixed(242); // grey
const COLOR_OFFSET: Color = Fixed(242); // grey
const COLOR_ASCII_PRINTABLE: Color = Color::Cyan;
const COLOR_ASCII_WHITESPACE: Color = Color::Green;
const COLOR_ASCII_OTHER: Color = Color::Purple;
const COLOR_NONASCII: Color = Color::Yellow;

enum ByteCategory {
    Null,
    AsciiPrintable,
    AsciiWhitespace,
    AsciiOther,
    NonAscii,
}

#[derive(Copy, Clone)]
struct Byte(u8);

impl Byte {
    fn category(self) -> ByteCategory {
        if self.0 == 0x00 {
            ByteCategory::Null
        } else if self.0.is_ascii_alphanumeric()
            || self.0.is_ascii_punctuation()
            || self.0.is_ascii_graphic()
        {
            ByteCategory::AsciiPrintable
        } else if self.0.is_ascii_whitespace() {
            ByteCategory::AsciiWhitespace
        } else if self.0.is_ascii() {
            ByteCategory::AsciiOther
        } else {
            ByteCategory::NonAscii
        }
    }

    fn color(self) -> &'static Color {
        use ByteCategory::*;

        match self.category() {
            Null => &COLOR_NULL,
            AsciiPrintable => &COLOR_ASCII_PRINTABLE,
            AsciiWhitespace => &COLOR_ASCII_WHITESPACE,
            AsciiOther => &COLOR_ASCII_OTHER,
            NonAscii => &COLOR_NONASCII,
        }
    }

    fn as_char(self) -> char {
        use ByteCategory::*;

        match self.category() {
            Null => '0',
            AsciiPrintable => self.0 as char,
            AsciiWhitespace if self.0 == 0x20 => ' ',
            AsciiWhitespace => '_',
            AsciiOther => '•',
            NonAscii => '×',
        }
    }
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "candumprb",
    about = "Candump Rainbow. A colorful can dump tool with dbc support."
)]
struct Opt {
    /// DBC file path, if not passed frame signals are not decoded
    #[structopt(short = "i", long = "input", parse(from_os_str))]
    input: Option<PathBuf>,

    /// Set can interface
    #[structopt(help = "socketcan CAN interface e.g. vcan0")]
    can_interface: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::from_args();

    let mut socket_rx = CanSocket::open(&opt.can_interface).unwrap();

    let byte_hex_table: Vec<String> = (0u8..=u8::MAX)
        .map(|i| {
            let byte_hex = format!("{:02x} ", i);
            Byte(i).color().paint(byte_hex).to_string()
        })
        .collect();

    // Read DBC and turn it into a hashmap for lookup
    let signal_lookup = if let Some(dbc_input) = opt.input.as_ref() {
        let mut f = File::open(dbc_input).await?;
        let mut buffer = Vec::new();
        f.read_to_end(&mut buffer).await?;
        let dbc = can_dbc::DBC::from_slice(&buffer).expect("Failed to parse DBC");

        let mut signal_lookup = HashMap::new();

        for msg in dbc.messages() {
            let message_id: u32 = match msg.message_id() {
                can_dbc::MessageId::Standard(id) => (*id).into(),
                can_dbc::MessageId::Extended(id) => *id,
            };
            signal_lookup.insert(
                message_id & !EFF_FLAG,
                (msg.message_name().clone(), msg.signals().clone()),
            );
        }
        Some(signal_lookup)
    } else {
        None
    };

    while let Some(socket_result) = socket_rx.next().await {
        match socket_result {
            Ok(frame) => {
                if let Some(signal_lookup) = signal_lookup.as_ref() {
                    print_dbc_signals(signal_lookup, &frame);
                }

                let mut buffer: String = String::new();

                if frame.is_extended() {
                    write!(buffer, "{}", COLOR_CAN_EFF.paint("EFF ")).unwrap();
                } else {
                    write!(buffer, "{}", COLOR_CAN_SFF.paint("SFF ")).unwrap();
                }

                let raw_id = match frame.id() {
                    socketcan::Id::Standard(id) => id.as_raw() as u32,
                    socketcan::Id::Extended(id) => id.as_raw(),
                };
                
                write!(
                    buffer,
                    "{}",
                    COLOR_CAN_ID.paint(format!("{:08x} ", raw_id))
                )?;

                for b in frame.data() {
                    write!(buffer, "{}", byte_hex_table[*b as usize]).unwrap();
                }

                println!("{}", buffer);
            }
            Err(err) => {
                eprintln!("IO error: {}", err);
            }
        }
    }

    Ok(())
}

// Given a CAN Frame, lookup the can signals and print the signal values
fn print_dbc_signals(signal_lookup: &HashMap<u32, (String, Vec<Signal>)>, frame: &CanFrame) {
    let raw_id = match frame.id() {
        socketcan::Id::Standard(id) => id.as_raw() as u32,
        socketcan::Id::Extended(id) => id.as_raw(),
    };
    let id = raw_id & !EFF_FLAG;
    let (message_name, signals) = signal_lookup.get(&id).expect("Unknown message id");
    println!("\n{}", Purple.paint(message_name));

    for signal in signals.iter() {
        let frame_data: [u8; 8] = frame
            .data()
            .try_into()
            .expect("slice with incorrect length");

        let signal_value: u64 = if *signal.byte_order() == ByteOrder::LittleEndian {
            u64::from_le_bytes(frame_data)
        } else {
            u64::from_be_bytes(frame_data)
        };

        // Calculate signal value
        let bit_mask: u64 = 2u64.pow(*signal.signal_size() as u32) - 1;
        let signal_value = ((signal_value >> signal.start_bit()) & bit_mask) as f32
            * *signal.factor() as f32
            + *signal.offset() as f32;

        let signal_value_s = format!("{:6.4}", signal_value);

        println!(
            "{} → value {}",
            Green.paint(signal.name()),
            Cyan.paint(signal_value_s)
        );
    }
}
