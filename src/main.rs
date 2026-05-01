use async_nats::jetstream::Context;
use deadpool_postgres::Pool;
use std::io;
use tokio::net::{TcpListener, TcpStream};

use crate::{
    db::{build_pool, get_unit_make},
    nats::nats_connect,
    units::teltonika::{teltonika_listen, utils::teltonika_read_imei},
};

mod db;
mod nats;
mod units;

async fn process_socket(mut socket: TcpStream, pool: &Pool, jetstream: &Context) -> io::Result<()> {
    let peer_addr = socket.peer_addr().ok();
    println!("Peer addr: {:?}", peer_addr);

    let imei = teltonika_read_imei(&mut socket).await?;
    println!("IMEI: {imei}");

    let make = match get_unit_make(&pool, &imei).await {
        Ok(Some(make)) => make,
        Ok(None) => return Ok(()),
        Err(err) => {
            sentry::capture_error(&err);
            eprintln!("imei lookup error: {err}");
            return Ok(());
        }
    };

    let accepted = true;
    teltonika_listen(socket, accepted, imei, jetstream, make).await
}

async fn tokio_main() -> io::Result<()> {
    let addr = dotenvy::var("ADDR").unwrap_or("127.0.0.1:4001".to_string());
    println!("Binding broker listener on {addr}...");
    let listener = TcpListener::bind(&addr).await?;
    println!("Building database pool...");
    let pool = build_pool()?;
    println!("Connecting to NATS...");
    let jetstream = nats_connect().await?;
    println!("Broker listening on {addr}");

    loop {
        let (socket, _) = listener.accept().await?;
        let pool = pool.clone();
        let jetstream = jetstream.clone();

        tokio::spawn(async move {
            if let Err(err) = process_socket(socket, &pool, &jetstream).await {
                sentry::capture_error(&err);
                eprintln!("socket error: {err}");
            }
        });
    }
}

fn main() -> io::Result<()> {
    dotenvy::dotenv().ok();

    let environment = dotenvy::var("ENV")
        .unwrap_or("production".to_string());
    let sentry_dns = dotenvy::var("SENTRY_DSN").expect("SENTRY_DSN must be set");
    let _guard = sentry::init((
        sentry_dns,
        sentry::ClientOptions {
            environment: Some(environment.into()),
            release: sentry::release_name!(),
            send_default_pii: true,
            traces_sample_rate: 0.1,
            ..Default::default()
        },
    ));

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(io::Error::other)?;

    if let Err(err) = runtime.block_on(tokio_main()) {
        sentry::capture_error(&err);
        eprintln!("broker failed to start: {err}");
        return Err(err);
    }

    Ok(())
}
