use std::collections::BTreeMap;

use serde::{Serialize, ser::SerializeMap};

const RFID_AVL_ID: u16 = 78;

#[derive(Debug, Clone, Serialize)]
pub struct TeltonikaFrame {
    pub imei: String,
    pub codec_id: u8,
    pub record_count: u8,
    pub records: Vec<AvlData>,
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
    #[serde(serialize_with = "serialize_eight_bytes")]
    pub eight_bytes: BTreeMap<u16, u64>,
    pub x_bytes: BTreeMap<u16, Vec<u8>>,
}

fn serialize_eight_bytes<S>(values: &BTreeMap<u16, u64>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let mut map = serializer.serialize_map(Some(values.len()))?;
    for (id, value) in values {
        if *id == RFID_AVL_ID {
            map.serialize_entry(id, &value.to_string())?;
        } else {
            map.serialize_entry(id, value)?;
        }
    }
    map.end()
}

#[cfg(test)]
mod tests {
    use super::{AvlData, GpsElementBlock, IoElementBlock, TeltonikaFrame};

    #[test]
    fn serializes_avl_id_78_as_a_decimal_string_only() {
        let frame = TeltonikaFrame {
            imei: "866069063120602".to_string(),
            codec_id: 0x8e,
            record_count: 1,
            records: vec![AvlData {
                timestamp: 1_750_000_000_001,
                priority: 0,
                gps_element: GpsElementBlock {
                    longitude: 0,
                    latitude: 0,
                    altitude: 0,
                    angle: 0,
                    satellites: 0,
                    speed: 0,
                },
                io_element: IoElementBlock {
                    event_io_id: 78,
                    n_total: 2,
                    eight_bytes: [(78, 111_388_404_872_642_594), (79, u64::MAX)].into(),
                    ..Default::default()
                },
            }],
        };

        let payload = serde_json::to_vec(&frame).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        let values = &json["records"][0]["io_element"]["eight_bytes"];

        assert_eq!(values["78"].as_str(), Some("111388404872642594"));
        assert_eq!(values["79"].as_u64(), Some(u64::MAX));
    }
}
