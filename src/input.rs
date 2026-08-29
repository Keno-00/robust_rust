use std::io::{self, Write};
use std::time::Duration;

pub const ARDUINO_BAUD_RATE: u32 = 115_200;
pub const DEFAULT_SERIAL_TIMEOUT: Duration = Duration::from_millis(100);
pub const SHOOT_COMMAND: &[u8] = b"shoot";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputBackend {
    Arduino,
}

pub fn encode_move(dx: i32, dy: i32) -> String {
    format!("{dx}:{dy}")
}

pub struct ArduinoTransport<W> {
    writer: W,
}

impl<W: Write> ArduinoTransport<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn move_mouse(&mut self, dx: i32, dy: i32) -> io::Result<()> {
        self.writer.write_all(encode_move(dx, dy).as_bytes())
    }

    pub fn shoot(&mut self) -> io::Result<()> {
        self.writer.write_all(SHOOT_COMMAND)
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

pub type ArduinoSerial = ArduinoTransport<Box<dyn serialport::SerialPort>>;

impl ArduinoSerial {
    pub fn open(port_name: &str) -> serialport::Result<Self> {
        let port = serialport::new(port_name, ARDUINO_BAUD_RATE)
            .timeout(DEFAULT_SERIAL_TIMEOUT)
            .open()?;
        Ok(Self::new(port))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_python_arduino_move_protocol() {
        assert_eq!(encode_move(12, -7), "12:-7");
        assert_eq!(encode_move(0, 0), "0:0");
    }

    #[test]
    fn writes_move_bytes_without_separator() {
        let mut transport = ArduinoTransport::new(Vec::<u8>::new());
        transport.move_mouse(14, -27).unwrap();

        assert_eq!(transport.into_inner(), b"14:-27");
    }

    #[test]
    fn writes_shoot_command() {
        let mut transport = ArduinoTransport::new(Vec::<u8>::new());
        transport.shoot().unwrap();

        assert_eq!(transport.into_inner(), SHOOT_COMMAND);
    }
}
