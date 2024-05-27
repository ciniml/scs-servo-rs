use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};


#[derive(Debug, Parser)]
#[clap(name = env!("CARGO_PKG_NAME"), version = env!("CARGO_PKG_VERSION"), author = env!("CARGO_PKG_AUTHORS"), about = env!("CARGO_PKG_DESCRIPTION"), arg_required_else_help = true)]
struct Cli {
    #[clap(subcommand)]
    subcommand: SubCommands,

    #[clap(short, long, help = "The serial port to use")]
    port: String,
    #[clap(short, long, help = "The baud rate to use")]
    baud: u32,
    #[clap(short, long, help = "The serial adapter echoes back sent data", default_value = "false")]
    echo: bool,
}

#[derive(Debug, Subcommand)]
enum SubCommands {
    Scan,
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
    env_logger::init();
    let cli = Cli::parse();

    match cli.subcommand {
        SubCommands::Scan => {
            log::info!("Scanning for servos on port {} at baud rate {}", cli.port, cli.baud);
            let serial = serialport::new(cli.port, cli.baud)
                .open()
                .expect("Failed to open serial port");
            let serial = std::cell::RefCell::new(serial);
            serial.borrow_mut().set_timeout(std::time::Duration::from_millis(10)).expect("Failed to set timeout");
            let mut reader = SerialReader { serial: &serial };
            let mut writer = SerialWriter { serial: &serial };
            let config = scs_servo::protocol::ProtocolMasterConfig {
                echo_back: cli.echo,
            };
            let mut master: scs_servo::protocol::ProtocolMaster<8> = scs_servo::protocol::ProtocolMaster::new(config);
            let progress_bar = ProgressBar::new(254);
            progress_bar.set_style(ProgressStyle::default_bar().template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}").unwrap());
            progress_bar.set_message("Scanning...");

            for id in 1..254 {
                let start = std::time::Instant::now();
                let mut buffer = [0; 3];
                match master.read_register(&mut reader, &mut writer, id, 0x03, &mut buffer, || start.elapsed().as_millis() > 10) {
                    Ok(_) => {
                        log::info!("Found servo with ID {} version {:02X} {:02X}", id, buffer[0], buffer[1]);
                    }
                    Err(err) => {
                        log::debug!("Err with ID {} {:?}", id, err);
                    }
                }
                progress_bar.inc(1);
            }
        }
    }
}
