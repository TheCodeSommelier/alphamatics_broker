use std::io;
use tokio::net::{TcpListener, TcpStream};

use crate::units::teltonika::{teltonika_listen, utils::teltonika_read_imei};

mod units;

async fn process_socket(mut socket: TcpStream) -> io::Result<()> {
    let peer_addr = socket.peer_addr().ok();
    println!("Peer addr: {:?}", peer_addr);

    let imei = teltonika_read_imei(&mut socket).await?;
    println!("IMEI: {imei}");

    // TODO: Implement proper check with DB
    let accepted = imei.len() == 15;

    teltonika_listen(socket, accepted, imei).await?;

    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let addr = "127.0.0.1:4001";
    let listener = TcpListener::bind(addr).await?;

    loop {
        println!("Listening on: {addr}");
        let (socket, _) = listener.accept().await?;
        process_socket(socket).await?;
    }
}
