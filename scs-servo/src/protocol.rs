use crate::packet::{PacketError, PacketReader, PacketWriter};

pub trait StreamReader {
    type Error;
    fn read(&mut self, data: &mut [u8]) -> nb::Result<usize, Self::Error>;
}

pub trait StreamWriter {
    type Error;
    fn write(&mut self, data: &[u8]) -> nb::Result<usize, Self::Error>;
}

pub struct ProtocolReader<const BUFFER_SIZE: usize> {
    buffer: [u8; BUFFER_SIZE],
    position: usize,
    state: ReaderState,
}

#[derive(PartialEq)]
enum ReaderState {
    Marker1,
    Marker2,
    Header,
    Data,
    Completed,
}

#[derive(Debug)]
pub enum ProtocolReaderError<ReaderError> {
    ReaderError(ReaderError),
    PacketError(PacketError),
    InsufficientBuffer,
}

impl From<PacketError> for ProtocolReaderError<()> {
    fn from(error: PacketError) -> Self {
        Self::PacketError(error)
    }
}

impl<const BUFFER_SIZE: usize> ProtocolReader<BUFFER_SIZE> {
    pub fn new() -> Self {
        Self {
            buffer: [0; BUFFER_SIZE],
            position: 0,
            state: ReaderState::Marker1,
        }
    }

    fn read_inner<R: StreamReader>(&mut self, reader: &mut R) -> Result<(bool, bool), ProtocolReaderError<R::Error>> {
        let (new_state, position, fully_read) = match self.state {
            ReaderState::Marker1 | ReaderState::Completed => {
                let bytes_read = match reader.read(&mut self.buffer[0..2]) {
                    Ok(bytes_read) => bytes_read,
                    Err(nb::Error::WouldBlock) => 0,
                    Err(nb::Error::Other(err)) => return Err(ProtocolReaderError::ReaderError(err)),
                };
                let new_state = if bytes_read == 1 && self.buffer[0] == 0xff {
                    ReaderState::Marker2
                } else if bytes_read == 2 {
                    if self.buffer[0] == 0xff {
                        if self.buffer[1] == 0xff {
                            ReaderState::Header
                        } else {
                            ReaderState::Marker2
                        }
                    } else if self.buffer[1] == 0xff {
                        ReaderState::Marker2
                    } else {
                        ReaderState::Marker1
                    }
                } else {
                    ReaderState::Marker1
                };
                (new_state, 0, bytes_read == 2)
            }
            ReaderState::Marker2 => {
                let bytes_read = match reader.read(&mut self.buffer[0..1]) {
                    Ok(bytes_read) => bytes_read,
                    Err(nb::Error::WouldBlock) => 0,
                    Err(nb::Error::Other(err)) => return Err(ProtocolReaderError::ReaderError(err)),
                };
                let new_state = if bytes_read == 1 && self.buffer[0] == 0xff {
                    ReaderState::Header
                } else {
                    ReaderState::Marker1
                };
                (new_state, 0, bytes_read == 1)
            }
            ReaderState::Header => {
                let bytes_read = match reader.read(&mut self.buffer[self.position..2]) {
                    Ok(bytes_read) => bytes_read,
                    Err(nb::Error::WouldBlock) => 0,
                    Err(nb::Error::Other(err)) => return Err(ProtocolReaderError::ReaderError(err)),
                };
                let new_position = self.position + bytes_read;
                let new_state = if new_position == 2 {
                    let length = self.buffer[1] as usize;
                    if length + 2 > BUFFER_SIZE {
                        return Err(ProtocolReaderError::InsufficientBuffer);
                    } else {
                        ReaderState::Data
                    }
                } else {
                    ReaderState::Header
                };
                (new_state, new_position, bytes_read == 2)
            }
            ReaderState::Data => {
                let length = self.buffer[1] as usize;
                let end = length + 2;
                let bytes_to_read = end - self.position;
                let bytes_read = match reader.read(&mut self.buffer[self.position..end]) {
                    Ok(bytes_read) => bytes_read,
                    Err(nb::Error::WouldBlock) => 0,
                    Err(nb::Error::Other(err)) => return Err(ProtocolReaderError::ReaderError(err)),
                };
                let new_position = self.position + bytes_read;
                let new_state = if new_position == end {
                    ReaderState::Completed
                } else {
                    ReaderState::Data
                };
                (new_state, new_position, bytes_read == bytes_to_read)
            }
        };
        self.state = new_state;
        self.position = position;
        Ok((self.state == ReaderState::Completed, fully_read))
    }

