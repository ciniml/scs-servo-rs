mod utils;

use std::convert::TryFrom;
use futures::{pin_mut, FutureExt};
use web_time::{Duration, Instant, SystemTime};

use js_sys::{Uint16Array, Uint8Array};
use scs_servo::protocol::{ProtocolMaster, ProtocolMasterConfig, StreamReader, StreamReaderAsync, StreamWriterAsync, WriteRegisterCommand};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{spawn_local, JsFuture};

use wasm_streams::{ReadableStream, WritableStream};
use web_sys::{SerialPort, SerialOptions};

#[wasm_bindgen]
pub fn start() {
    wasm_logger::init(wasm_logger::Config::default());
}

pub async fn delay_ms(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        web_sys::Window::set_timeout_with_callback_and_timeout_and_arguments_0(&web_sys::window().unwrap(), &resolve, ms);
    });
    let _ = JsFuture::from(promise).await;
}

struct ReadableStreamWrapper {
    stream: ReadableStream,
    buffer: Vec<u8>,
    position: usize,
}

impl ReadableStreamWrapper {
    fn new(stream: ReadableStream) -> Self {
        Self { stream, buffer: Vec::new(), position: 0}
    }
}

impl StreamReaderAsync for ReadableStreamWrapper {
    type Error = JsValue;
    async fn read(&mut self, data: &mut [u8]) -> Result<usize, Self::Error> {
        if data.len() == 0 {
            return Ok(0);
        }
        let bytes_remaining = self.buffer.len() - self.position;
        if bytes_remaining < data.len() {
            {
                let timed_out = {
                    let mut reader = self.stream.get_reader();
                    let read_future = reader.read().fuse();
                    let delay = delay_ms(10).fuse();
                    pin_mut!(read_future, delay);
                    futures::select! {
                        result = read_future => {
                            if let Some(chunk) = result? {
                                if let Ok(buffer) = js_sys::Uint8Array::try_from(chunk) {
                                    let length = buffer.length() as usize;
                                    let prev_len = self.buffer.len();
                                    self.buffer.resize(prev_len + length, 0);
                                    buffer.copy_to(&mut self.buffer[prev_len..]);
                                }
                            }
                            false
                        },
                        _ = delay => {
                            true
                        }
                    }
                };
                if timed_out {
                    //self.stream.get_reader().cancel().await?;
                    return Ok(0);
                }
            }
        }

        let bytes_remaining = self.buffer.len() - self.position;
        if bytes_remaining == 0 {
            return Ok(0);
        } else if bytes_remaining < data.len() {
            data[..bytes_remaining].copy_from_slice(&self.buffer[self.position..]);
            self.position = 0;
            self.buffer.clear();
            Ok(bytes_remaining)
        } else {
            data.copy_from_slice(&self.buffer[self.position..self.position + data.len()]);
            self.position += data.len();
            Ok(data.len())
        }
    }
}

struct WritableStreamWrapper {
    stream: WritableStream,
}

impl WritableStreamWrapper {
    fn new(stream: WritableStream) -> Self {
        Self { stream }
    }
}

impl StreamWriterAsync for WritableStreamWrapper {
    type Error = JsValue;
    async fn write(&mut self, data: &[u8]) -> Result<usize, Self::Error> {
        let buffer = js_sys::Uint8Array::from(data);
        let writer = self.stream.get_writer();
        pin_mut!(writer);
        writer.write(buffer.into()).await?;
        Ok(data.len())
    }
}

#[wasm_bindgen]
pub struct JsProtocolMasterConfig {
    pub echo_back: bool,
}
#[wasm_bindgen]
impl JsProtocolMasterConfig {
    #[wasm_bindgen(constructor)]
    pub fn new(echo_back: bool) -> Self {
        Self { echo_back }
    }
}
impl Into<ProtocolMasterConfig> for JsProtocolMasterConfig {
    fn into(self) -> ProtocolMasterConfig {
        ProtocolMasterConfig {
            echo_back: self.echo_back,
        }
    }
}

#[wasm_bindgen]
pub async fn scan_servo(port: SerialPort, config: JsProtocolMasterConfig, cb: &js_sys::Function) -> Result<JsValue, JsValue> {
    let mut reader = ReadableStreamWrapper::new(ReadableStream::from_raw(port.readable()));
    let mut writer = WritableStreamWrapper::new(WritableStream::from_raw(port.writable()));
    
    let config: ProtocolMasterConfig = config.into();
    log::info!("echo_back: {}", config.echo_back);
    let mut master = ProtocolMaster::<300>::new(config);
    let mut found_ids = js_sys::Array::new();
    for id in 1..254 {
        cb.call1(&JsValue::null(), &JsValue::from_f64(id as f64)).ok();

        log::info!("Scanning {}", id);
        let start = Instant::now();
        let mut timeout_counter = 0;
        let mut buffer = [0; 3];
        match master.read_register_async(&mut reader, &mut writer, id, 0x03, &mut buffer, || { start.elapsed().as_millis() > 10 }).await {
            Ok(_) => {
                found_ids.push(&JsValue::from_f64(id as f64));
                log::info!("Found servo with ID {} version {:02X} {:02X}", id, buffer[0], buffer[1]);
            }
            Err(err) => {
                log::debug!("Err with ID {} {:?}", id, err);
            }
        }
    }
    Ok(found_ids.into())
}

#[wasm_bindgen]
pub async fn change_servo_id(port: SerialPort, config: JsProtocolMasterConfig, old_id: u8, new_id: u8) -> Result<JsValue, JsValue> {
    let mut reader = ReadableStreamWrapper::new(ReadableStream::from_raw(port.readable()));
    let mut writer = WritableStreamWrapper::new(WritableStream::from_raw(port.writable()));
    
    let mut master = ProtocolMaster::<300>::new(config.into());

    // Unlock the EEPROM by writing 0 to register 0x30
    let start = Instant::now();
    let mut command = WriteRegisterCommand::<10>::new(old_id, 0x30, 1);
    command.writer().data_mut().unwrap()[2] = 0;
    command.writer().update_checksum().unwrap();
    master.write_register_async(&mut reader, &mut writer, &command, || start.elapsed().as_millis() > 100).await
        .map_err(|err| JsValue::from_str(&format!("Failed to unlocking the EEPROM - {:?}", err)))?;

    // Write New ID to register 0x05
    let start = Instant::now();
    let mut command = WriteRegisterCommand::<10>::new(old_id, 0x05, 1);
    command.writer().data_mut().unwrap()[2] = new_id;
    command.writer().update_checksum().unwrap();
    master.write_register_async(&mut reader, &mut writer, &command, || start.elapsed().as_millis() > 100).await
        .map_err(|err| JsValue::from_str(&format!("Failed to updating ID register - {:?}", err)))?;

    // Lock the EEPROM by writing 1 to register 0x30
    let start = Instant::now();
    let mut command = WriteRegisterCommand::<10>::new(new_id, 0x30, 1);
    command.writer().data_mut().unwrap()[2] = 1;
    command.writer().update_checksum().unwrap();
    master.write_register_async(&mut reader, &mut writer, &command, || start.elapsed().as_millis() > 100).await
        .map_err(|err| JsValue::from_str(&format!("Failed to locking the EEPROM - {:?}", err)))?;

    Ok(JsValue::undefined())
}