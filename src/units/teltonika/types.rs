use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct TeltonikaData {
    pub data_field_length: u32,
    pub codec_id: u8,
    pub number_of_data_1: u8,
    pub avl_data: AvlData,
    pub number_of_data_2: u8,
    pub crc_16: u32,
}

#[derive(Debug, Clone)]
pub struct AvlData {
    pub timestamp: u64,
    pub priority: u8,
    pub gps_element: GpsElementBlock,
    pub io_element: IoElementBlock,
}

#[derive(Debug, Clone)]
pub struct GpsElementBlock {
    pub longitude: i32, // NOTE: longitude/latitude are signed in Teltonika
    pub latitude: i32,
    pub altitude: i16, // often signed too (depends), but i16 is safe
    pub angle: u16,
    pub satellites: u8,
    pub speed: u16,
}

#[derive(Debug, Clone, Default)]
pub struct IoElementBlock {
    pub event_io_id: u8,
    pub n_total: u8,
    pub one_byte: BTreeMap<u8, u8>,
    pub two_bytes: BTreeMap<u8, u16>,
    pub four_bytes: BTreeMap<u8, u32>,
    pub eight_bytes: BTreeMap<u8, u64>,
}