    pub fn read<R: StreamReader>(&mut self, reader: &mut R) -> Result<bool, ProtocolReaderError<R::Error>> {
        loop {
            let (completed, fully_read) = self.read_inner(reader)?;
            if completed {
                return Ok(true);
            } else if !fully_read {
                return Ok(false);
            }
        }
    }

    pub fn packet(&self) -> Option<PacketReader> {
        if self.state == ReaderState::Completed {
            Some(PacketReader::new(&self.buffer[0..self.position]))
        } else {
            None
        }
    }
}

pub struct ProtocolMasterConfig {
    // The underlying reader receives command from this master.
    pub echo_back: bool,
}

pub struct ProtocolMaster<const BUFFER_SIZE: usize> {
    config: ProtocolMasterConfig,
    reader: ProtocolReader<BUFFER_SIZE>,
}

#[repr(u8)]
pub enum Command {
    ReadRegister = 0x02,
    WriteRegister = 0x03,
}

#[derive(Debug)]
pub enum ProtocolHandlerError<ReaderError, WriterError> {
    PacketError(PacketError),
    ReaderError(ReaderError),
    WriterError(WriterError),
    ProtocolReaderError(ProtocolReaderError<ReaderError>),
    UnexpectedPacketId(u8),
    UnexpectedLength(usize),
    TimedOut,
}
impl<ReaderError, WriterError> From<ProtocolReaderError<ReaderError>> for ProtocolHandlerError<ReaderError, WriterError> {
    fn from(error: ProtocolReaderError<ReaderError>) -> Self {
        Self::ProtocolReaderError(error)
    }
}

pub struct ReadRegisterCommand {
    pub raw: [u8; 8],
}
impl ReadRegisterCommand {
    pub fn new(id: u8, address: u8, length: u8) -> Self {
        let mut raw = [0; 8];
        {
            raw[0] = 0xff;  // Marker1
            raw[1] = 0xff;  // Marker2
            let mut writer = PacketWriter::new(&mut raw[2..]);
            writer.set_id(id).unwrap();
            writer.set_length(4).unwrap();
            let data = writer.data_mut().unwrap();
            data[0] = Command::ReadRegister as u8;
            data[1] = address;
            data[2] = length;
            writer.update_checksum().unwrap();
        }
        Self { raw }
    }
}

pub struct WriteRegisterCommand<const SIZE: usize> {
    pub raw: [u8; SIZE],
}

impl<const SIZE: usize> WriteRegisterCommand<SIZE> {
    pub fn new(id: u8, address: u8, length: usize) -> Self {
        let mut raw = [0; SIZE];
        {
            raw[0] = 0xff;  // Marker1
            raw[1] = 0xff;  // Marker2
            let mut writer = PacketWriter::new(&mut raw[2..]);
            writer.set_id(id).unwrap();
            writer.set_length(3 + length as u8).unwrap();
            let data = writer.data_mut().unwrap();
            data[0] = Command::WriteRegister as u8;
            data[1] = address;
            //data[2..].copy_from_slice(data);
            //writer.update_checksum().unwrap();
        }
        Self { raw }
    }
    pub fn len(&self) -> usize {
        self.reader().length_unchecked() as usize + 4
    }
    pub fn packet(&self) -> &[u8] {
        &self.raw[..self.len()]
    }
    pub fn reader(&self) -> PacketReader {
        PacketReader::new(&self.raw[2..])
    }
    pub fn writer(&mut self) -> PacketWriter {
        PacketWriter::new(&mut self.raw[2..])
    }
}

impl<const BUFFER_SIZE: usize> ProtocolMaster<BUFFER_SIZE> {
    pub fn new(config: ProtocolMasterConfig) -> Self {
        Self {
            config,
            reader: ProtocolReader::new(),
        }
    }

