use std::io;

use async_nats::jetstream::Context;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    time::{Duration, timeout},
};

use crate::{
    commands::{CommandQueue, CommandResponsePayload, QueuedCommand},
    db::UnitMake,
    nats::{nats_publish, nats_publish_command_response},
    rfid::RfidEnrollmentPublisher,
    units::teltonika::{
        codec12::{build_command_frame, parse_response_frame},
        data_parser::teltonika_parse_frame,
        errors::TeltonikaFrameError,
        utils::{crc16_ibm, teltonika_write_frame_ack, teltonika_write_imei_handshake},
    },
};

pub mod codec12;
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
    command_queue: CommandQueue,
    rfid_publisher: RfidEnrollmentPublisher,
) -> io::Result<()> {
    #[cfg(debug_assertions)]
    let peer_addr = socket.peer_addr().ok();

    teltonika_write_imei_handshake(&mut socket, accepted).await?;

    if !accepted {
        #[cfg(debug_assertions)]
        println!("Rejected IMEI {imei}, closing connection.");
        return Ok(());
    }

    let mut acc: Vec<u8> = Vec::with_capacity(8192);
    let mut ready_for_commands = false;

    loop {
        if ready_for_commands {
            while let Some(command) = command_queue.peek(&imei).await? {
                if let Err(_err) = execute_queued_command(
                    &mut socket,
                    &mut acc,
                    jetstream,
                    &imei,
                    make,
                    &command,
                    &rfid_publisher,
                )
                .await
                {
                    #[cfg(debug_assertions)]
                    let err = &_err;

                    #[cfg(debug_assertions)]
                    eprintln!(
                        "command execution failed for {imei}: {err}; command remains queued"
                    );
                    return Err(_err);
                }

                command_queue.remove_front(&imei).await?;
            }
        }

        let mut buf = [0u8; 4096];
        if ready_for_commands {
            tokio::select! {
                read = socket.read(&mut buf) => {
                    let n = read?;

                    if n == 0 {
                        #[cfg(debug_assertions)]
                        println!("Client disconnected: {:?}", peer_addr);
                        return Ok(());
                    }

                    acc.extend_from_slice(&buf[..n]);

                while let Some(frame) = try_extract_frame(&mut acc)? {
                    if handle_unsolicited_frame(
                        &mut socket,
                        jetstream,
                        &imei,
                        make,
                        &rfid_publisher,
                        frame,
                    )
                    .await?
                    {
                        ready_for_commands = true;
                    }
                }
            }
                wait = command_queue.wait_for_command(&imei) => {
                    wait?;
                }
            }
        } else {
            let n = socket.read(&mut buf).await?;

            if n == 0 {
                #[cfg(debug_assertions)]
                println!("Client disconnected: {:?}", peer_addr);
                return Ok(());
            }

            acc.extend_from_slice(&buf[..n]);

            while let Some(frame) = try_extract_frame(&mut acc)? {
                if handle_unsolicited_frame(
                    &mut socket,
                    jetstream,
                    &imei,
                    make,
                    &rfid_publisher,
                    frame,
                )
                .await?
                {
                    ready_for_commands = true;
                }
            }
        }
    }
}

async fn execute_queued_command(
    socket: &mut tokio::net::TcpStream,
    acc: &mut Vec<u8>,
    jetstream: &Context,
    imei: &str,
    make: UnitMake,
    command: &QueuedCommand,
    rfid_publisher: &RfidEnrollmentPublisher,
) -> io::Result<()> {
    let frame = build_command_frame(&command.command);
    socket.write_all(&frame).await?;
    socket.flush().await?;

    #[cfg(debug_assertions)]
    println!(
        "Sent Codec12 command {} to IMEI {}: {:?}",
        command.request_id, imei, command.command
    );

    let response_timeout = Duration::from_millis(command.timeout_ms.unwrap_or(30_000));
    let response = timeout(
        response_timeout,
        wait_for_command_response(socket, acc, jetstream, imei, make, rfid_publisher),
    )
    .await
    .map_err(|_| {
        io::Error::new(
            io::ErrorKind::TimedOut,
            format!("timed out waiting for response to {}", command.request_id),
        )
    })??;

    publish_command_result(jetstream, imei, command, response, true).await
}

