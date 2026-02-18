use std::io;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::units::teltonika::types::TeltonikaData;

pub async fn read_imei(stream: &mut TcpStream) -> io::Result<String> {
    let mut len_buf = [0u8; 2];
    stream.read_exact(&mut len_buf).await?;
    let imei_len = u16::from_be_bytes(len_buf) as usize;

    if imei_len == 0 || imei_len > 32 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid IMEI length: {imei_len}"),
        ));
    }

    let mut imei_bytes = vec![0u8; imei_len];
    stream.read_exact(&mut imei_bytes).await?;

    let imei = String::from_utf8(imei_bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "IMEI is not valid UTF-8/ASCII"))?;

    if imei.len() != imei_len || !imei.chars().all(|c| c.is_ascii_digit()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid IMEI format: {imei:?}"),
        ));
    }

    Ok(imei)
}

pub async fn write_imei_handshake(stream: &mut TcpStream, accepted: bool) -> io::Result<()> {
    let b = if accepted { 0x01u8 } else { 0x00u8 };
    stream.write_all(&[b]).await?;
    stream.flush().await?;
    Ok(())
}

pub fn teltonika_print(data: TeltonikaData) {
    println!("\n======= Teltonika =======");
    println!("field length: {:?}", data.data_field_length);
    println!("codec id: 0x{:02X}", data.codec_id);
    println!("num of data 1: {:?}", data.number_of_data_1);
    println!("num of data 2: {:?}", data.number_of_data_2);
    println!("crc 16: {:?}", data.crc_16);

    println!("\n======= AVL =======");
    println!("priority: {:?}", data.avl_data.priority);
    println!("time: {:?}", data.avl_data.timestamp);

    println!("\n======= GPS =======");
    println!("altitude: {:?}", data.avl_data.gps_element.altitude);
    println!("latitude: {:?}", data.avl_data.gps_element.latitude);
    println!("longitude: {:?}", data.avl_data.gps_element.longitude);
    println!("satellites: {:?}", data.avl_data.gps_element.satellites);
    println!("speed: {:?}", data.avl_data.gps_element.speed);
    println!("angle: {:?}", data.avl_data.gps_element.angle);

    println!("\n======= IO =======");
    println!("eight_bytes: {:?}", data.avl_data.io_element.eight_bytes);
    println!("event_io_id: {:?}", data.avl_data.io_element.event_io_id);
    println!("four_bytes: {:?}", data.avl_data.io_element.four_bytes);
    println!("n_total: {:?}", data.avl_data.io_element.n_total);
    println!("one_byte: {:?}", data.avl_data.io_element.one_byte);
    println!("two_bytes: {:?}", data.avl_data.io_element.two_bytes);
}
