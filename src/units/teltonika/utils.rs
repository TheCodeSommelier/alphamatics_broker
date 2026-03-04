use std::i64;
use std::io::{Error, ErrorKind, Result};

use chrono::DateTime;
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
    println!("num of data 1: {:?}", data.record_count);
    println!("codec_id: {:?}", data.codec_id);

    println!("\n======= Records =======");
    let records = &data.records;

    for avl_data in records.iter().enumerate() {
        println!("\n======= AVL generic {:?} =======", avl_data.0);
        println!("Timestamp: {:?}", avl_data.1.timestamp);
        println!("Time and date: {:?}", DateTime::from_timestamp_millis(avl_data.1.timestamp as i64));
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
