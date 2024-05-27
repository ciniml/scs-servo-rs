use std::io::{Stdout, Write};

use clap::{builder, Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};


#[derive(Debug, Parser)]
#[clap(name = env!("CARGO_PKG_NAME"), version = env!("CARGO_PKG_VERSION"), author = env!("CARGO_PKG_AUTHORS"), about = env!("CARGO_PKG_DESCRIPTION"), arg_required_else_help = true)]
struct Cli {
    #[clap(subcommand)]
    subcommand: SubCommands,

    #[clap(short, long, help = "The serial port to use")]
    port: String,
    #[clap(short, long, help = "The baud rate to use", default_value = "1000000")]
    baud: u32,
    #[clap(short, long, help = "The serial adapter echoes back sent data", default_value = "false")]
    echo: bool,
    #[clap(short, long, help = "The timeout in milliseconds every ID", default_value = "10")]
    timeout_ms: u32,
}

#[derive(Debug, Clone)]
enum Format {
    Raw,
    Hex,
}
impl std::str::FromStr for Format {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "raw" => Ok(Format::Raw),
            "hex" => Ok(Format::Hex),
            _ => Err("Invalid format".to_string()),
        }
    }
}

fn id_in_range(s: &str) -> Result<u8, String> {
    clap_num::maybe_hex_range(s, 1, 254)
}

#[derive(Debug, Subcommand)]
enum SubCommands {
    Scan,
    Read {
        #[clap(short, long, help = "The servo ID to read from", value_parser = id_in_range)]
        id: u8,
        #[clap(short, long, help = "The register address to read from", value_parser = clap_num::maybe_hex::<u8>)]
        address: u8,
        #[clap(short, long, help = "The number of bytes to read", value_parser = clap_num::maybe_hex::<u8>)]
        length: u8,
        #[clap(short, long, help = "The format to output the data in", default_value = "hex")]
        format: Format,
        #[clap(short, long, help = "The file to write the output to")]
        output: Option<String>,
    },
    Write {
        #[clap(short, long, help = "The servo ID to write to", value_parser = id_in_range)]
        id: u8,
        #[clap(short, long, help = "The register address to write to", value_parser = clap_num::maybe_hex::<u8>)]
        address: u8,
        // #[clap(short, long, help = "The number of bytes to write", value_parser = clap_num::maybe_hex::<u8>)]
        // length: Option<u8>,
        #[clap(short, long, help = "The format to input the data", default_value = "hex")]
        format: Format,
        #[clap(short = 'r', long, help = "The file to read the input from")]
        input: Option<String>,
    },
}

struct SerialReader<'a> {
    serial: &'a std::cell::RefCell<Box<dyn serialport::SerialPort>>
}
struct SerialWriter<'a> {
    serial: &'a std::cell::RefCell<Box<dyn serialport::SerialPort>>,
}
impl<'a> scs_servo::protocol::StreamReader for SerialReader<'a> {
    type Error = serialport::Error;
    fn read(&mut self, data: &mut [u8]) -> nb::Result<usize, Self::Error> {
        self.serial.borrow_mut().read(data).map_err(|err| nb::Error::Other(serialport::Error::from(err)))
    }
}
impl<'a> scs_servo::protocol::StreamWriter for SerialWriter<'a> {
    type Error = serialport::Error;
    fn write(&mut self, data: &[u8]) -> nb::Result<usize, Self::Error> {
        self.serial.borrow_mut().write(data).map_err(|err| nb::Error::Other(serialport::Error::from(err)))
    }
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();
    let cli = Cli::parse();

    let serial = serialport::new(&cli.port, cli.baud)
        .open()
        .expect("Failed to open serial port");
    let serial = std::cell::RefCell::new(serial);
    serial.borrow_mut().set_timeout(std::time::Duration::from_millis(cli.timeout_ms as u64)).expect("Failed to set timeout");
    let mut reader = SerialReader { serial: &serial };
    let mut writer = SerialWriter { serial: &serial };
    let config = scs_servo::protocol::ProtocolMasterConfig {
        echo_back: cli.echo,
    };
    let mut master: scs_servo::protocol::ProtocolMaster<8> = scs_servo::protocol::ProtocolMaster::new(config);

