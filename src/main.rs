use async_nats::jetstream::Context;
use socket2::{SockRef, TcpKeepalive};
use std::{io, time::Duration};
use tokio::{
    net::{TcpListener, TcpStream},
    time::{sleep, timeout},
};

use crate::{
    commands::{CommandQueue, run_command_listener},
    db::{UnitCache, build_pool},
    nats::nats_connect,
    rfid::RfidEnrollmentPublisher,
    units::teltonika::{teltonika_listen, utils::teltonika_read_imei},
};

mod commands;
mod db;
mod nats;
mod redis;
mod rfid;
mod units;

/// A device must send its IMEI this soon after connecting. Also bounds NLB
/// health checks and port scanners that connect and never speak.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// Backoff after a failed accept (e.g. EMFILE) instead of spinning or exiting.
const ACCEPT_ERROR_BACKOFF: Duration = Duration::from_millis(100);

/// Detects dead peers (devices dropping off GSM without a FIN) and keeps NLB
/// flows alive past its 350s idle timeout.
fn enable_keepalive(socket: &TcpStream) -> io::Result<()> {
    let keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(60))
        .with_interval(Duration::from_secs(15))
        .with_retries(4);

    SockRef::from(socket).set_tcp_keepalive(&keepalive)
}

async fn process_socket(
    mut socket: TcpStream,
    units: &UnitCache,
    jetstream: &Context,
    command_queue: CommandQueue,
    rfid_publisher: RfidEnrollmentPublisher,
) -> io::Result<()> {
    #[cfg(debug_assertions)]
    let peer_addr = socket.peer_addr().ok();

    #[cfg(debug_assertions)]
    println!("Peer addr: {:?}", peer_addr);

    enable_keepalive(&socket)?;

    let imei = timeout(HANDSHAKE_TIMEOUT, teltonika_read_imei(&mut socket))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "timed out waiting for IMEI"))??;

    #[cfg(debug_assertions)]
    println!("IMEI: {imei}");

    let make = match units.get_unit_make(&imei).await {
        Ok(Some(make)) => make,
        Ok(None) => return Ok(()),
        Err(_err) => {
            #[cfg(debug_assertions)]
            let err = _err;

            #[cfg(debug_assertions)]
            eprintln!("imei lookup error: {err}");
            return Ok(());
        }
    };

    let accepted = true;
    teltonika_listen(
        socket,
        accepted,
        imei,
        jetstream,
        make,
        command_queue,
        rfid_publisher,
    )
    .await
}

async fn tokio_main() -> io::Result<()> {
    let addr = dotenvy::var("ADDR").unwrap_or("127.0.0.1:4001".to_string());

    #[cfg(debug_assertions)]
    println!("Binding broker listener on {addr}...");

    let listener = TcpListener::bind(&addr).await?;

    #[cfg(debug_assertions)]
    println!("Building database pool...");

    let units = UnitCache::new(build_pool()?);

    #[cfg(debug_assertions)]
    println!("Connecting to NATS...");

    let jetstream = nats_connect().await?;

    #[cfg(debug_assertions)]
    println!("Connecting to Redis...");

    let command_queue = CommandQueue::connect()?;
    let rfid_publisher = RfidEnrollmentPublisher::connect(jetstream.client())?;
    let command_listener = run_command_listener(jetstream.clone(), command_queue.clone());

    tokio::spawn(async move {
        if let Err(err) = command_listener.await {
            sentry::capture_error(&err);
            eprintln!("CRITICAL: command listener stopped: {err}");
        }
    });

    #[cfg(debug_assertions)]
    println!("Broker listening on {addr}");

    loop {
        let socket = match listener.accept().await {
            Ok((socket, _)) => socket,
            Err(err) => {
                // Usually fd exhaustion; existing connections are still fine,
                // so back off and keep serving rather than taking them down.
                eprintln!("accept error: {err}");
                sleep(ACCEPT_ERROR_BACKOFF).await;
                continue;
            }
        };
        let units = units.clone();
        let jetstream = jetstream.clone();
        let command_queue = command_queue.clone();
        let rfid_publisher = rfid_publisher.clone();

        tokio::spawn(async move {
            if let Err(_err) = process_socket(
                socket,
                &units,
                &jetstream,
                command_queue,
                rfid_publisher,
            )
            .await
            {
                #[cfg(debug_assertions)]
                let err = _err;

                #[cfg(debug_assertions)]
                eprintln!("socket error: {err}");
            }
        });
    }
}

fn main() -> io::Result<()> {
    dotenvy::dotenv().ok();

    let environment = dotenvy::var("ENV").unwrap_or("production".to_string());
    let sentry_dns = dotenvy::var("SENTRY_DSN").expect("SENTRY_DSN must be set");
    let _guard = sentry::init((
        sentry_dns,
        sentry::ClientOptions {
            environment: Some(environment.into()),
            release: sentry::release_name!(),
            send_default_pii: true,
            traces_sample_rate: 0.0,
            ..Default::default()
        },
    ));

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(io::Error::other)?;

    if let Err(err) = runtime.block_on(tokio_main()) {
        sentry::capture_error(&err);
        eprintln!("CRITICAL: broker stopped: {err}");
        return Err(err);
    }

    Ok(())
}
