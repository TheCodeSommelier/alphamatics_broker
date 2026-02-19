use std::{env, io};
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

async fn tokio_main() -> io::Result<()> {
    let addr = "127.0.0.1:4001";
    let listener = TcpListener::bind(addr).await?;

    loop {
        println!("Listening on: {addr}");

        match listener.accept().await {
            Ok((socket, _)) => {
                if let Err(err) = process_socket(socket).await {
                    sentry::capture_error(&err);
                    eprintln!("socket error: {err}");
                }
            }
            Err(err) => {
                sentry::capture_error(&err);
                eprintln!("accept error: {err}");
            }
        }
    }
}
fn main() {
    let _guard = sentry::init((
        env::var("SETRY_DNS").expect("SETRY_DNS must be set"),
        sentry::ClientOptions {
            release: sentry::release_name!(),
            send_default_pii: true,
            traces_sample_rate: 0.1,
            ..Default::default()
        },
    ));

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let result = tokio_main().await;
            if let Err(err) = &result {
                sentry::capture_error(err);
            }
        });
}
