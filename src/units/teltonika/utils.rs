use std::io::{Error, ErrorKind, Result};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::units::teltonika::types::TeltonikaFrame;

pub async fn teltonika_read_imei(stream: &mut TcpStream) -> Result<String> {
    let mut len_buf = [0u8; 2];
    stream.read_exact(&mut len_buf).await?;
    let imei_len = u16::from_be_bytes(len_buf) as usize;

    if imei_len == 0 || imei_len > 32 {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("Invalid IMEI length: {imei_len}"),
        ));
    }

    let mut imei_bytes = vec![0u8; imei_len];
    stream.read_exact(&mut imei_bytes).await?;

    let imei = String::from_utf8(imei_bytes)
        .map_err(|_| Error::new(ErrorKind::InvalidData, "IMEI is not valid UTF-8/ASCII"))?;

    if imei.len() != imei_len || !imei.chars().all(|c| c.is_ascii_digit()) {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("Invalid IMEI format: {imei:?}"),
        ));
    }

    Ok(imei)
}

pub async fn teltonika_write_imei_handshake(stream: &mut TcpStream, accepted: bool) -> Result<()> {
    let b = if accepted { 0x01u8 } else { 0x00u8 };
    stream.write_all(&[b]).await?;
    stream.flush().await?;
    Ok(())
}

pub fn teltonika_print(data: &TeltonikaFrame) {
    println!("============ Start ============");

    println!("\n======= Teltonika =======");
    println!("field length: {:?}", data.data_field_length);
    println!("codec id: 0x{:02X}", data.codec_id);
    println!("num of data 1: {:?}", data.record_count);
    println!("num of data 2: {:?}", data.record_count_2);
    println!("crc 16: {:?}", data.crc_16);

    println!("\n======= Records =======");
    let records = &data.records;

    for avl_data in records.iter().enumerate() {
        println!("\n======= AVL generic {:?} =======", avl_data.0);
        println!("Timestamp: {:?}", avl_data.1.timestamp);
        println!("Priority: {:?}", avl_data.1.priority);

        println!("\n======= GPS {:?} =======", avl_data.0);
        println!("altitude: {:?}", avl_data.1.gps_element.altitude);
        println!("latitude: {:?}", avl_data.1.gps_element.latitude);
        println!("longitude: {:?}", avl_data.1.gps_element.longitude);
        println!("satellites: {:?}", avl_data.1.gps_element.satellites);
        println!("angle: {:?}", avl_data.1.gps_element.angle);
        println!("speed: {:?}", avl_data.1.gps_element.speed);

        println!("\n======= IO {:?} =======", avl_data.0);
        println!("eight_bytes: {:?}", avl_data.1.io_element.eight_bytes);
        println!("event_io_id: {:?}", avl_data.1.io_element.event_io_id);
        println!("four_bytes: {:?}", avl_data.1.io_element.four_bytes);
        println!("n_total: {:?}", avl_data.1.io_element.n_total);
        println!("one_byte: {:?}", avl_data.1.io_element.one_byte);
        println!("two_bytes: {:?}", avl_data.1.io_element.two_bytes);
        println!("x_bytes: {:?}", avl_data.1.io_element.x_bytes);
    }
    println!("\n============ END ============");
}

// ===================
//       Cursor
// ===================

pub struct Cur<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Cur<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Self { b, i: 0 }
    }

    pub fn _pos(&self) -> usize {
        self.i
    }

    pub fn remaining(&self) -> usize {
        self.b.len().saturating_sub(self.i)
    }

    pub fn need(&self, n: usize) -> Result<()> {
        if self.remaining() < n {
            return Err(Error::new(ErrorKind::UnexpectedEof, "not enough bytes"));
        }
        Ok(())
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        self.need(n)?;
        let s = &self.b[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16> {
        let s = self.take(2)?;
        Ok(u16::from_be_bytes([s[0], s[1]]))
    }

    pub fn u32(&mut self) -> Result<u32> {
        let s = self.take(4)?;
        Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }

    pub fn u64(&mut self) -> Result<u64> {
        let s = self.take(8)?;
        Ok(u64::from_be_bytes([
            s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
        ]))
    }

    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
}
