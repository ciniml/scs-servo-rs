use core::{borrow::Borrow, marker::PhantomData, time::Duration};

use crate::protocol::{ProtocolHandlerError, ProtocolMaster, ProtocolMasterConfig, WriteRegisterCommand};

use super::{Error, Instant, RegisterDefinition, RegisterStorage};
//                            Register Name,            Address,     R,     W,        Def, Description
define_register!(EEPROM, REGISTER_VERSION_H,               0x03,  true, false, None      , "Software Version H");
define_register!(EEPROM, REGISTER_VERSION_L,               0x04,  true, false, None      , "Software Version H");
define_register!(EEPROM, REGISTER_ID,                      0x05,  true,  true, Some(0x00), "ID");
define_register!(EEPROM, REGISTER_BAUD_RATE,               0x06,  true,  true, Some(0x00), "Baud Rate");
define_register!(EEPROM, REGISTER_RESPONSE_TIME,           0x07,  true,  true, Some(0x00), "Response Time");
define_register!(EEPROM, REGISTER_RESPONSE_ENABLE,         0x08,  true,  true, Some(0x01), "Response Enable");
define_register!(EEPROM, REGISTER_LOWER_POSITION_LIMIT_H,  0x09,  true,  true, Some(0x00), "Lower Position Limit H");
define_register!(EEPROM, REGISTER_LOWER_POSITION_LIMIT_L,  0x0a,  true,  true, Some(0x00), "Lower Position Limit L");
define_register!(EEPROM, REGISTER_UPPER_POSITION_LIMIT_H,  0x0b,  true,  true, Some(0x03), "Upper Position Limit H");
define_register!(EEPROM, REGISTER_UPPER_POSITION_LIMIT_L,  0x0c,  true,  true, Some(0xff), "Upper Position Limit L");
define_register!(EEPROM, REGISTER_UPPER_TEMPERATURE_LIMIT, 0x0d,  true,  true, Some(0x50), "Upper Temperature Limit");
define_register!(EEPROM, REGISTER_MAX_INPUT_VOLTAGE,       0x0e,  true,  true, Some(0xfa), "Max Input Voltage");
define_register!(EEPROM, REGISTER_MIN_INPUT_VOLTAGE,       0x0f,  true,  true, Some(0x32), "Min Input Voltage");
define_register!(EEPROM, REGISTER_MAX_TORQUE_H,            0x10,  true,  true, Some(0x03), "Max Torque H");
define_register!(EEPROM, REGISTER_MAX_TORQUE_L,            0x11,  true,  true, Some(0xff), "Max Torque L");
define_register!(EEPROM, REGISTER_HIGH_VOLTAGE_FLAG,       0x12,  true,  true, Some(0x00), "High Voltage Flag");
define_register!(EEPROM, REGISTER_ALARM_FLAG,              0x13,  true,  true, Some(0x25), "Alarm Flag");
define_register!(EEPROM, REGISTER_LED_ALARM_FLAG,          0x14,  true,  true, Some(0x25), "LED Alarm Flag");
define_register!(RAM,    REGISTER_TORQUE_SWITCH,           0x28,  true,  true, Some(0x00), "Torque Switch");
define_register!(RAM,    REGISTER_TARGET_POSITION_H,       0x2a,  true,  true, None      , "Target Position H");
define_register!(RAM,    REGISTER_TARGET_POSITION_L,       0x2b,  true,  true, None      , "Target Position L");
define_register!(RAM,    REGISTER_TARGET_PERIOD_H,         0x2c,  true,  true, Some(0x00), "Target Period H");
define_register!(RAM,    REGISTER_TARGET_PERIOD_L,         0x2d,  true,  true, Some(0x00), "Target Period L");
define_register!(RAM,    REGISTER_TARGET_SPEED_H,          0x2e,  true,  true, Some(0x00), "Target Speed H");
define_register!(RAM,    REGISTER_TARGET_SPEED_L,          0x2f,  true,  true, Some(0x00), "Target Speed L");
define_register!(RAM,    REGISTER_EEPROM_LOCK,             0x30,  true,  true, Some(0x01), "EEPROM Lock");
define_register!(RAM,    REGISTER_CURRENT_POSITION_H,      0x38,  true, false, None      , "Current Position H");
define_register!(RAM,    REGISTER_CURRENT_POSITION_L,      0x39,  true, false, None      , "Current Position L");
define_register!(RAM,    REGISTER_CURRENT_SPEED_H,         0x3a,  true, false, None      , "Current Speed H");
define_register!(RAM,    REGISTER_CURRENT_SPEED_L,         0x3b,  true, false, None      , "Current Speed L");
define_register!(RAM,    REGISTER_CURRENT_LOAD_H,          0x3c,  true, false, None      , "Current Load H");
define_register!(RAM,    REGISTER_CURRENT_LOAD_L,          0x3d,  true, false, None      , "Current Load L");
define_register!(RAM,    REGISTER_CURRENT_VOLTAGE,         0x3e,  true, false, None      , "Current Voltage");
define_register!(RAM,    REGISTER_CURRENT_TEMPERATURE,     0x3f,  true, false, None      , "Current Temperature");