async fn wait_for_command_response(
    socket: &mut tokio::net::TcpStream,
    acc: &mut Vec<u8>,
    jetstream: &Context,
    imei: &str,
    make: UnitMake,
    rfid_publisher: &RfidEnrollmentPublisher,
) -> io::Result<String> {
    loop {
        while let Some(frame) = try_extract_frame(acc)? {
            match classify_frame(&frame)? {
                IncomingFrame::Avl => {
                    handle_avl_frame(socket, jetstream, imei, make, rfid_publisher, frame).await?;
                }
                IncomingFrame::CommandResponse => {
                    return parse_response_frame(&frame);
                }
                IncomingFrame::Unsupported(_codec_id) => {
                    #[cfg(debug_assertions)]
                    eprintln!("ignoring unsupported Teltonika codec {_codec_id:#x} while waiting for command response");
                }
            }
        }

        let mut buf = [0u8; 4096];
        let n = socket.read(&mut buf).await?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "device disconnected while waiting for command response",
            ));
        }

        acc.extend_from_slice(&buf[..n]);
    }
}

async fn handle_unsolicited_frame(
    socket: &mut tokio::net::TcpStream,
    jetstream: &Context,
    imei: &str,
    make: UnitMake,
    rfid_publisher: &RfidEnrollmentPublisher,
    frame: Vec<u8>,
) -> io::Result<bool> {
    match classify_frame(&frame)? {
        IncomingFrame::Avl => {
            handle_avl_frame(socket, jetstream, imei, make, rfid_publisher, frame).await?;
            Ok(true)
        }
        IncomingFrame::CommandResponse => {
            let _response = parse_response_frame(&frame)?;

            #[cfg(debug_assertions)]
            eprintln!("unsolicited Codec12 response from {imei}: {_response}");
            Ok(false)
        }
        IncomingFrame::Unsupported(_codec_id) => {
            #[cfg(debug_assertions)]
            eprintln!("ignoring unsupported Teltonika codec {_codec_id:#x} from {imei}");
            Ok(false)
        }
    }
}

async fn handle_avl_frame(
    socket: &mut tokio::net::TcpStream,
    jetstream: &Context,
    imei: &str,
    make: UnitMake,
    rfid_publisher: &RfidEnrollmentPublisher,
    frame: Vec<u8>,
) -> io::Result<()> {
    let data = match teltonika_parse_frame(&frame, imei) {
        Ok(data) => data,
        Err(err) => {
            if let Some(ack_record_count) = err.ack_record_count() {
                #[cfg(debug_assertions)]
                eprintln!("discarded frame: {err}");
                teltonika_write_frame_ack(socket, ack_record_count).await?;
            } else {
                match err {
                    TeltonikaFrameError::Parse(_err) => {
                        #[cfg(debug_assertions)]
                        eprintln!("parse error: {_err}");
                    }
                    TeltonikaFrameError::Discarded { .. } => unreachable!(),
                }
            }
            return Ok(());
        }
    };

    nats_publish(jetstream, &data, imei, make).await?;

    if let Err(err) = rfid_publisher.publish_scan(&data).await {
        sentry::capture_error(&err);
        eprintln!("failed to publish RFID scan for {imei}: {err}");
    }

    #[cfg(debug_assertions)]
    println!(
        "Frame ingested for IMEI {imei} ({} records)",
        data.record_count
    );

    teltonika_write_frame_ack(socket, data.record_count).await
}

async fn publish_command_result(
    jetstream: &Context,
    imei: &str,
    command: &QueuedCommand,
    response: String,
    ok: bool,
) -> io::Result<()> {
    nats_publish_command_response(
        jetstream,
        &CommandResponsePayload {
            request_id: command.request_id.clone(),
            imei: imei.to_string(),
            command: command.command.clone(),
            response,
            ok,
        },
    )
    .await
}

