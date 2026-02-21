use deadpool_postgres::Pool;
use std::io;
use tokio::net::{TcpListener, TcpStream};

use crate::{
    db::{build_pool, imei_allowed},
    units::teltonika::{teltonika_listen, utils::teltonika_read_imei},
};

mod db;
mod units;

async fn process_socket(mut socket: TcpStream, pool: &Pool) -> io::Result<()> {
    let peer_addr = socket.peer_addr().ok();
    println!("Peer addr: {:?}", peer_addr);

    let imei = teltonika_read_imei(&mut socket).await?;
    println!("IMEI: {imei}");

    let accepted = match imei_allowed(&pool, &imei).await {
        Ok(allowed) => allowed,
        Err(err) => {
            sentry::capture_error(&err);
            eprintln!("imei lookup error: {err}");
            false
        }
    };

    teltonika_listen(socket, accepted, imei).await
}

async fn tokio_main() -> io::Result<()> {
    let addr = "127.0.0.1:4001";
    let listener = TcpListener::bind(addr).await?;
    let pool = build_pool()?;

    loop {
        println!("Listening on: {addr}");

        match listener.accept().await {
            Ok((socket, _)) => {
                if let Err(err) = process_socket(socket, &pool).await {
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
    dotenvy::dotenv().ok();

    let sentry_dns = dotenvy::var("SENTRY_DSN").expect("SENTRY_DSN must be set");
    let _guard = sentry::init((
        sentry_dns,
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