pub const REGISTER_LIST: &[RegisterDefinition] = &[
    REGISTER_VERSION_H,
    REGISTER_VERSION_L,
    REGISTER_ID,
    REGISTER_BAUD_RATE,
    REGISTER_RESPONSE_TIME,
    REGISTER_RESPONSE_ENABLE,
    REGISTER_LOWER_POSITION_LIMIT_H,
    REGISTER_LOWER_POSITION_LIMIT_L,
    REGISTER_UPPER_POSITION_LIMIT_H,
    REGISTER_UPPER_POSITION_LIMIT_L,
    REGISTER_UPPER_TEMPERATURE_LIMIT,
    REGISTER_MAX_INPUT_VOLTAGE,
    REGISTER_MIN_INPUT_VOLTAGE,
    REGISTER_MAX_TORQUE_H,
    REGISTER_MAX_TORQUE_L,
    REGISTER_HIGH_VOLTAGE_FLAG,
    REGISTER_ALARM_FLAG,
    REGISTER_LED_ALARM_FLAG,
    REGISTER_TORQUE_SWITCH,
    REGISTER_TARGET_POSITION_H,
    REGISTER_TARGET_POSITION_L,
    REGISTER_TARGET_PERIOD_H,
    REGISTER_TARGET_PERIOD_L,
    REGISTER_TARGET_SPEED_H,
    REGISTER_TARGET_SPEED_L,
    REGISTER_EEPROM_LOCK,
    REGISTER_CURRENT_POSITION_H,
    REGISTER_CURRENT_POSITION_L,
    REGISTER_CURRENT_SPEED_H,
    REGISTER_CURRENT_SPEED_L,
    REGISTER_CURRENT_LOAD_H,
    REGISTER_CURRENT_LOAD_L,
    REGISTER_CURRENT_VOLTAGE,
    REGISTER_CURRENT_TEMPERATURE,
];

pub struct Scs0009ServoControl<R, W, Timer> {
    id: u8,
    reader: R,
    writer: W,
    master_config: ProtocolMasterConfig,
    timeout: Duration,
    current_values: Option<CurrentValues>,
    timer: PhantomData<Timer>,
}

struct CurrentValues {
    buffer: [u8; 8],
}
impl CurrentValues {
    fn new() -> Self {
        Self {
            buffer: [0; 8],
        }
    }
    fn position(&self) -> u16 {
        u16::from_be_bytes([self.buffer[0], self.buffer[1]])
    }
    fn speed(&self) -> i16 {
        let speed = u16::from_be_bytes([self.buffer[2], self.buffer[3]]);
        if speed >= 32768 {
            -((speed - 32768) as i16)
        } else {
            speed as i16
        }
    }
    fn load(&self) -> u16 {
        u16::from_be_bytes([self.buffer[4], self.buffer[5]])
    }
    #[allow(dead_code)]
    fn voltage(&self) -> u8 {
        self.buffer[6]
    }
    #[allow(dead_code)]
    fn temperature(&self) -> u8 {
        self.buffer[7]
    }
}

impl<R, W, Timer> Scs0009ServoControl<R, W, Timer> {
    pub fn new(id: u8, reader: R, writer: W, master_config: ProtocolMasterConfig, timeout: Duration) -> Self {
        Self {
            id,
            reader,
            writer,
            master_config,
            timeout,
            current_values: None,
            timer: PhantomData,
        }
    }
}

const COMMAND_BUFFER_SIZE: usize = 16;

