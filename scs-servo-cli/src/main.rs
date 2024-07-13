use std::io::{Write};

use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use scs_servo::{device::{scs0009::Scs0009ServoControl, ServoControl}, protocol::ProtocolMasterConfig};


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

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum DeviceModel {
    Scs0009,
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
    Control {
        #[clap(short, long, help = "The servo ID", value_parser = id_in_range)]
        id: u8,
        #[clap(short, long, help = "The device model")]
        model: DeviceModel,

        #[clap(subcommand)]
        control: Control,
    },
}

fn valid_range(s: &str, min: f64, max: f64) -> Result<f64, String> {
    let value = s.parse::<f64>().map_err(|_| "Invalid number".to_string())?;
    if value < min || value > max {
        Err(format!("Value must be between {} and {}", min, max))
    } else {
        Ok(value)
    }
}

fn valid_ratio(s: &str) -> Result<f64, String> {
    valid_range(s, 0.0, 1.0)
}

fn valid_sampling_interval(s: &str) -> Result<f64, String> {
    valid_range(s, 1.0e-2, 1.0)
}

fn valid_sampling_timeout(s: &str) -> Result<f64, String> {
    valid_range(s, 1.0, 30.0)
}

#[derive(Debug, Subcommand)]
enum Control {
    SetId {
        #[clap(short, long, help = "The new servo ID", value_parser = id_in_range)]
        new_id: u8,
    },
    SetPosition {
        #[clap(short, long, help = "The new position", value_parser = valid_ratio)]
        position: f64,
        #[clap(short, long, help = "The time to reach the position in seconds")]
        time: Option<f64>,
        #[clap(short, long, help = "The speed to reach the position in degrees per second")]
        speed: Option<f64>,
        #[clap(long, help = "The sampling interval in seconds", value_parser = valid_sampling_interval)]
        sampling_interval: Option<f64>,
        #[clap(long, help = "Timeout to end sampling.", value_parser = valid_sampling_timeout, default_value = "10")]
        sampling_timeout: f64,
        #[clap(long, help = "The file to write the sampling output to.")]
        sampling_output: Option<String>,
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

    match cli.subcommand {
        SubCommands::Scan => {
            let mut master: scs_servo::protocol::ProtocolMaster<8> = scs_servo::protocol::ProtocolMaster::new(config);
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
            let mut master: scs_servo::protocol::ProtocolMaster<8> = scs_servo::protocol::ProtocolMaster::new(config);
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
            let mut master: scs_servo::protocol::ProtocolMaster<8> = scs_servo::protocol::ProtocolMaster::new(config);
            match master.write_register(&mut reader, &mut writer, &command, || start.elapsed().as_millis() > cli.timeout_ms as u128) {
                Ok(_) => {
                    log::info!("Wrote {} bytes to register {:02X} on servo {}", data.len(), address, id);
                }
                Err(err) => {
                    log::error!("Error writing register: {:?}", err);
                }
            }
        }
        SubCommands::Control { id, model, control } => {
            let _model = model; // Currently unused.
            let mut servo_control = Scs0009ServoControl::<_, _, std::time::Instant>::new(id, reader, writer, ProtocolMasterConfig { echo_back: cli.echo }, std::time::Duration::from_secs(2));
            match control {
                Control::SetId { new_id } => {
                    servo_control.set_id(new_id).expect("Failed to set ID");
                }
                Control::SetPosition { position, time, speed, sampling_interval , sampling_timeout, sampling_output} => {
                    let period = match time {
                        Some(time) => {
                            servo_control.to_period(time).expect("Invalid time")
                        },
                        None => { 0 },
                    };
                    let speed = match speed {
                        Some(speed) => {
                            servo_control.to_speed(speed).expect("Invalid speed")
                        },
                        None => { 0 },
                    };
                    servo_control.set_target_period(period).expect("Failed to set period");
                    servo_control.set_target_speed(speed).expect("Failed to set speed");

                    let lower_limit = servo_control.position_lower_limit().expect("Failed to get lower limit") as f64;
                    let upper_limit = servo_control.position_upper_limit().expect("Failed to get upper limit") as f64;
                    let position_raw = ((upper_limit - lower_limit) * position + lower_limit) as u16;
                    servo_control.set_target_position(position_raw).expect("Failed to set position");

                    if let Some(sampling_interval) = sampling_interval {
                        let output_writer = match sampling_output {
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

                        let sampling_interval = std::time::Duration::from_secs_f64(sampling_interval);
                        let mut last_update = std::time::Instant::now();
                        let start_time = std::time::Instant::now();
                        while std::time::Instant::now().duration_since(start_time) < std::time::Duration::from_secs_f64(sampling_timeout) {
                            let now = std::time::Instant::now();
                            let elapsed = now.duration_since(last_update);
                            if elapsed >= sampling_interval {
                                last_update = now;
                                servo_control.update().expect("Failed to update");
                                let current_position = servo_control.current_position().expect("Failed to get current position");
                                let current_speed = servo_control.current_speed().expect("Failed to get current speed");
                                let current_load = servo_control.current_load().expect("Failed to get current load");
                                let total_elapsed = now.duration_since(start_time).as_secs_f64();
                                writeln!(&mut output_writer, "{},{},{},{}", total_elapsed, current_position, current_speed, current_load).ok();

                                if current_position == position_raw {
                                    break;
                                }
                            }
                        }
                    }
                }
            }

        }
    }
}