    pub fn read_register<R: StreamReader, W: StreamWriter, Timeout: FnMut() -> bool>(&mut self, reader: &mut R, writer: &mut W, id: u8, address: u8, buffer: &mut [u8], mut timeout: Timeout) -> Result<(), ProtocolHandlerError<R::Error, W::Error>> {
        let command = ReadRegisterCommand::new(id, address, buffer.len() as u8);
        let mut total_bytes_written = 0;
        while total_bytes_written < command.raw.len() {
            match writer.write(&command.raw[total_bytes_written..]) {
                Ok(bytes_written) => {
                    total_bytes_written += bytes_written;
                }
                Err(nb::Error::WouldBlock) => {
                    // TODO: wait for writer to be ready
                }
                Err(nb::Error::Other(err)) => {
                    return Err(ProtocolHandlerError::WriterError(err));
                }
            }
            if timeout() {
                return Err(ProtocolHandlerError::TimedOut);
            }
        }

        if self.config.echo_back {
            // Discard echo backed packet.
            while !self.reader.read(reader)? {
                if timeout() {
                    return Err(ProtocolHandlerError::TimedOut);
                }
            }
        }

        while !self.reader.read(reader)? {
            if timeout() {
                return Err(ProtocolHandlerError::TimedOut);
            }
        }

        let packet = self.reader.packet().unwrap();
        packet.verify_checksum().map_err(|err| ProtocolHandlerError::PacketError(err))?;
        let response_id = packet.id().map_err(|err| ProtocolHandlerError::PacketError(err))?;
        if response_id != id {
            return Err(ProtocolHandlerError::UnexpectedPacketId(response_id));
        }
        let data = packet.data().map_err(|err| ProtocolHandlerError::PacketError(err))?;
        if data.len() != buffer.len() + 1 {
            return Err(ProtocolHandlerError::UnexpectedLength(data.len()));
        }
        buffer.copy_from_slice(&data[1..]);
        Ok(())
    }

    pub fn write_register<R: StreamReader, W: StreamWriter, Timeout: FnMut() -> bool, const SIZE: usize>(&mut self, reader: &mut R, writer: &mut W, command: &WriteRegisterCommand<SIZE>, mut timeout: Timeout) -> Result<(), ProtocolHandlerError<R::Error, W::Error>> {
        let buffer = command.packet();
        let mut total_bytes_written = 0;
        while total_bytes_written < buffer.len() {
            match writer.write(&buffer[total_bytes_written..]) {
                Ok(bytes_written) => {
                    total_bytes_written += bytes_written;
                }
                Err(nb::Error::WouldBlock) => {
                    // TODO: wait for writer to be ready
                }
                Err(nb::Error::Other(err)) => {
                    return Err(ProtocolHandlerError::WriterError(err));
                }
            }
            if timeout() {
                return Err(ProtocolHandlerError::TimedOut);
            }
        }

        if self.config.echo_back {
            // Discard echo backed packet.
            while !self.reader.read(reader)? {
                if timeout() {
                    return Err(ProtocolHandlerError::TimedOut);
                }
            }
        }

        while !self.reader.read(reader)? {
            if timeout() {
                return Err(ProtocolHandlerError::TimedOut);
            }
        }

        let packet = self.reader.packet().unwrap();
        packet.verify_checksum().map_err(|err| ProtocolHandlerError::PacketError(err))?;
        let response_id = packet.id().map_err(|err| ProtocolHandlerError::PacketError(err))?;
        if response_id != command.reader().id().unwrap() {
            return Err(ProtocolHandlerError::UnexpectedPacketId(response_id));
        }
        // TODO: Check the write response.
        Ok(())
    }
}


pub struct ProtocolSlaveConfig {
}

pub struct ProtocolSlave<const BUFFER_SIZE: usize> {
    #[allow(dead_code)]
    config: ProtocolSlaveConfig,
    reader: ProtocolReader<BUFFER_SIZE>,
    response_buffer: [u8; BUFFER_SIZE],
    response_position: usize,
    response_length: usize,
    state: ProtocolSlaveState,
}

enum ProtocolSlaveState {
    Idle,
    ProcessCommand,
    SendResponse,
}