impl<R, W, Timer> Scs0009ServoControl<R, W, Timer>
    where R: crate::protocol::StreamReader,
          W: crate::protocol::StreamWriter,
          Timer: super::Timer,
{
    fn read_continuous_registers(&mut self, address: u8, data: &mut [u8]) -> Result<(), ProtocolHandlerError<R::Error, W::Error>> {
        let mut master = ProtocolMaster::<COMMAND_BUFFER_SIZE>::new(self.master_config.clone());
        let start = Timer::now();
        master.read_register(&mut self.reader, &mut self.writer, self.id, address, data, || start.elapsed() >= self.timeout)?;
        Ok(())
    }
    fn write_continuous_registers(&mut self, address: u8, data: &[u8]) -> Result<(), ProtocolHandlerError<R::Error, W::Error>> {
        let mut master = ProtocolMaster::<COMMAND_BUFFER_SIZE>::new(self.master_config.clone());
        let mut command = WriteRegisterCommand::<COMMAND_BUFFER_SIZE>::new(self.id, address, data.len());
        command.writer().data_mut().unwrap()[2..2+data.len()].copy_from_slice(data);
        command.update_checksum().unwrap();
        let start = Timer::now();
        master.write_register(&mut self.reader, &mut self.writer, &command, || start.elapsed() >= self.timeout)?;
        Ok(())
    }
    #[allow(dead_code)]
    fn read_register_u8(&mut self, address: u8) -> Result<u8, ProtocolHandlerError<R::Error, W::Error>> {
        let mut data = [0];
        self.read_continuous_registers(address, &mut data)?;
        Ok(data[0])
    }
    fn read_register_u16(&mut self, address: u8) -> Result<u16, ProtocolHandlerError<R::Error, W::Error>> {
        let mut data = [0; 2];
        self.read_continuous_registers(address, &mut data)?;
        Ok(u16::from_be_bytes(data))
    }
    fn write_register_u8(&mut self, address: u8, value: u8) -> Result<(), ProtocolHandlerError<R::Error, W::Error>> {
        self.write_continuous_registers(address, &[value])
    }
    fn write_register_u16(&mut self, address: u8, value: u16) -> Result<(), ProtocolHandlerError<R::Error, W::Error>> {
        self.write_continuous_registers(address, &value.to_be_bytes())
    }
}

impl<R, W, Timer> super::ServoControl for Scs0009ServoControl<R, W, Timer>
    where R: crate::protocol::StreamReader,
          W: crate::protocol::StreamWriter,
          Timer: super::Timer,
{
    type Error = Error<ProtocolHandlerError<R::Error, W::Error>>;
    type Id = u8;
    type Position = u16;
    type Period = u16;
    type Speed = i16;
    type Torque = u16;
    
    fn id(&self) -> Self::Id {
        self.id
    }

    fn set_id(&mut self, id: Self::Id) -> Result<(), Self::Error> {
        self.write_register_u8(REGISTER_ID.address, id)?;
        self.id = id;
        Ok(())
    }

    fn output_enable(&mut self) -> Result<(), Self::Error> {
        self.write_register_u8(REGISTER_TORQUE_SWITCH.address, 0x01)?;
        Ok(())
    }

    fn output_disable(&mut self) -> Result<(), Self::Error> {
        self.write_register_u8(REGISTER_TORQUE_SWITCH.address, 0x00)?;
        Ok(())
    }

    fn position_lower_limit(&mut self) -> Result<Self::Position, Self::Error> {
        Ok(self.read_register_u16(REGISTER_LOWER_POSITION_LIMIT_H.address)?)
    }

    fn position_upper_limit(&mut self) -> Result<Self::Position, Self::Error> {
        Ok(self.read_register_u16(REGISTER_UPPER_POSITION_LIMIT_H.address)?)
    }

    fn target_position(&mut self) -> Result<Self::Position, Self::Error> {
        Ok(self.read_register_u16(REGISTER_TARGET_POSITION_H.address)?)
    }

    fn set_target_position(&mut self, position: Self::Position) -> Result<(), Self::Error> {
        Ok(self.write_register_u16(REGISTER_TARGET_POSITION_H.address, position)?)
    }

    fn target_period(&mut self) -> Result<Self::Period, Self::Error> {
        Ok(self.read_register_u16(REGISTER_TARGET_PERIOD_H.address)?)
    }

    fn set_target_period(&mut self, period: Self::Period) -> Result<(), Self::Error> {
        Ok(self.write_register_u16(REGISTER_TARGET_PERIOD_H.address, period)?)
    }

    fn target_speed(&mut self) -> Result<Self::Speed, Self::Error> {
        Ok(self.read_register_u16(REGISTER_TARGET_SPEED_H.address)? as i16)
    }

    fn set_target_speed(&mut self, speed: Self::Speed) -> Result<(), Self::Error> {
        Ok(self.write_register_u16(REGISTER_TARGET_SPEED_H.address, speed as u16)?)
    }

    fn current_position(&mut self) -> Result<Self::Position, Self::Error> {
        if let Some(values) = self.current_values.borrow() {
            Ok(values.position())
        } else {
            Err(Error::NotUpdated)
        }
    }

    fn current_speed(&mut self) -> Result<Self::Speed, Self::Error> {
        if let Some(values) = self.current_values.borrow() {
            Ok(values.speed())
        } else {
            Err(Error::NotUpdated)
        }
    }

    fn current_load(&mut self) -> Result<Self::Torque, Self::Error> {
        if let Some(values) = self.current_values.borrow() {
            Ok(values.load())
        } else {
            Err(Error::NotUpdated)
        }
    }

    fn update(&mut self) -> Result<(), Self::Error> {
        let mut values = CurrentValues::new();
        self.read_continuous_registers(REGISTER_CURRENT_POSITION_H.address, &mut values.buffer)?;
        self.current_values = Some(values);
        Ok(())
    }

    fn min_speed(&self) -> Self::Speed {
        0
    }
    fn max_speed(&self) -> Self::Speed {
        0x7fff
    }
    fn max_period(&self) -> Self::Period {
        0xffff
    }
    fn to_speed(&self, speed: f64) -> Result<Self::Speed, Self::Error> {
        let speed = speed / 0.19;
        if speed < 0.0 || speed > 65535.0 {
            Err(Error::InvalidArgument)
        } else {
            Ok(speed as Self::Speed)
        }
    }
    fn to_period(&self, period: f64) -> Result<Self::Period, Self::Error> {
        if period < 0.0 || period > 65.535 {
            Err(Error::InvalidArgument)
        } else {
            Ok((period * 1000.0) as Self::Period)
        }
        
    }

}