    match cli.subcommand {
        SubCommands::Scan => {
            log::info!("Scanning for servos on port {} at baud rate {}", &cli.port, cli.baud);
            let progress_bar = ProgressBar::new(254);
            progress_bar.set_style(ProgressStyle::default_bar().template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}").unwrap());
            progress_bar.set_message("Scanning...");

            for id in 1..254 {
                let start = std::time::Instant::now();
                let mut buffer = [0; 3];
                match master.read_register(&mut reader, &mut writer, id, 0x03, &mut buffer, || start.elapsed().as_millis() > cli.timeout_ms as u128) {
                    Ok(_) => {
                        log::info!("Found servo with ID {} version {:02X} {:02X}", id, buffer[0], buffer[1]);
                    }
                    Err(err) => {
                        log::debug!("Err with ID {} {:?}", id, err);
                    }
                }
                progress_bar.inc(1);
            }
        },
        SubCommands::Read { id, address, length, format, output } => {
            let mut buffer = vec![0; length as usize];
            let start = std::time::Instant::now();
            match master.read_register(&mut reader, &mut writer, id, address, &mut buffer, || start.elapsed().as_millis() > cli.timeout_ms as u128) {
                Ok(_) => {
                    let output_writer = match output {
                        Some(path) => {
                            match std::fs::File::create(path) {
                                Ok(file) => Some(Box::new(std::io::BufWriter::new(file)) as Box<dyn std::io::Write>),
                                Err(err) => {
                                    log::error!("Error opening file: {:?}", err);
                                    None
                                }
                            }
                        }
                        None => Some(Box::new(std::io::stdout()) as Box<dyn std::io::Write>),
                    };
                    if output_writer.is_none() {
                        return;
                    }
                    let mut output_writer = output_writer.unwrap();
                    match format {
                        Format::Raw => {
                            output_writer.write_all(&buffer).expect("Failed to write to output");
                        }
                        Format::Hex => {
                            let hex_string = hex::encode(&buffer);
                            output_writer.write_all(hex_string.as_bytes()).expect("Failed to write to output");
                        }
                    }
                    println!();
                }
                Err(err) => {
                    log::error!("Error reading register: {:?}", err);
                }
            }
        },
        SubCommands::Write { id, address, format, input } => {
            let input_reader = match input {
                Some(path) => {
                    match std::fs::File::open(path) {
                        Ok(file) => Some(Box::new(std::io::BufReader::new(file)) as Box<dyn std::io::Read>),
                        Err(err) => {
                            log::error!("Error opening file: {:?}", err);
                            None
                        }
                    }
                }
                None => Some(Box::new(std::io::stdin()) as Box<dyn std::io::Read>),
            };
            let mut input_reader = match input_reader {
                Some(reader) => reader,
                None => return,
            };
            let mut buffer = Vec::new();
            input_reader.read_to_end(&mut buffer).expect("Failed to read input");
            let data = match format {
                Format::Raw => {
                    buffer
                },
                Format::Hex => {
                    let last_non_space = buffer.iter().rev().enumerate().find(|(_, &b)| b != b'\r' && b != b'\n' && b != b' ');
                    let buffer = match last_non_space {
                        Some((i, _)) => &buffer[..buffer.len() - i],
                        None => &buffer,
                    };
                    hex::decode(&buffer).expect("Failed to decode hex")
                }
            };
            let start = std::time::Instant::now();
            let mut command = scs_servo::protocol::WriteRegisterCommand::<260>::new(id, address, data.len());
            {
                let mut writer = command.writer();
                writer.data_mut().unwrap()[2..2 + data.len()].copy_from_slice(&data);
                writer.update_checksum().expect("Failed to update checksum");
            }   
            match master.write_register(&mut reader, &mut writer, &command, || start.elapsed().as_millis() > cli.timeout_ms as u128) {
                Ok(_) => {
                    log::info!("Wrote {} bytes to register {:02X} on servo {}", data.len(), address, id);
                }
                Err(err) => {
                    log::error!("Error writing register: {:?}", err);
                }
            }
        }
    }
}
