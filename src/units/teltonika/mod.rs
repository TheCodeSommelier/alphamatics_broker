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
    units::teltonika::{
        codec12::{build_command_frame, parse_response_frame},
        data_parser::teltonika_parse_frame,
        errors::TeltonikaFrameError,
        utils::{teltonika_print, teltonika_write_frame_ack, teltonika_write_imei_handshake},
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
) -> io::Result<()> {
    let peer_addr = socket.peer_addr().ok();
    teltonika_write_imei_handshake(&mut socket, accepted).await?;

    if !accepted {
        println!("Rejected IMEI {imei}, closing connection.");
        return Ok(());
    }

    let mut acc: Vec<u8> = Vec::with_capacity(8192);
    let mut ready_for_commands = false;

    loop {
        if ready_for_commands {
            while let Some(command) = command_queue.peek(&imei).await {
                if let Err(err) = execute_queued_command(
                    &mut socket,
                    &mut acc,
                    jetstream,
                    &imei,
                    make,
                    &command,
                )
                .await
                {
                    sentry::capture_error(&err);
                    eprintln!(
                        "command execution failed for {imei}: {err}; command remains queued"
                    );
                    return Err(err);
                }

                command_queue.remove_front(&imei).await;
            }
        }

        let mut buf = [0u8; 4096];
        if ready_for_commands {
            tokio::select! {
                read = socket.read(&mut buf) => {
                    let n = read?;

                    if n == 0 {
                        println!("Client disconnected: {:?}", peer_addr);
                        return Ok(());
                    }

                    println!("Received {} bytes: {}", n, hex::encode(&buf[..n]));
                    acc.extend_from_slice(&buf[..n]);

                    while let Some(frame) = try_extract_frame(&mut acc) {
                        if handle_unsolicited_frame(&mut socket, jetstream, &imei, make, frame).await? {
                            ready_for_commands = true;
                        }
                    }
                }
                _ = command_queue.wait_for_command(&imei) => {}
            }
        } else {
            let n = socket.read(&mut buf).await?;

            if n == 0 {
                println!("Client disconnected: {:?}", peer_addr);
                return Ok(());
            }

            println!("Received {} bytes: {}", n, hex::encode(&buf[..n]));
            acc.extend_from_slice(&buf[..n]);

            while let Some(frame) = try_extract_frame(&mut acc) {
                if handle_unsolicited_frame(&mut socket, jetstream, &imei, make, frame).await? {
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
) -> io::Result<()> {
    let frame = build_command_frame(&command.command);
    socket.write_all(&frame).await?;
    socket.flush().await?;
    println!(
        "Sent Codec12 command {} to IMEI {}: {:?}",
        command.request_id, imei, command.command
    );

    let response_timeout = Duration::from_millis(command.timeout_ms.unwrap_or(30_000));
    let response = timeout(
        response_timeout,
        wait_for_command_response(socket, acc, jetstream, imei, make),
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
) -> io::Result<String> {
    loop {
        while let Some(frame) = try_extract_frame(acc) {
            match classify_frame(&frame)? {
                IncomingFrame::Avl => {
                    handle_avl_frame(socket, jetstream, imei, make, frame).await?;
                }
                IncomingFrame::CommandResponse => {
                    return parse_response_frame(&frame);
                }
                IncomingFrame::Unsupported(codec_id) => {
                    eprintln!("ignoring unsupported Teltonika codec {codec_id:#x} while waiting for command response");
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

        println!("Received {} bytes: {}", n, hex::encode(&buf[..n]));
        acc.extend_from_slice(&buf[..n]);
    }
}

async fn handle_unsolicited_frame(
    socket: &mut tokio::net::TcpStream,
    jetstream: &Context,
    imei: &str,
    make: UnitMake,
    frame: Vec<u8>,
) -> io::Result<bool> {
    match classify_frame(&frame)? {
        IncomingFrame::Avl => {
            handle_avl_frame(socket, jetstream, imei, make, frame).await?;
            Ok(true)
        }
        IncomingFrame::CommandResponse => {
            let response = parse_response_frame(&frame)?;
            eprintln!("unsolicited Codec12 response from {imei}: {response}");
            Ok(false)
        }
        IncomingFrame::Unsupported(codec_id) => {
            eprintln!("ignoring unsupported Teltonika codec {codec_id:#x} from {imei}");
            Ok(false)
        }
    }
}

async fn handle_avl_frame(
    socket: &mut tokio::net::TcpStream,
    jetstream: &Context,
    imei: &str,
    make: UnitMake,
    frame: Vec<u8>,
) -> io::Result<()> {
    let data = match teltonika_parse_frame(&frame, imei) {
        Ok(data) => data,
        Err(err) => {
            if let Some(ack_record_count) = err.ack_record_count() {
                eprintln!("discarded frame: {err}");
                teltonika_write_frame_ack(socket, ack_record_count).await?;
            } else {
                match err {
                    TeltonikaFrameError::Parse(err) => {
                        sentry::capture_error(&err);
                        eprintln!("parse error: {err}");
                    }
                    TeltonikaFrameError::Discarded { .. } => unreachable!(),
                }
            }
            return Ok(());
        }
    };

    nats_publish(jetstream, &data, imei, make).await?;
    teltonika_print(&data);
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

fn try_extract_frame(acc: &mut Vec<u8>) -> Option<Vec<u8>> {
    loop {
        if acc.len() < 8 {
            return None;
        }

        if acc[0..4] != [0, 0, 0, 0] {
            if let Some(pos) = acc.windows(4).position(|w| w == [0, 0, 0, 0]) {
                acc.drain(..pos);
            } else {
                acc.clear();
                return None;
            }
        }

        if acc.len() < 8 {
            return None;
        }

        let data_len = u32::from_be_bytes(acc[4..8].try_into().unwrap()) as usize;
        let frame_len = 8 + data_len + 4;

        if acc.len() < frame_len {
            return None;
        }

        return Some(acc.drain(..frame_len).collect());
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