#[cfg(test)]
mod test {
    use super::*;
    use crate::device::ServoControl;
    use crate::{packet::PacketWriter, protocol::{Command, ProtocolMasterConfig, ProtocolSlave, ProtocolSlaveConfig}};
    extern crate std;
    
    #[test]
    fn test_scs0009() {
        let mut slave = ProtocolSlave::<256>::new(ProtocolSlaveConfig {});
        
        let (master_writer, mut slave_reader) = std::sync::mpsc::channel();
        let (mut slave_writer, master_reader) = std::sync::mpsc::channel();

        let register_storage = std::sync::Arc::new(std::sync::Mutex::new([0u8; 256]));
        let register_storage_clone = register_storage.clone();
        std::thread::spawn(move || {
            let register_storage = register_storage_clone;
            {
                let mut register_storage = register_storage.lock().unwrap();
                register_storage[REGISTER_ID.address as usize] = 0x01; // ID = 1
                register_storage[REGISTER_LOWER_POSITION_LIMIT_H.address as usize] = 0x00; // Lower Position Limit = 0x001f
                register_storage[REGISTER_LOWER_POSITION_LIMIT_L.address as usize] = 0x1f; // /
                register_storage[REGISTER_UPPER_POSITION_LIMIT_H.address as usize] = 0x03; // Upper Position Limit = 0x03ff
                register_storage[REGISTER_UPPER_POSITION_LIMIT_L.address as usize] = 0xff; // /
            }
            loop {
                match slave.process(&mut slave_reader, &mut slave_writer, |packet, buffer| {
                    std::println!("Received packet: {:?}", packet.id().unwrap());
                    let id = register_storage.lock().unwrap()[REGISTER_ID.address as usize];

                    if packet.id().unwrap() == id {
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
                            {
                                let register_storage = register_storage.lock().unwrap();
                                for i in 0..length {
                                    writer.data_mut().unwrap()[i as usize + 1] = register_storage[(start + i) as usize];
                                }
                            }
                            writer.update_checksum().unwrap();
                            Some(2 + 1 + length as usize + 3)
                        } else if data[0] == Command::WriteRegister as u8 {
                            let start = data[1] as usize;
                            let body = &data[2..];
                            let count = body.len();
                            {
                                let mut register_storage = register_storage.lock().unwrap();
                                register_storage[start..start+count].copy_from_slice(body);
                            }
                            writer.set_length(2).ok();
                            writer.data_mut().unwrap()[0] = 0;  // fixed
                            writer.update_checksum().unwrap();
                            Some(2 + 1 + 3)
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

        let mut control = Scs0009ServoControl::<_, _, std::time::Instant>::new(0x01, master_reader, master_writer, ProtocolMasterConfig { echo_back: false }, Duration::from_secs(2));
        // Check ID
        assert_eq!(control.id(), 0x01);
        // Limit
        assert_eq!(control.position_lower_limit().unwrap(), 0x001f);
        assert_eq!(control.position_upper_limit().unwrap(), 0x03ff);

        // Output Enable/Disable
        assert_eq!(register_storage.lock().unwrap()[REGISTER_TORQUE_SWITCH.address as usize], 0x00);
        control.output_enable().unwrap();
        assert_eq!(register_storage.lock().unwrap()[REGISTER_TORQUE_SWITCH.address as usize], 0x01);
        control.output_disable().unwrap();
        assert_eq!(register_storage.lock().unwrap()[REGISTER_TORQUE_SWITCH.address as usize], 0x00);
        let current_load = control.current_load();
        assert!(current_load.is_err());

        // Target Position
        assert_eq!(control.target_position().unwrap(), 0x0000);
        control.set_target_position(0x1234).unwrap();
        assert_eq!(register_storage.lock().unwrap()[REGISTER_TARGET_POSITION_H.address as usize], 0x12);
        assert_eq!(register_storage.lock().unwrap()[REGISTER_TARGET_POSITION_L.address as usize], 0x34);
        assert_eq!(control.target_position().unwrap(), 0x1234);

        // Target Period
        assert_eq!(control.target_period().unwrap(), 0x0000);
        control.set_target_period(0x5678).unwrap();
        assert_eq!(register_storage.lock().unwrap()[REGISTER_TARGET_PERIOD_H.address as usize], 0x56);
        assert_eq!(register_storage.lock().unwrap()[REGISTER_TARGET_PERIOD_L.address as usize], 0x78);
        assert_eq!(control.target_period().unwrap(), 0x5678);

        // Current status
        let current_load: Result<u16, Error<ProtocolHandlerError<(), ()>>> = control.current_load();
        assert!(current_load.is_err()); // Must fail because not updated
        control.update().unwrap();
        assert_eq!(control.current_load().unwrap(), 0);
        assert_eq!(control.current_position().unwrap(), 0);
        assert_eq!(control.current_speed().unwrap(), 0);
        
        register_storage.lock().unwrap()[REGISTER_CURRENT_LOAD_H.address as usize] = 0x01;
        register_storage.lock().unwrap()[REGISTER_CURRENT_LOAD_L.address as usize] = 0x23;
        register_storage.lock().unwrap()[REGISTER_CURRENT_POSITION_H.address as usize] = 0x45;
        register_storage.lock().unwrap()[REGISTER_CURRENT_POSITION_L.address as usize] = 0x67;
        register_storage.lock().unwrap()[REGISTER_CURRENT_SPEED_H.address as usize] = 0x89;
        register_storage.lock().unwrap()[REGISTER_CURRENT_SPEED_L.address as usize] = 0xab;
        control.update().unwrap();
        assert_eq!(control.current_load().unwrap(), 0x0123);
        assert_eq!(control.current_position().unwrap(), 0x4567);
        assert_eq!(control.current_speed().unwrap(), 0x89ab);

        register_storage.lock().unwrap()[REGISTER_CURRENT_LOAD_H.address as usize] = 0xcd;
        register_storage.lock().unwrap()[REGISTER_CURRENT_LOAD_L.address as usize] = 0xef;
        register_storage.lock().unwrap()[REGISTER_CURRENT_POSITION_H.address as usize] = 0xfe;
        register_storage.lock().unwrap()[REGISTER_CURRENT_POSITION_L.address as usize] = 0xdc;
        register_storage.lock().unwrap()[REGISTER_CURRENT_SPEED_H.address as usize] = 0xba;
        register_storage.lock().unwrap()[REGISTER_CURRENT_SPEED_L.address as usize] = 0x98;
        // Not updated, so the previous values are returned
        assert_eq!(control.current_load().unwrap(), 0x0123);
        assert_eq!(control.current_position().unwrap(), 0x4567);
        assert_eq!(control.current_speed().unwrap(), 0x89ab);
        control.update().unwrap();
        assert_eq!(control.current_load().unwrap(), 0xcdef);
        assert_eq!(control.current_position().unwrap(), 0xfedc);
        assert_eq!(control.current_speed().unwrap(), 0xba98);


        // Change ID
        control.set_id(0x02).unwrap();
        assert_eq!(control.id(), 0x02);
        assert_eq!(register_storage.lock().unwrap()[REGISTER_ID.address as usize], 0x02);
        control.output_enable().unwrap(); // Check if the new ID is used
        assert_eq!(register_storage.lock().unwrap()[REGISTER_TORQUE_SWITCH.address as usize], 0x01);
    }
}