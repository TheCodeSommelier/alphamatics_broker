use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TeltonikaFrame {
    pub imei: String,
    pub data_field_length: u32,
    pub codec_id: u8,
    pub record_count: u8,
    pub records: Vec<AvlData>,
    /// Must equal record_count
    pub record_count_2: u8,
    pub crc_16: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct AvlData {
    pub timestamp: u64, // epoch ms
    pub priority: u8,
    pub gps_element: GpsElementBlock,
    pub io_element: IoElementBlock,
}

#[derive(Debug, Clone, Serialize)]
pub struct GpsElementBlock {
    pub longitude: i32,
    pub latitude: i32,
    pub altitude: i16,
    pub angle: u16,
    pub satellites: u8,
    pub speed: u16,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct IoElementBlock {
    pub event_io_id: u16, // u16 for 8E (safe even if you don't use it yet)
    pub n_total: u16,
    pub one_byte: BTreeMap<u16, u8>,
    pub two_bytes: BTreeMap<u16, u16>,
    pub four_bytes: BTreeMap<u16, u32>,
    pub eight_bytes: BTreeMap<u16, u64>,
    pub x_bytes: BTreeMap<u16, Vec<u8>>,
}
