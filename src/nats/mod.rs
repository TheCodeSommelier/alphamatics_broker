use bytes::Bytes;
use std::{io, time::Duration};

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
    let limits = stream_limits_from_env()?;
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
        max_age: limits.max_age,
        max_bytes: limits.telematics_max_bytes,
        storage: async_nats::jetstream::stream::StorageType::File,
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
        max_age: limits.max_age,
        max_bytes: limits.commands_max_bytes,
        storage: async_nats::jetstream::stream::StorageType::File,
        ..Default::default()
    })
    .await
    .map_err(io::Error::other)?;

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StreamLimits {
    max_age: Duration,
    telematics_max_bytes: i64,
    commands_max_bytes: i64,
}

fn stream_limits_from_env() -> io::Result<StreamLimits> {
    let max_age_hours =
        dotenvy::var("NATS_MAX_AGE_HOURS").unwrap_or_else(|_| "72".to_string());
    let telematics_max_bytes = required_env("NATS_TELEMATICS_MAX_BYTES")?;
    let commands_max_bytes = required_env("NATS_COMMANDS_MAX_BYTES")?;

    parse_stream_limits(
        &max_age_hours,
        &telematics_max_bytes,
        &commands_max_bytes,
    )
}

fn required_env(name: &str) -> io::Result<String> {
    dotenvy::var(name).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be set to a positive byte limit"),
        )
    })
}

fn parse_stream_limits(
    max_age_hours: &str,
    telematics_max_bytes: &str,
    commands_max_bytes: &str,
) -> io::Result<StreamLimits> {
    let max_age_hours = parse_positive_u64("NATS_MAX_AGE_HOURS", max_age_hours)?;
    let max_age_seconds = max_age_hours.checked_mul(60 * 60).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "NATS_MAX_AGE_HOURS is too large",
        )
    })?;

    Ok(StreamLimits {
        max_age: Duration::from_secs(max_age_seconds),
        telematics_max_bytes: parse_positive_i64(
            "NATS_TELEMATICS_MAX_BYTES",
            telematics_max_bytes,
        )?,
        commands_max_bytes: parse_positive_i64(
            "NATS_COMMANDS_MAX_BYTES",
            commands_max_bytes,
        )?,
    })
}

fn parse_positive_u64(name: &str, value: &str) -> io::Result<u64> {
    let value = value.trim().parse::<u64>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be a positive integer"),
        )
    })?;

    if value == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be greater than zero"),
        ));
    }

    Ok(value)
}

fn parse_positive_i64(name: &str, value: &str) -> io::Result<i64> {
    let value = value.trim().parse::<i64>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be a positive integer"),
        )
    })?;

    if value <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be greater than zero"),
        ));
    }

    Ok(value)
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
    use super::{parse_stream_limits, response_subject};
    use std::{io::ErrorKind, time::Duration};

    #[test]
    fn parses_stream_limits() {
        let limits = parse_stream_limits("72", "75000000000", "5000000000").unwrap();

        assert_eq!(limits.max_age, Duration::from_secs(72 * 60 * 60));
        assert_eq!(limits.telematics_max_bytes, 75_000_000_000);
        assert_eq!(limits.commands_max_bytes, 5_000_000_000);
    }

    #[test]
    fn rejects_zero_stream_limits() {
        let age_err = parse_stream_limits("0", "1", "1").unwrap_err();
        let telematics_err = parse_stream_limits("72", "0", "1").unwrap_err();
        let commands_err = parse_stream_limits("72", "1", "0").unwrap_err();

        assert_eq!(age_err.kind(), ErrorKind::InvalidInput);
        assert_eq!(telematics_err.kind(), ErrorKind::InvalidInput);
        assert_eq!(commands_err.kind(), ErrorKind::InvalidInput);
    }

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