impl<const BUFFER_SIZE: usize> ProtocolSlave<BUFFER_SIZE> {
    pub fn new(config: ProtocolSlaveConfig) -> Self {
        Self {
            config,
            reader: ProtocolReader::new(),
            response_buffer: [0; BUFFER_SIZE],
            response_position: 0,
            response_length: 0,
            state: ProtocolSlaveState::Idle,
        }
    }

    pub fn reset(&mut self) {
        self.state = ProtocolSlaveState::Idle;
    }

    pub fn process<R: StreamReader, W: StreamWriter, PacketHandler: FnMut(&PacketReader, &mut [u8]) -> Option<usize>>(&mut self, reader: &mut R, writer: &mut W, mut handler: PacketHandler) -> Result<(), ProtocolHandlerError<R::Error, W::Error>> {
        self.state = match self.state {
            ProtocolSlaveState::Idle => {
                match self.reader.read(reader) {
                    Ok(true) => ProtocolSlaveState::ProcessCommand,
                    Ok(false) => ProtocolSlaveState::Idle,
                    Err(err) => return Err(ProtocolHandlerError::ProtocolReaderError(err)),
                }
            },
            ProtocolSlaveState::ProcessCommand => {
                let packet = self.reader.packet().unwrap();
                if packet.verify_checksum().is_err() {
                    ProtocolSlaveState::Idle
                } else {
                    match handler(&packet, &mut self.response_buffer) {
                        Some(length) => {
                            self.response_position = 0;
                            self.response_length = length;
                            ProtocolSlaveState::SendResponse
                        },
                        None => ProtocolSlaveState::Idle,
                    }
                }
            },
            ProtocolSlaveState::SendResponse => {
                while self.response_position < self.response_length {
                    let buffer = &self.response_buffer[self.response_position..self.response_length];
                    let bytes_to_write = self.response_length - self.response_position;
                    match writer.write(buffer) {
                        Ok(bytes_written) => {
                            self.response_position += bytes_written;
                            if bytes_to_write != bytes_written {
                                break; 
                            }
                        },
                        Err(nb::Error::WouldBlock) => {
                            break;
                        },
                        Err(nb::Error::Other(err)) => {
                            return Err(ProtocolHandlerError::WriterError(err));
                        },
                    }
                }
                if self.response_position == self.response_length {
                    ProtocolSlaveState::Idle
                } else {
                    ProtocolSlaveState::SendResponse
                }
            },
        };

        Ok(())
    }
}

pub struct StreamWrapper<'a, T> {
    inner: &'a mut T,
}
impl<'a, T> StreamWrapper<'a, T> {
    pub fn new(inner: &'a mut T) -> Self {
        Self { inner }
    }
}

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
impl<'a, T: std::io::Read> StreamReader for StreamWrapper<'a, T> {
    type Error = std::io::Error;
    fn read(&mut self, data: &mut [u8]) -> nb::Result<usize, Self::Error> {
        std::io::Read::read(self.inner, data).map_err(|err| nb::Error::Other(err))
    }
}
#[cfg(feature = "std")]
impl<'a, T: std::io::Write> StreamWriter for StreamWrapper<'a, T> {
    type Error = std::io::Error;
    fn write(&mut self, data: &[u8]) -> nb::Result<usize, Self::Error> {
        std::io::Write::write(self.inner, data).map_err(|err| nb::Error::Other(err))
    }
}

#[cfg(feature = "std")]
impl StreamReader for std::sync::mpsc::Receiver<u8> {
    type Error = ();
    fn read(&mut self, data: &mut [u8]) -> nb::Result<usize, Self::Error> {
        let mut bytes_read = 0;
        for i in 0..data.len() {
            match self.try_recv() {
                Ok(byte) => {
                    data[i] = byte;
                    bytes_read += 1;
                },
                Err(std::sync::mpsc::TryRecvError::Empty) => { 
                    if bytes_read == 0 {
                        return Err(nb::Error::WouldBlock);
                    } else {
                        break;
                    }
                },
                Err(_err) => return Err(nb::Error::Other(())),
            }
        }
        Ok(bytes_read)
    }

}