/// Upper bound for a frame's data field. Real AVL packets are ~1-2 KB and
/// Codec12 responses are small; this only stops a bogus length header from
/// making us buffer gigabytes.
const MAX_FRAME_DATA_LEN: usize = 64 * 1024;

/// Pulls the next complete, CRC-valid frame out of `acc`. Frames failing the
/// CRC are dropped without an ack so the device retransmits them. A length
/// header over `MAX_FRAME_DATA_LEN` is an error that closes the connection.
fn try_extract_frame(acc: &mut Vec<u8>) -> io::Result<Option<Vec<u8>>> {
    loop {
        if acc.len() < 8 {
            return Ok(None);
        }

        if acc[0..4] != [0, 0, 0, 0] {
            if let Some(pos) = acc.windows(4).position(|w| w == [0, 0, 0, 0]) {
                acc.drain(..pos);
            } else {
                acc.clear();
                return Ok(None);
            }
        }

        if acc.len() < 8 {
            return Ok(None);
        }

        let data_len = u32::from_be_bytes(acc[4..8].try_into().unwrap()) as usize;
        if data_len > MAX_FRAME_DATA_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("frame data length {data_len} exceeds {MAX_FRAME_DATA_LEN}"),
            ));
        }

        let frame_len = 8 + data_len + 4;
        if acc.len() < frame_len {
            return Ok(None);
        }

        let frame: Vec<u8> = acc.drain(..frame_len).collect();
        let expected_crc = u32::from_be_bytes(frame[8 + data_len..].try_into().unwrap());
        let actual_crc = crc16_ibm(&frame[8..8 + data_len]) as u32;

        if expected_crc == actual_crc {
            return Ok(Some(frame));
        }

        eprintln!("dropping frame with crc mismatch: expected {expected_crc:#x}, got {actual_crc:#x}");
    }
}

fn classify_frame(frame: &[u8]) -> io::Result<IncomingFrame> {
    if frame.len() < 9 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "frame too short to classify",
        ));
    }

    Ok(match frame[8] {
        0x8E => IncomingFrame::Avl,
        0x0C => IncomingFrame::CommandResponse,
        codec_id => IncomingFrame::Unsupported(codec_id),
    })
}

enum IncomingFrame {
    Avl,
    CommandResponse,
    Unsupported(u8),
}

#[cfg(test)]
mod tests {
    use super::{MAX_FRAME_DATA_LEN, try_extract_frame};

    // Codec12 getio response with a valid CRC.
    const VALID_FRAME: &str = "00000000000000370c01060000002f4449313a31204449323a30204449333a302041494e313a302041494e323a313639323420444f313a3020444f323a3101000066e3";

    fn valid_frame() -> Vec<u8> {
        hex::decode(VALID_FRAME).unwrap()
    }

    #[test]
    fn extracts_a_valid_frame_and_leaves_the_rest() {
        let mut acc = valid_frame();
        acc.extend_from_slice(&[0, 0]);

        assert_eq!(try_extract_frame(&mut acc).unwrap(), Some(valid_frame()));
        assert_eq!(acc, vec![0, 0]);
    }

    #[test]
    fn waits_for_a_partial_frame() {
        let frame = valid_frame();
        let mut acc = frame[..frame.len() - 1].to_vec();

        assert_eq!(try_extract_frame(&mut acc).unwrap(), None);
        assert_eq!(acc.len(), frame.len() - 1);
    }

    #[test]
    fn drops_a_frame_with_a_bad_crc_and_returns_the_next_one() {
        let mut corrupted = valid_frame();
        let last = corrupted.len() - 1;
        corrupted[last] ^= 0xFF;

        let mut acc = corrupted;
        acc.extend_from_slice(&valid_frame());

        assert_eq!(try_extract_frame(&mut acc).unwrap(), Some(valid_frame()));
        assert!(acc.is_empty());
    }

    #[test]
    fn rejects_an_oversized_length_header() {
        let mut acc = vec![0, 0, 0, 0];
        acc.extend_from_slice(&((MAX_FRAME_DATA_LEN + 1) as u32).to_be_bytes());

        let err = try_extract_frame(&mut acc).unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }
}
