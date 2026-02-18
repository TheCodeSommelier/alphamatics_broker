use std::{collections::BTreeMap, io, u8};

use crate::units::teltonika::types::{AvlData, GpsElementBlock, IoElementBlock, TeltonikaData};

pub fn teltonika_parse_data(data: &[u8]) -> io::Result<TeltonikaData> {
    // Basic minimum: preamble(4) + len(4) + codec(1) + n1(1) + AVL(n) + n2(1) + crc(4) = 15 + n
    if data.len() < 15 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "packet too short",
        ));
    }

    let data_length_u32 = u32::from_be_bytes(
        data[4..8]
            .try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad length slice"))?,
    );

    let data_length = usize::try_from(data_length_u32)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "data_length too large"))?;

    let codec_id = data[8];
    let number_of_data_1 = data[9];

    let number_of_data_2 = data[8 + data_length - 1];

    // CRC starts right after the data field
    let crc_16 = u32::from_be_bytes(
        data[8 + data_length..8 + data_length + 4]
            .try_into()
            .unwrap(),
    );

    // AVL data is between byte 10 and the byte before number_of_data_2
    let avl = &data[10..data.len() - 5];

    let avl_data = parse_avl_data(avl)?;

    Ok(TeltonikaData {
        data_field_length: data_length_u32,
        codec_id,
        number_of_data_1,
        avl_data,
        number_of_data_2,
        crc_16,
    })
}

// ========================
//         Private
// ========================

fn parse_avl_data(avl: &[u8]) -> io::Result<AvlData> {
    // timestamp(8) + priority(1) + gps(15) = 24 minimum
    if avl.len() < 24 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "AVL record too short",
        ));
    }

    let timestamp = u64::from_be_bytes(
        avl[0..8]
            .try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad timestamp slice"))?,
    );

    let priority = avl[8];

    let gps = &avl[9..24];
    let io = &avl[24..];

    Ok(AvlData {
        timestamp,
        priority,
        gps_element: parse_gps_element(gps)?,
        io_element: parse_io(io)?, // placeholder still returns Ok(...)
    })
}

fn parse_gps_element(gps: &[u8]) -> io::Result<GpsElementBlock> {
    if gps.len() != 15 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "GPS element must be 15 bytes",
        ));
    }

    let longitude = i32::from_be_bytes(
        gps[0..4]
            .try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad lon slice"))?,
    );

    let latitude = i32::from_be_bytes(
        gps[4..8]
            .try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad lat slice"))?,
    );

    let altitude = i16::from_be_bytes(
        gps[8..10]
            .try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad altitude slice"))?,
    );

    let angle = u16::from_be_bytes(
        gps[10..12]
            .try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad angle slice"))?,
    );

    let satellites = gps[12];

    // FIX: needs 2 bytes
    let speed = u16::from_be_bytes(
        gps[13..15]
            .try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad speed slice"))?,
    );

    Ok(GpsElementBlock {
        longitude,
        latitude,
        altitude,
        angle,
        satellites,
        speed,
    })
}

fn parse_io(_io: &[u8]) -> io::Result<IoElementBlock> {
    Ok(IoElementBlock {
        event_io_id: 0,
        n_total: 0,
        one_byte: BTreeMap::new(),
        two_bytes: BTreeMap::new(),
        four_bytes: BTreeMap::new(),
        eight_bytes: BTreeMap::new(),
    })
}
