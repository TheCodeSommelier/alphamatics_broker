use std::io;

use crate::units::utils::Cur;

const CODEC12_ID: u8 = 0x0C;
const COMMAND_TYPE: u8 = 0x05;
const RESPONSE_TYPE: u8 = 0x06;

pub fn build_command_frame(command: &str) -> Vec<u8> {
    let command_bytes = command.as_bytes();
    let data_size = 1 + 1 + 1 + 4 + command_bytes.len() + 1;

    let mut frame = Vec::with_capacity(8 + data_size + 4);
    frame.extend_from_slice(&[0, 0, 0, 0]); // Preamble 4 bytes
    frame.extend_from_slice(&(data_size as u32).to_be_bytes()); // Data size from codec ID to second Quantity 2
    frame.push(CODEC12_ID); // Codec 12 id see line 5
    frame.push(0x01); // Command size rn we allow one command
    frame.push(COMMAND_TYPE);
    frame.extend_from_slice(&(command_bytes.len() as u32).to_be_bytes()); // Command size in bytes
    frame.extend_from_slice(command_bytes);
    frame.push(0x01); // Command size rn we allow one command

    let crc = crc16_ibm(&frame[8..]);
    frame.extend_from_slice(&(crc as u32).to_be_bytes());

    println!("Sending bytes: {}", hex::encode(&frame));

    frame
}

pub fn parse_response_frame(frame: &[u8]) -> io::Result<String> {
    if frame.len() < 4 + 4 + 1 + 1 + 1 + 4 + 1 + 4 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "codec12 frame too short",
        ));
    }

    if frame[0..4] != [0, 0, 0, 0] {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "codec12 frame missing preamble",
        ));
    }

    let data_len = u32::from_be_bytes(frame[4..8].try_into().unwrap()) as usize;
    if frame.len() != 8 + data_len + 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "codec12 frame length mismatch",
        ));
    }

    let expected_crc = u32::from_be_bytes(frame[8 + data_len..8 + data_len + 4].try_into().unwrap());
    let actual_crc = crc16_ibm(&frame[8..8 + data_len]) as u32;
    if expected_crc != actual_crc {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("codec12 crc mismatch: expected {expected_crc:#x}, got {actual_crc:#x}"),
        ));
    }

    let mut cur = Cur::new(&frame[8..8 + data_len]);
    let codec_id = cur.u8()?;
    if codec_id != CODEC12_ID {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unexpected codec12 codec id: {codec_id:#x}"),
        ));
    }

    let quantity_1 = cur.u8()?;
    let message_type = cur.u8()?;
    if message_type != RESPONSE_TYPE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unexpected codec12 message type: {message_type:#x}"),
        ));
    }

    let response_size = cur.u32()? as usize;
    let response = cur.take(response_size)?;
    let quantity_2 = cur.u8()?;

    if quantity_1 != quantity_2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "codec12 quantity mismatch",
        ));
    }

    String::from_utf8(response.to_vec())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "codec12 response is not utf-8"))
}

fn crc16_ibm(data: &[u8]) -> u16 {
    let mut crc = 0u16;

    for &byte in data {
        crc ^= byte as u16;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }

    crc
}

#[cfg(test)]
mod tests {
    use super::{build_command_frame, parse_response_frame};

    #[test]
    fn builds_getinfo_codec12_command() {
        let frame = build_command_frame("getinfo");

        assert_eq!(hex::encode(frame), "000000000000000f0c010500000007676574696e666f0100004312");
    }

    #[test]
    fn parses_getio_codec12_response() {
        let frame = hex::decode("00000000000000370c01060000002f4449313a31204449323a30204449333a302041494e313a302041494e323a313639323420444f313a3020444f323a3101000066e3").unwrap();

        let response = parse_response_frame(&frame).unwrap();

        assert_eq!(response, "DI1:1 DI2:0 DI3:0 AIN1:0 AIN2:16924 DO1:0 DO2:1");
    }
}
