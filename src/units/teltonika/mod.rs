use std::io;

use async_nats::jetstream::Context;
use tokio::io::AsyncReadExt;

use crate::{
    db::UnitMake,
    nats::nats_publish,
    units::teltonika::{
        data_parser::teltonika_parse_frame,
        errors::TeltonikaFrameError,
        utils::{teltonika_print, teltonika_write_frame_ack, teltonika_write_imei_handshake},
    },
};

pub mod data_parser;
pub mod errors;
pub mod types;
pub mod utils;
pub mod validations;

pub async fn teltonika_listen(
    mut socket: tokio::net::TcpStream,
    accepted: bool,
    imei: String,
    jetstream: &Context,
    make: UnitMake,
) -> io::Result<()> {
    let peer_addr = socket.peer_addr().ok();
    teltonika_write_imei_handshake(&mut socket, accepted).await?;

    if !accepted {
        println!("Rejected IMEI {imei}, closing connection.");
        return Ok(());
    }

    let mut acc: Vec<u8> = Vec::with_capacity(8192);

    loop {
        let mut buf = [0u8; 4096];
        let n = socket.read(&mut buf).await?;

        if n == 0 {
            println!("Client disconnected: {:?}", peer_addr);
            return Ok(());
        }

        println!("Received {} bytes: {}", n, hex::encode(&buf[..n]));
        acc.extend_from_slice(&buf[..n]);

        loop {
            if acc.len() < 8 {
                break;
            }

            // Resync: Teltonika TCP preamble should be 0x00000000
            if acc[0..4] != [0, 0, 0, 0] {
                if let Some(pos) = acc.windows(4).position(|w| w == [0, 0, 0, 0]) {
                    acc.drain(..pos);
                } else {
                    acc.clear();
                }
                break;
            }

            let data_len = u32::from_be_bytes(acc[4..8].try_into().unwrap()) as usize;
            let frame_len = 8 + data_len + 4;

            if acc.len() < frame_len {
                // We don't have the full Teltonika frame yet
                break;
            }

            let frame: Vec<u8> = acc.drain(..frame_len).collect();

            let data = match teltonika_parse_frame(&frame, &imei) {
                Ok(data) => data,
                Err(err) => {
                    if let Some(ack_record_count) = err.ack_record_count() {
                        eprintln!("discarded frame: {err}");
                        teltonika_write_frame_ack(&mut socket, ack_record_count).await?;
                    } else {
                        match err {
                            TeltonikaFrameError::Parse(err) => {
                                sentry::capture_error(&err);
                                eprintln!("parse error: {err}");
                            }
                            TeltonikaFrameError::Discarded { .. } => unreachable!(),
                        }
                    }
                    continue;
                }
            };
            nats_publish(jetstream, &data, &imei, make).await?;
            teltonika_print(&data);

            teltonika_write_frame_ack(&mut socket, data.record_count).await?;
        }
    }
}
