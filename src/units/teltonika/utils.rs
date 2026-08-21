use std::io::{Error, ErrorKind, Result};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

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

pub async fn teltonika_write_frame_ack(stream: &mut TcpStream, record_count: u8) -> Result<()> {
    let ack = (record_count as u32).to_be_bytes();
    stream.write_all(&ack).await?;
    stream.flush().await?;
    Ok(())
}
