use bytes::Bytes;
use std::io;

use async_nats::{
    connect,
    jetstream::{self, Context},
};

use crate::{
    commands::CommandResponsePayload, db::UnitMake, units::teltonika::types::TeltonikaFrame,
};

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
    make: UnitMake,
) -> io::Result<()> {
    let subject = format!("units.avl.{:?}.{imei}", make);

    let payload = serde_json::to_vec(data).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    jetstream
        .publish(subject, Bytes::from(payload))
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    Ok(())
}

pub async fn nats_publish_command_response(
    jetstream: &Context,
    payload: &CommandResponsePayload,
) -> io::Result<()> {
    let subject_prefix =
        dotenvy::var("NATS_COMMAND_RESPONSE_SUBJECT")
            .unwrap_or("units.command_response.*".to_string());
    let subject = response_subject(&subject_prefix, &payload.imei)?;

    let payload = serde_json::to_vec(payload).map_err(io::Error::other)?;

    jetstream
        .publish(subject, Bytes::from(payload))
        .await
        .map_err(io::Error::other)?;

    Ok(())
}

// ==============================
//            Private
// ==============================

async fn ensure_stream(js: &Context) -> io::Result<()> {
    let telematics_stream = dotenvy::var("NATS_STREAM").unwrap_or("TELEMATICS".to_string());
    let command_stream = dotenvy::var("NATS_COMMAND_STREAM").unwrap_or("COMMANDS".to_string());
    let avl_subject = dotenvy::var("NATS_SUBJECT").expect("NATS_SUBJECT has to be defined.");
    let command_subject = command_subject();
    let command_response_subject =
        dotenvy::var("NATS_COMMAND_RESPONSE_SUBJECT")
            .unwrap_or("units.command_response.*".to_string());

    #[cfg(debug_assertions)]
    println!(
        "Ensuring stream {} with subjects {:?}",
        telematics_stream,
        vec![avl_subject.clone()]
    );
    js.create_or_update_stream(async_nats::jetstream::stream::Config {
        name: telematics_stream,
        subjects: vec![avl_subject],
        max_messages: 10_000_000,
        ..Default::default()
    })
    .await
    .map_err(io::Error::other)?;

    #[cfg(debug_assertions)]
    println!(
        "Ensuring stream {} with subjects {:?}",
        command_stream,
        vec![command_subject.clone(), command_response_subject.clone()]
    );
    js.create_or_update_stream(async_nats::jetstream::stream::Config {
        name: command_stream,
        subjects: vec![command_subject, command_response_subject],
        max_messages: 10_000_000,
        ..Default::default()
    })
    .await
    .map_err(io::Error::other)?;

    Ok(())
}

fn command_subject() -> String {
    dotenvy::var("NATS_COMMAND_SUBJECT")
        .or_else(|_| dotenvy::var("NATS_SUBJECT_COMMANDS"))
        .unwrap_or("units.command.*".to_string())
}

fn response_subject(template: &str, imei: &str) -> io::Result<String> {
    match template.split('.').collect::<Vec<_>>().as_slice() {
        [prefix @ .., "*"] if !prefix.is_empty() => Ok(format!("{}.{}", prefix.join("."), imei)),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid command response subject template: {template}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::response_subject;
    use std::io::ErrorKind;

    #[test]
    fn builds_response_subject_from_wildcard_template() {
        let subject = response_subject("units.command_response.*", "123456789012345").unwrap();

        assert_eq!(subject, "units.command_response.123456789012345");
    }

    #[test]
    fn rejects_response_subject_without_terminal_wildcard() {
        let err = response_subject("units.command_response", "123456789012345").unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }
}
