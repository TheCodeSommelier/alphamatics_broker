use std::io;

use async_nats::Client;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

use crate::{
    redis::{RedisClient, redis_connect},
    units::teltonika::types::TeltonikaFrame,
};

const RFID_AVL_ID: u16 = 78;

#[derive(Clone)]
pub struct RfidEnrollmentPublisher {
    redis: RedisClient,
    nats: Client,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActiveEnrollment {
    started_at_ms: u64,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RfidScanMessage<'a> {
    imei: &'a str,
    event_io_id: u16,
    rfid_chip_uid: String,
    timestamp_ms: u64,
}

impl RfidEnrollmentPublisher {
    pub fn connect(nats: Client) -> io::Result<Self> {
        Ok(Self {
            redis: redis_connect()?,
            nats,
        })
    }

    pub async fn publish_scan(&self, frame: &TeltonikaFrame) -> io::Result<bool> {
        if !frame.records.iter().any(is_rfid_record) {
            return Ok(false);
        }

        let Some(enrollment) = self.active_enrollment(&frame.imei).await? else {
            return Ok(false);
        };

        let Some(message) = extract_scan(frame, enrollment.started_at_ms) else {
            return Ok(false);
        };
        let payload = serde_json::to_vec(&message).map_err(io::Error::other)?;

        self.nats
            .publish(rfid_subject(&frame.imei), payload.into())
            .await
            .map_err(io::Error::other)?;

        Ok(true)
    }

    async fn active_enrollment(&self, imei: &str) -> io::Result<Option<ActiveEnrollment>> {
        let mut connection = self
            .redis
            .get_multiplexed_async_connection()
            .await
            .map_err(io::Error::other)?;
        let value: Option<String> = connection
            .get(active_enrollment_key(imei))
            .await
            .map_err(io::Error::other)?;

        value
            .map(|value| serde_json::from_str(&value).map_err(io::Error::other))
            .transpose()
    }
}

fn is_rfid_record(record: &crate::units::teltonika::types::AvlData) -> bool {
    record.io_element.event_io_id == RFID_AVL_ID
        && record.io_element.eight_bytes.contains_key(&RFID_AVL_ID)
}

fn extract_scan(frame: &TeltonikaFrame, started_at_ms: u64) -> Option<RfidScanMessage<'_>> {
    frame
        .records
        .iter()
        .filter(|record| is_rfid_record(record))
        .filter(|record| record.timestamp >= started_at_ms)
        .min_by_key(|record| record.timestamp)
        .map(|record| RfidScanMessage {
            imei: &frame.imei,
            event_io_id: record.io_element.event_io_id,
            // Keep all 64 bits intact across JSON/JavaScript. Decimal also
            // matches the lossless representation used by AVL ingestion.
            rfid_chip_uid: record.io_element.eight_bytes[&RFID_AVL_ID].to_string(),
            timestamp_ms: record.timestamp,
        })
}

fn active_enrollment_key(imei: &str) -> String {
    format!("rfid-enrollment:active:{imei}")
}

fn rfid_subject(imei: &str) -> String {
    format!("units.rfid.{imei}")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::units::teltonika::types::{
        AvlData, GpsElementBlock, IoElementBlock, TeltonikaFrame,
    };

    use super::{RFID_AVL_ID, extract_scan};

    fn record(timestamp: u64, event_io_id: u16, uid: Option<u64>) -> AvlData {
        let mut eight_bytes = BTreeMap::new();
        if let Some(uid) = uid {
            eight_bytes.insert(RFID_AVL_ID, uid);
        }

        AvlData {
            timestamp,
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
                event_io_id,
                n_total: eight_bytes.len() as u16,
                eight_bytes,
                ..Default::default()
            },
        }
    }

    #[test]
    fn extracts_the_earliest_rfid_scan_after_enrollment_started() {
        let frame = TeltonikaFrame {
            imei: "866069063120602".to_string(),
            codec_id: 0x8e,
            record_count: 3,
            records: vec![
                record(1_749_999_999_999, RFID_AVL_ID, Some(1)),
                record(1_750_000_000_002, RFID_AVL_ID, Some(u64::MAX)),
                record(1_750_000_000_001, RFID_AVL_ID, Some(42)),
            ],
        };

        let message = extract_scan(&frame, 1_750_000_000_000).unwrap();

        assert_eq!(message.imei, frame.imei);
        assert_eq!(message.event_io_id, RFID_AVL_ID);
        assert_eq!(message.rfid_chip_uid, "42");
        assert_eq!(message.timestamp_ms, 1_750_000_000_001);
    }

    #[test]
    fn requires_event_78_and_its_eight_byte_value() {
        let frame = TeltonikaFrame {
            imei: "866069063120602".to_string(),
            codec_id: 0x8e,
            record_count: 2,
            records: vec![
                record(1_750_000_000_001, 1, Some(42)),
                record(1_750_000_000_002, RFID_AVL_ID, None),
            ],
        };

        assert_eq!(extract_scan(&frame, 1_750_000_000_000), None);
    }

    #[test]
    fn serializes_the_uid_without_losing_u64_precision() {
        let frame = TeltonikaFrame {
            imei: "866069063120602".to_string(),
            codec_id: 0x8e,
            record_count: 1,
            records: vec![record(1_750_000_000_001, RFID_AVL_ID, Some(u64::MAX))],
        };
        let message = extract_scan(&frame, 1_750_000_000_000).unwrap();

        assert_eq!(
            serde_json::to_string(&message).unwrap(),
            r#"{"imei":"866069063120602","eventIoId":78,"rfidChipUid":"18446744073709551615","timestampMs":1750000000001}"#
        );
    }
}
