pub struct PacketReader<'a> {
    raw: &'a [u8],
}
impl<'a> PacketReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { raw: data }
    }
    
    pub fn id_unchecked(&self) -> u8 {
        self.raw[0]
    }
    pub fn length_unchecked(&self) -> u8 {
        self.raw[1]
    }

    pub fn checksum_unchecked(&self) -> u8 {
        self.raw[self.length_unchecked() as usize + 1]
    }

    fn check_header_length(&self) -> Result<(), PacketError> {
        if self.raw.len() < 3 {
            return Err(PacketError::InvalidHeader);
        }
        Ok(())
    }

    pub fn id(&self) -> Result<u8, PacketError> {
        self.check_header_length()?;
        Ok(self.id_unchecked())
    }
    pub fn length(&self) -> Result<u8, PacketError> {
        self.check_header_length()?;
        Ok(self.length_unchecked())
    }
    pub fn checksum(&self) -> Result<u8, PacketError> {
        self.check_header_length()?;
        Ok(self.checksum_unchecked())
    }
    pub fn data(&self) -> Result<&[u8], PacketError> {
        self.check_header_length()?;
        let length = self.length_unchecked() as usize;
        if length + 2 > self.raw.len() {
            return Err(PacketError::InvalidLength);
        }
        Ok(&self.raw[2..length + 2 - 1])
    }

    pub fn calculate_checksum(&self) -> Result<u8, PacketError> {
        self.check_header_length()?;
        let mut checksum = 0u8;
        for i in 0..self.length_unchecked() as usize + 2 - 1 {
            checksum = checksum.wrapping_add(self.raw[i as usize]);
        }
        Ok(!checksum)
    }

    pub fn verify_checksum(&self) -> Result<(), PacketError> {
        if self.checksum()? != self.calculate_checksum()? {
            return Err(PacketError::InvalidChecksum);
        }
        Ok(())
    }
}

pub struct PacketWriter<'a> {
    data: &'a mut [u8],
}
impl<'a> PacketWriter<'a> {
    pub fn new(data: &'a mut [u8]) -> Self {
        Self { data }
    }

    pub fn id_unchecked(&self) -> u8 {
        self.data[0]
    }
    pub fn length_unchecked(&self) -> u8 {
        self.data[1]
    }
    pub fn data_unchecked(&self, index: usize) -> u8 {
        self.data[index + 2]
    }
    pub fn checksum_unchecked(&self) -> u8 {
        self.data[self.length_unchecked() as usize + 1]
    }

    fn check_header_length(&self) -> Result<(), PacketError> {
        if self.data.len() < 3 {
            return Err(PacketError::InvalidHeader);
        }
        if self.length_unchecked() as usize > self.data.len() - 2 {
            return Err(PacketError::InvalidLength);
        }
        Ok(())
    }

    pub fn id(&self) -> Result<u8, PacketError> {
        self.check_header_length()?;
        Ok(self.id_unchecked())
    }
    pub fn length(&self) -> Result<u8, PacketError> {
        self.check_header_length()?;
        Ok(self.length_unchecked())
    }
    pub fn data(&self) -> Result<&[u8], PacketError> {
        self.check_header_length()?;
        let length = self.length_unchecked() as usize;
        if length + 2 > self.data.len() {
            return Err(PacketError::InvalidLength);
        }
        Ok(&self.data[2..length + 2 - 1])
    }

    pub fn checksum(&self) -> Result<u8, PacketError> {
        self.check_header_length()?;
        Ok(self.checksum_unchecked())
    }

    pub fn set_id(&mut self, id: u8) -> Result<(), PacketError> {
        self.check_header_length()?;
        self.data[0] = id;
        Ok(())
    }
    pub fn set_length(&mut self, length: u8) -> Result<(), PacketError> {
        if self.data.len() < 3 {
            return Err(PacketError::InvalidHeader);
        }
        self.data[1] = length;
        Ok(())
    }
    pub fn data_mut(&mut self) -> Result<&mut [u8], PacketError> {
        self.check_header_length()?;
        let length = self.length_unchecked() as usize;
        if length + 2 > self.data.len() {
            return Err(PacketError::InvalidLength);
        }
        Ok(&mut self.data[2..length + 2 - 1])
    }

