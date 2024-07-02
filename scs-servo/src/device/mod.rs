use crate::protocol::ProtocolHandlerError;

#[derive(Debug, Clone, Copy)]
pub enum RegisterStorage {
    /// EEPROM
    Eeprom,
    /// RAM
    Ram,
}

#[derive(Debug, Clone, Copy)]
pub struct RegisterDefinition {
    pub address: u8,
    pub storage: RegisterStorage,
    pub readable: bool,
    pub writable: bool,
    pub default: Option<u8>,
    pub description: &'static str,
}

impl RegisterDefinition {
    pub const fn new(
        address: u8,
        storage: RegisterStorage,
        readable: bool,
        writable: bool,
        default: Option<u8>,
        description: &'static str,
    ) -> Self {
        Self {
            address,
            storage,
            readable,
            writable,
            default,
            description,
        }
    }
}

macro_rules! define_register {
    (RAM, $name:ident, $address:expr, $readable:expr, $writable:expr, $default:expr, $description:literal) => {
        #[allow(dead_code)]
        const $name: RegisterDefinition = RegisterDefinition::new($address, RegisterStorage::Ram, $readable, $writable, $default, $description);
    };
    (EEPROM, $name:ident, $address:expr, $readable:expr, $writable:expr, $default:expr, $description:literal) => {
        #[allow(dead_code)]
        const $name: RegisterDefinition = RegisterDefinition::new($address, RegisterStorage::Eeprom, $readable, $writable, $default, $description);
    };
}

pub trait ServoControl {
    type Error;
    type Id;
    type Period;
    type Position;
    type Speed;
    type Torque;

    fn min_speed(&self) -> Self::Speed;
    fn max_speed(&self) -> Self::Speed;
    fn max_period(&self) -> Self::Period;
    fn to_speed(&self, speed: f64) -> Result<Self::Speed, Self::Error>;
    fn to_period(&self, period: f64) -> Result<Self::Period, Self::Error>;

    fn id(&self) -> Self::Id;
    fn set_id(&mut self, id: Self::Id) -> Result<(), Self::Error>;

    fn output_enable(&mut self) -> Result<(), Self::Error> ;
    fn output_disable(&mut self) -> Result<(), Self::Error>;
    fn position_lower_limit(&mut self)  -> Result<Self::Position, Self::Error>;
    fn position_upper_limit(&mut self)  -> Result<Self::Position, Self::Error>;

    fn target_position(&mut self) -> Result<Self::Position, Self::Error>;
    fn set_target_position(&mut self, position: Self::Position) -> Result<(), Self::Error>;

    fn target_period(&mut self) -> Result<Self::Period, Self::Error>;
    fn set_target_period(&mut self, period: Self::Period) -> Result<(), Self::Error>;

    fn target_speed(&mut self) -> Result<Self::Speed, Self::Error>;
    fn set_target_speed(&mut self, speed: Self::Speed) -> Result<(), Self::Error>;

    
    fn current_position(&mut self) -> Result<Self::Position, Self::Error>;
    fn current_speed(&mut self) -> Result<Self::Speed, Self::Error>;
    fn current_load(&mut self) -> Result<Self::Torque, Self::Error>;

    fn update(&mut self) -> Result<(), Self::Error>;
}

pub trait Timer {
    type Instant : Instant;
    fn now() -> Self::Instant;
}
pub trait Instant {
    fn elapsed(&self) -> core::time::Duration;
}

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
impl Instant for std::time::Instant {
    fn elapsed(&self) -> core::time::Duration {
        std::time::Instant::now().duration_since(*self)
    }
}

#[cfg(feature = "std")]
impl Timer for std::time::Instant {
    type Instant = std::time::Instant;

    fn now() -> Self::Instant {
        std::time::Instant::now()
    }
}


#[derive(Debug)]
pub enum Error<ProtocolHandlerError> {
    ProtocolError(ProtocolHandlerError),
    InvalidArgument,
    NotUpdated,
}

impl<R, W> From<ProtocolHandlerError<R, W>> for Error<ProtocolHandlerError<R, W>> {
    fn from(err: ProtocolHandlerError<R, W>) -> Self {
        Error::ProtocolError(err)
    }
}

pub mod scs0009;