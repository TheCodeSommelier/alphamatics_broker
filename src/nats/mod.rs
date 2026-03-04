use bytes::Bytes;
use std::io;

use async_nats::{
    connect,
    jetstream::{self, Context},
};

use crate::{db::UnitMake, units::teltonika::types::TeltonikaFrame};

pub async fn nats_connect() -> io::Result<jetstream::Context> {
    let nats_url = dotenvy::var("NATS_URL").expect("NATS_URL to be defined...");
    let client = connect(nats_url)
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let js = jetstream::new(client);

    ensure_stream(&js).await?;

    Ok(js)
}

pub async fn nats_publish(
    jetstream: &Context,
    data: &TeltonikaFrame,
    imei: &str,
    make: UnitMake
) -> io::Result<()> {
    let subject = format!("units.avl.{:?}.{imei}", make);

    let payload = serde_json::to_vec(data).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    jetstream
        .publish(subject, Bytes::from(payload))
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    Ok(())
}

// ==============================
//            Private
// ==============================

async fn ensure_stream(js: &Context) -> io::Result<()> {
    let subject = dotenvy::var("NATS_SUBJECT").expect("NATS_SUBJECT has to be defined.");
    js.get_or_create_stream(async_nats::jetstream::stream::Config {
        name: "TELEMATICS".to_string(),
        subjects: vec![subject],
        max_messages: 10_000_000,
        ..Default::default()
    })
    .await
    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    Ok(())
}