    pub fn calculate_checksum(&self) -> Result<u8, PacketError> {
        self.check_header_length()?;
        let mut checksum = 0u8;
        for i in 0..self.length_unchecked() as usize + 2 - 1 {
            checksum = checksum.wrapping_add(self.data[i as usize]);
        }
        Ok(!checksum)
    }

    pub fn update_checksum(&mut self) -> Result<(), PacketError> {
        self.check_header_length()?;
        self.data[self.length_unchecked() as usize + 1] = self.calculate_checksum()?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum PacketError {
    InvalidHeader,
    InvalidChecksum,
    InvalidLength,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_packet_reader_valid() {
        let data = [0x01, 0x05, 0x03, 0x2a, 0x00, 0x14, 0xb8];
        let reader = PacketReader::new(&data);
        assert_eq!(reader.id_unchecked(), 0x01);
        assert_eq!(reader.length_unchecked(), 0x05);
        assert_eq!(reader.checksum_unchecked(), 0xb8);
        assert_eq!(reader.id().unwrap(), 0x01);
        assert_eq!(reader.length().unwrap(), 0x05);
        assert_eq!(reader.checksum().unwrap(), 0xb8);
        assert_eq!(reader.data().unwrap(), &[0x03, 0x2a, 0x00, 0x14]);
        assert_eq!(reader.calculate_checksum().unwrap(), 0xb8);
        assert_eq!(reader.verify_checksum().is_ok(), true);
    }
    #[test]
    fn test_packet_reader_checksum_error() {
        let data = [0x01, 0x05, 0x03, 0x2a, 0x00, 0x14, 0xb7];
        let reader = PacketReader::new(&data);
        assert_eq!(reader.calculate_checksum().unwrap(), 0xb8);
        assert_eq!(reader.verify_checksum().is_ok(), false);
    }

    #[test]
    fn test_packet_reader_length_error() {
        let data = [0x01, 0x00];
        let reader = PacketReader::new(&data);
        assert_eq!(reader.id().is_err(), true);
        assert_eq!(reader.length().is_err(), true);
        assert_eq!(reader.checksum().is_err(), true);
        assert_eq!(reader.data().is_err(), true);
        assert_eq!(reader.verify_checksum().is_err(), true);
    }

    #[test]
    fn test_packet_writer_valid() {
        let mut data = [0x00; 7];
        {
            let mut writer = PacketWriter::new(&mut data);
            writer.set_id(0x01).unwrap();
            writer.set_length(0x05).unwrap();
            writer.data_mut().unwrap()[0] = 0x03;
            writer.data_mut().unwrap()[1] = 0x2a;
            writer.data_mut().unwrap()[2] = 0x00;
            writer.data_mut().unwrap()[3] = 0x14;
            writer.update_checksum().unwrap();
            assert_eq!(writer.id_unchecked(), 0x01);
            assert_eq!(writer.length_unchecked(), 0x05);
            assert_eq!(writer.checksum_unchecked(), 0xb8);
            assert_eq!(writer.id().unwrap(), 0x01);
            assert_eq!(writer.length().unwrap(), 0x05);
            assert_eq!(writer.checksum().unwrap(), 0xb8);
            assert_eq!(writer.data().unwrap(), &[0x03, 0x2a, 0x00, 0x14]);
            assert_eq!(writer.calculate_checksum().unwrap(), 0xb8);
        }
        // check with reader
        let reader = PacketReader::new(&data);
        assert_eq!(reader.id_unchecked(), 0x01);
        assert_eq!(reader.length_unchecked(), 0x05);
        assert_eq!(reader.data().unwrap(), &[0x03, 0x2a, 0x00, 0x14]);
        assert_eq!(reader.verify_checksum().is_ok(), true);
    }
}