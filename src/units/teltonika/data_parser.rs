use std::{collections::BTreeMap, io, u8};

use crate::units::{
    teltonika::types::{AvlData, GpsElementBlock, IoElementBlock, TeltonikaFrame},
    utils::Cur,
};

pub fn teltonika_parse_frame(frame: &[u8], imei: &String) -> io::Result<TeltonikaFrame> {
    // preamble(4) + data_len(4) + codec(1) + n1(1) + ... + n2(1) + crc(4)
    if frame.len() < 4 + 4 + 1 + 1 + 1 + 4 {
        let err = io::Error::new(io::ErrorKind::UnexpectedEof, "frame too short");
        return Err(err);
    }

    let data_len = u32::from_be_bytes(frame[4..8].try_into().unwrap()) as usize;

    let data_start = 8;
    let data_end = data_start + data_len;
    if frame.len() < data_end + 4 {
        let err = io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "frame shorter than data_len+crc",
        );
        return Err(err);
    }

    let codec_id = frame[data_start];
    let record_count = frame[data_start + 1];
    let record_count_2 = frame[data_end - 1];

    // AVL payload sits between record_count and record_count_2
    let avl_bytes = &frame[data_start + 2..data_end - 1];

    // Codec 8 Extended should be 0x8E
    if codec_id != 0x8E {
        let err = io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!("unexpected codec: {codec_id:#x}"),
        );
        return Err(err);
    }

    if record_count != record_count_2 {
        let err = io::Error::new(io::ErrorKind::InvalidData, "record_count != record_count_2");
        return Err(err);
    }

    let mut cur = Cur::new(avl_bytes);
    let mut records = Vec::with_capacity(record_count as usize);

    for _ in 0..record_count {
        records.push(parse_avl_record_8e(&mut cur)?);
    }

    if cur.remaining() != 0 {
        let err = io::Error::new(
            io::ErrorKind::InvalidData,
            format!("extra bytes after parsing records: {}", cur.remaining()),
        );
        return Err(err);
    }

    Ok(TeltonikaFrame {
        imei: imei.to_string(),
        codec_id,
        record_count,
        records,
    })
}

// ========================
//         Private
// ========================

fn parse_avl_record_8e(cur: &mut Cur<'_>) -> io::Result<AvlData> {
    // Timestamp(8) Priority(1) GPS(15) IO(variable)
    let timestamp = cur.u64()?;
    let priority = cur.u8()?;
    let gps_element = parse_gps(cur)?;
    let io_element = parse_io_8e(cur)?;

    Ok(AvlData {
        timestamp,
        priority,
        gps_element,
        io_element,
    })
}

fn parse_gps(cur: &mut Cur<'_>) -> io::Result<GpsElementBlock> {
    let longitude = cur.i32()?;
    let latitude = cur.i32()?;
    let altitude = cur.i16()?;
    let angle = cur.u16()?;
    let satellites = cur.u8()?;
    let speed = cur.u16()?;

    Ok(GpsElementBlock {
        longitude,
        latitude,
        altitude,
        angle,
        satellites,
        speed,
    })
}

/// Codec 8 Extended IO element parsing:
/// Event IO ID (2)
/// N of Total (2)
/// N1 (2) => [id(2), val(1)] * N1
/// N2 (2) => [id(2), val(2)] * N2
/// N4 (2) => [id(2), val(4)] * N4
/// N8 (2) => [id(2), val(8)] * N8
/// NX (2) => [id(2), len(2), val(len)] * NX
fn parse_io_8e(cur: &mut Cur<'_>) -> io::Result<IoElementBlock> {
    let event_io_id = cur.u16()?;
    let n_total = cur.u16()?;

    let n1 = cur.u16()? as usize;
    let mut one_byte = BTreeMap::new();
    for _ in 0..n1 {
        let id = cur.u16()?;
        let val = cur.u8()?;
        one_byte.insert(id, val);
    }

    let n2 = cur.u16()? as usize;
    let mut two_bytes = BTreeMap::new();
    for _ in 0..n2 {
        let id = cur.u16()?;
        let val = cur.u16()?;
        two_bytes.insert(id, val);
    }

    let n4 = cur.u16()? as usize;
    let mut four_bytes = BTreeMap::new();
    for _ in 0..n4 {
        let id = cur.u16()?;
        let val = cur.u32()?;
        four_bytes.insert(id, val);
    }

    let n8 = cur.u16()? as usize;
    let mut eight_bytes = BTreeMap::new();
    for _ in 0..n8 {
        let id = cur.u16()?;
        let val = cur.u64()?;
        eight_bytes.insert(id, val);
    }

    let nx = cur.u16()? as usize;
    let mut x_bytes = BTreeMap::new();
    for _ in 0..nx {
        let id = cur.u16()?;
        let len = cur.u16()? as usize;
        let val = cur.take(len)?.to_vec();
        x_bytes.insert(id, val);
    }

    let computed_total = (n1 + n2 + n4 + n8 + nx) as u16;
    if n_total != computed_total {
        let err = io::Error::new(
            io::ErrorKind::InvalidData,
            format!("n_total mismatch: got {n_total}, computed {computed_total}"),
        );
        return Err(err);
    }

    Ok(IoElementBlock {
        event_io_id,
        n_total,
        one_byte,
        two_bytes,
        four_bytes,
        eight_bytes,
        x_bytes,
    })
}
