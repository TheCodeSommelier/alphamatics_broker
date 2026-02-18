use std::io;

use tokio::io::AsyncReadExt;

use crate::units::teltonika::{
    data_parser::teltonika_parse_data,
    utils::{teltonika_print, write_imei_handshake},
};

pub mod data_parser;
pub mod types;
pub mod utils;

pub async fn teltonika_listen(
    mut socket: tokio::net::TcpStream,
    accepted: bool,
    imei: String,
) -> io::Result<()> {
    let peer_addr = socket.peer_addr().ok();
    write_imei_handshake(&mut socket, accepted).await?;

    if !accepted {
        println!("Rejected IMEI {imei}, closing connection.");
        return Ok(());
    }

    loop {
        let mut buf = [0u8; 4096];
        let n = socket.read(&mut buf).await?;

        println!("N: {n}");
        if n == 0 {
            println!("Client disconnected: {:?}", peer_addr);
            return Ok(());
        }

        println!("Received {} bytes: {}", n, hex::encode(&buf[..n]));

        // TODO: replace with read_teltonika_frame(&mut socket).await? and ACK logic
        let data = teltonika_parse_data(&buf).unwrap();

        teltonika_print(data);
    }
}