#[cfg(feature = "std")]
impl StreamWriter for std::sync::mpsc::Sender<u8> {
    type Error = ();
    fn write(&mut self, data: &[u8]) -> nb::Result<usize, Self::Error> {
        let mut bytes_written = 0;
        for byte in data {
            match self.send(*byte) {
                Ok(()) => { bytes_written += 1; },
                Err(_err) => return Err(nb::Error::Other(())),
            }
        }
        Ok(bytes_written)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    extern crate std;
    use std::io::Cursor;
    
    #[test]
    fn test_protocol_reader() {
        let mut reader = ProtocolReader::<8>::new();
        let mut raw = [0xff, 0xff, 0x01, 0x05, 0x03, 0x2a, 0x00, 0x14, 0xb8];
        let mut stream = Cursor::new(raw);
        let mut stream = StreamWrapper::new(&mut stream);

        let result = reader.read(&mut stream);
        assert!(result.unwrap());
        let packet = reader.packet().unwrap();
        assert!(packet.verify_checksum().is_ok());
        assert_eq!(packet.id().unwrap(), 0x01);
        assert_eq!(packet.length().unwrap(), 0x05);
        assert_eq!(packet.data().unwrap(), &[0x03, 0x2a, 0x00, 0x14]);
    }

    #[test]
    fn test_protocol_reader_insuffucient_buffer() {
        let mut reader = ProtocolReader::<5>::new();
        let mut raw = [0xff, 0xff, 0x01, 0x05, 0x03, 0x2a, 0x00, 0x14, 0xb8];
        let mut stream = Cursor::new(raw);
        let mut stream = StreamWrapper::new(&mut stream);

        let result = reader.read(&mut stream);
        match result {
            Err(ProtocolReaderError::InsufficientBuffer) => {}
            _ => panic!("Unexpected result: {:?}", result),
        }
    }

    #[test]
    fn test_protocol_reader_two_phase() {
        let mut reader = ProtocolReader::<8>::new();
        let mut buffer = [0; 8];
        let mut raw = [0xff, 0xff, 0x01, 0x05, 0x03, 0x2a, 0x00, 0x14, 0xb8];
        let mut stream = Cursor::new(&raw[0..4]);
        let mut stream = StreamWrapper::new(&mut stream);

        let result = reader.read(&mut stream);
        assert!(!result.unwrap());

        let mut stream = Cursor::new(&raw[4..]);
        let mut stream = StreamWrapper::new(&mut stream); 
        let result = reader.read(&mut stream);
        assert!(result.unwrap());
        
        let packet = reader.packet().unwrap();
        assert!(packet.verify_checksum().is_ok());
        assert_eq!(packet.id().unwrap(), 0x01);
        assert_eq!(packet.length().unwrap(), 0x05);
        assert_eq!(packet.data().unwrap(), &[0x03, 0x2a, 0x00, 0x14]);
    }

    #[test]
    fn test_protocol_reader_garbages() {
        let mut reader = ProtocolReader::<8>::new();
        let mut buffer = [0; 8];
        let mut raw = [0x01, 0xff, 0x00, 0xff, 0xff, 0x01, 0x05, 0x03, 0x2a, 0x00, 0x14, 0xb8];
        let mut stream = Cursor::new(&raw);
        let mut stream = StreamWrapper::new(&mut stream);

        let result = reader.read(&mut stream);
        assert!(result.unwrap());
        
        let packet = reader.packet().unwrap();
        assert!(packet.verify_checksum().is_ok());
        assert_eq!(packet.id().unwrap(), 0x01);
        assert_eq!(packet.length().unwrap(), 0x05);
        assert_eq!(packet.data().unwrap(), &[0x03, 0x2a, 0x00, 0x14]);
    }
    #[test]
    fn test_protocol_reader_two_packets() {
        let mut reader = ProtocolReader::<8>::new();
        let mut buffer = [0; 8];
        let mut raw = [0xff, 0xff, 0x01, 0x05, 0x03, 0x2a, 0x00, 0x14, 0xb8, 0xff, 0xff, 0x01, 0x05, 0x03, 0x2a, 0x00, 0x14, 0xb8];
        let mut stream = Cursor::new(&raw);
        let mut stream = StreamWrapper::new(&mut stream);

        let result = reader.read(&mut stream);
        assert!(result.unwrap());
        
        let packet = reader.packet().unwrap();
        assert!(packet.verify_checksum().is_ok());
        assert_eq!(packet.id().unwrap(), 0x01);
        assert_eq!(packet.length().unwrap(), 0x05);
        assert_eq!(packet.data().unwrap(), &[0x03, 0x2a, 0x00, 0x14]);
        
        let result = reader.read(&mut stream);
        assert!(result.unwrap());
        
        let packet = reader.packet().unwrap();
        assert!(packet.verify_checksum().is_ok());
        assert_eq!(packet.id().unwrap(), 0x01);
        assert_eq!(packet.length().unwrap(), 0x05);
        assert_eq!(packet.data().unwrap(), &[0x03, 0x2a, 0x00, 0x14]);
    }

    #[test]
    fn test_protocol_reader_two_packets_with_garbage() {
        let mut reader = ProtocolReader::<8>::new();
        let mut buffer = [0; 8];
        let mut raw = [0xff, 0xff, 0x01, 0x05, 0x03, 0x2a, 0x00, 0x14, 0xb8, 0x00, 0xff, 0x01, 0xff, 0xff, 0x01, 0x05, 0x03, 0x2a, 0x00, 0x14, 0xb8];
        let mut stream = Cursor::new(&raw);
        let mut stream = StreamWrapper::new(&mut stream);

        let result = reader.read(&mut stream);
        assert!(result.unwrap());
        
        let packet = reader.packet().unwrap();
        assert!(packet.verify_checksum().is_ok());
        assert_eq!(packet.id().unwrap(), 0x01);
        assert_eq!(packet.length().unwrap(), 0x05);
        assert_eq!(packet.data().unwrap(), &[0x03, 0x2a, 0x00, 0x14]);
        
        let result = reader.read(&mut stream);
        assert!(result.unwrap());
        
        let packet = reader.packet().unwrap();
        assert!(packet.verify_checksum().is_ok());
        assert_eq!(packet.id().unwrap(), 0x01);
        assert_eq!(packet.length().unwrap(), 0x05);
        assert_eq!(packet.data().unwrap(), &[0x03, 0x2a, 0x00, 0x14]);
    }

    #[test]
    fn test_protocol_master() {
        let mut master = ProtocolMaster::<256>::new(ProtocolMasterConfig { echo_back: false });
        let mut slave = ProtocolSlave::<256>::new(ProtocolSlaveConfig {});
        
        let (mut master_writer, mut slave_reader) = std::sync::mpsc::channel();
        let (mut slave_writer, mut master_reader) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            loop {
                match slave.process(&mut slave_reader, &mut slave_writer, |packet, buffer| {
                    std::println!("Received packet: {:?}", packet.id().unwrap());
                    if packet.id().unwrap() == 0x01 {
                        let data = packet.data().unwrap();
                        buffer[0] = 0xff;
                        buffer[1] = 0xff;
                        let mut writer = PacketWriter::new(&mut buffer[2..]);
                        writer.set_id(packet.id().unwrap()).ok();
                        if data[0] == Command::ReadRegister as u8 {
                            let start = data[1];
                            let length = data[2];
                            writer.set_length(1 + length + 1).ok();
                            writer.data_mut().unwrap()[0] = 0;  // fixed
                            for i in 0..length {
                                writer.data_mut().unwrap()[i as usize + 1] = start + i;
                            }
                            writer.update_checksum().unwrap();
                            Some(2 + 1 + length as usize + 3)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }) {
                    Ok(()) => {},
                    Err(err) => {
                        std::println!("Error: {:?}", err);
                        break;
                    }
                }
            }
        });

        let mut buffer = [0; 4];
        let start_time = std::time::Instant::now();
        let result = master.read_register(&mut master_reader, &mut master_writer, 0x01, 0x10, &mut buffer, || std::time::Instant::now() - start_time > std::time::Duration::from_secs(1));
        assert!(result.is_ok(), "Error: {:?}", result);
        assert_eq!(buffer, [0x10, 0x11, 0x12, 0x13]);

        let result = master.read_register(&mut master_reader, &mut master_writer, 0x01, 0x20, &mut buffer, || std::time::Instant::now() - start_time > std::time::Duration::from_secs(1));
        assert!(result.is_ok(), "Error: {:?}", result);
        assert_eq!(buffer, [0x20, 0x21, 0x22, 0x23]);
    }
}