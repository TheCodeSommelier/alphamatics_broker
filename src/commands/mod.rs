use std::{
    io,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_nats::jetstream::{
    consumer::{self, AckPolicy},
    Context,
};
use futures_util::StreamExt;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use crate::redis::{RedisClient, redis_connect};

/// Sends of a command that ended without a result (e.g. the device
/// disconnected mid-command) before it is failed with `max_attempts`.
pub const MAX_COMMAND_ATTEMPTS: u32 = 3;
const DEFAULT_COMMAND_TTL_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, Deserialize)]
pub struct CommandPayload {
    pub request_id: String,
    pub command: String,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Unix ms after which the command is failed instead of sent. Defaults to
    /// now + `COMMAND_TTL_SECS` (24h).
    #[serde(default)]
    pub expires_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedCommand {
    pub imei: String,
    pub request_id: String,
    pub command: String,
    pub timeout_ms: Option<u64>,
    // Defaulted so commands queued before these fields existed still load;
    // those never expire.
    #[serde(default)]
    pub expires_at_ms: Option<u64>,
    #[serde(default)]
    pub attempts: u32,
}

impl QueuedCommand {
    pub fn is_expired(&self) -> bool {
        self.expires_at_ms.is_some_and(|expires_at_ms| now_ms() >= expires_at_ms)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandError {
    /// The device did not answer within `timeout_ms`.
    Timeout,
    /// `expires_at_ms` passed before the command could be sent.
    Expired,
    /// The command was sent `MAX_COMMAND_ATTEMPTS` times without a result.
    MaxAttempts,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandResponsePayload {
    pub request_id: String,
    pub imei: String,
    pub command: String,
    pub response: String,
    pub ok: bool,
    pub error: Option<CommandError>,
}

impl CommandResponsePayload {
    pub fn success(command: &QueuedCommand, response: String) -> Self {
        Self {
            request_id: command.request_id.clone(),
            imei: command.imei.clone(),
            command: command.command.clone(),
            response,
            ok: true,
            error: None,
        }
    }

    pub fn failure(command: &QueuedCommand, error: CommandError) -> Self {
        Self {
            request_id: command.request_id.clone(),
            imei: command.imei.clone(),
            command: command.command.clone(),
            response: String::new(),
            ok: false,
            error: Some(error),
        }
    }
}

#[derive(Clone)]
pub struct CommandQueue {
    redis: RedisClient,
    notify: Arc<Notify>,
}

impl CommandQueue {
    pub fn connect() -> io::Result<Self> {
        Ok(Self {
            redis: redis_connect()?,
            notify: Arc::new(Notify::new()),
        })
    }

    async fn connection(&self) -> io::Result<redis::aio::MultiplexedConnection> {
        self.redis
            .get_multiplexed_async_connection()
            .await
            .map_err(io::Error::other)
    }

    pub async fn enqueue(&self, command: QueuedCommand) -> io::Result<usize> {
        let mut redis = self.connection().await?;
        let payload_key = command_payload_key(&command.imei, &command.request_id);
        let queue_key = command_queue_key(&command.imei);
        let payload = serde_json::to_string(&command).map_err(io::Error::other)?;
        let _: () = redis::pipe()
            .atomic()
            .cmd("SET")
            .arg(&payload_key)
            .arg(payload)
            .ignore()
            .cmd("RPUSH")
            .arg(&queue_key)
            .arg(&command.request_id)
            .ignore()
            .query_async(&mut redis)
            .await
            .map_err(io::Error::other)?;
        let len: usize = redis.llen(&queue_key).await.map_err(io::Error::other)?;
        self.notify.notify_waiters();
        Ok(len)
    }

    pub async fn peek(&self, imei: &str) -> io::Result<Option<QueuedCommand>> {
        let mut redis = self.connection().await?;
        let queue_key = command_queue_key(imei);
        let req_id: Option<String> = redis
            .lindex(&queue_key, 0)
            .await
            .map_err(io::Error::other)?;
        let Some(req_id) = req_id else {
            return Ok(None);
        };
        let payload_key = command_payload_key(imei, &req_id);
        let payload: Option<String> = redis.get(&payload_key).await.map_err(io::Error::other)?;
        let payload = payload
            .map(|payload| serde_json::from_str(&payload).map_err(io::Error::other))
            .transpose()?;

        Ok(payload)
    }

    pub async fn remove_front(&self, imei: &str) -> io::Result<Option<QueuedCommand>> {
        let Some(command) = self.peek(imei).await? else {
            return Ok(None);
        };
        let mut redis = self.connection().await?;
        let queue_key = command_queue_key(imei);
        let payload_key = command_payload_key(imei, &command.request_id);
        let _: () = redis::pipe()
            .atomic()
            .cmd("LPOP")
            .arg(&queue_key)
            .ignore()
            .cmd("DEL")
            .arg(&payload_key)
            .ignore()
            .query_async(&mut redis)
            .await
            .map_err(io::Error::other)?;
        Ok(Some(command))
    }

    /// Persists an incremented attempt count before the command is sent, so a
    /// crash or disconnect mid-command still counts towards the limit.
    pub async fn record_attempt(&self, command: &QueuedCommand) -> io::Result<()> {
        let mut redis = self.connection().await?;
        let payload_key = command_payload_key(&command.imei, &command.request_id);
        let payload = serde_json::to_string(command).map_err(io::Error::other)?;
        // XX: don't resurrect a command that was removed in the meantime.
        let _: Option<String> = redis::cmd("SET")
            .arg(&payload_key)
            .arg(payload)
            .arg("XX")
            .query_async(&mut redis)
            .await
            .map_err(io::Error::other)?;
        Ok(())
    }

    pub async fn has_pending(&self, imei: &str) -> io::Result<bool> {
        let mut redis = self.connection().await?;
        let len: usize = redis
            .llen(command_queue_key(imei))
            .await
            .map_err(io::Error::other)?;
        Ok(len > 0)
    }

    pub async fn wait_for_command(&self, imei: &str) -> io::Result<()> {
        while !self.has_pending(imei).await? {
            self.notify.notified().await;
        }

        Ok(())
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn command_ttl() -> Duration {
    let secs = dotenvy::var("COMMAND_TTL_SECS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(DEFAULT_COMMAND_TTL_SECS);
    Duration::from_secs(secs)
}

fn command_payload_key(imei: &str, req_id: &str) -> String {
    format!("command.{imei}.{req_id}")
}

fn command_queue_key(imei: &str) -> String {
    format!("command.queue.{imei}")
}

pub async fn run_command_listener(jetstream: Context, command_queue: CommandQueue) -> io::Result<()> {
    let subject = command_subject();
    let stream_name = dotenvy::var("NATS_COMMAND_STREAM").unwrap_or("COMMANDS".to_string());
    let consumer_name =
        dotenvy::var("NATS_COMMAND_CONSUMER").unwrap_or("broker_commands".to_string());
    let stream = jetstream
        .get_stream(stream_name)
        .await
        .map_err(io::Error::other)?;
    let consumer = stream
        .create_consumer(consumer::pull::Config {
            durable_name: Some(consumer_name.clone()),
            filter_subject: subject.clone(),
            ack_policy: AckPolicy::Explicit,
            ack_wait: Duration::from_secs(30),
            max_deliver: 5,
            ..Default::default()
        })
        .await
        .map_err(io::Error::other)?;
    let mut messages = consumer.messages().await.map_err(io::Error::other)?;

    #[cfg(debug_assertions)]
    println!("Subscribed to JetStream command subject {subject} on consumer {consumer_name}");

    while let Some(message) = messages.next().await {
        let message = message.map_err(io::Error::other)?;

        if let Err(_err) = handle_message(&command_queue, message.subject.as_str(), &message.payload).await {
            #[cfg(debug_assertions)]
            let err = _err;

            #[cfg(debug_assertions)]
            eprintln!("failed to process command message on {}: {err}", message.subject);
            continue;
        }

        message.ack().await.map_err(io::Error::other)?;
    }

    Err(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        format!("command consumer closed for {subject}"),
    ))
}

async fn handle_message(
    command_queue: &CommandQueue,
    subject: &str,
    payload: &[u8],
) -> io::Result<()> {
    let imei = parse_subject(subject)?;
    let payload: CommandPayload = serde_json::from_slice(payload).map_err(io::Error::other)?;

    let expires_at_ms = payload
        .expires_at_ms
        .unwrap_or_else(|| now_ms().saturating_add(command_ttl().as_millis() as u64));

    let queued_command = QueuedCommand {
        imei,
        request_id: payload.request_id,
        command: payload.command,
        timeout_ms: payload.timeout_ms,
        expires_at_ms: Some(expires_at_ms),
        attempts: 0,
    };

    let _queue_len = command_queue.enqueue(queued_command.clone()).await?;

    #[cfg(debug_assertions)]
    println!(
        "Queued command {} for IMEI {}; timeout_ms: {:?}; payload: {:?}; queue depth: {}",
        queued_command.request_id,
        queued_command.imei,
        queued_command.timeout_ms,
        queued_command.command,
        _queue_len
    );

    Ok(())
}

fn parse_subject(subject: &str) -> io::Result<String> {
    let imei = match subject.split('.').collect::<Vec<_>>().as_slice() {
        ["command", imei] => *imei,
        ["units", "command", imei] => *imei,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unexpected command subject: {subject}"),
            ))
        }
    };

    if !imei.chars().all(|c| c.is_ascii_digit()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid imei in subject: {imei}"),
        ));
    }

    Ok(imei.to_string())
}

fn command_subject() -> String {
    dotenvy::var("NATS_COMMAND_SUBJECT")
        .or_else(|_| dotenvy::var("NATS_SUBJECT_COMMANDS"))
        .unwrap_or("units.command.*".to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        CommandError, CommandResponsePayload, QueuedCommand, command_subject, now_ms,
        parse_subject,
    };

    fn queued(expires_at_ms: Option<u64>) -> QueuedCommand {
        QueuedCommand {
            imei: "123456789012345".to_string(),
            request_id: "req-1".to_string(),
            command: "getinfo".to_string(),
            timeout_ms: None,
            expires_at_ms,
            attempts: 0,
        }
    }

    #[test]
    fn loads_commands_queued_before_attempts_and_expiry_existed() {
        let command: QueuedCommand = serde_json::from_str(
            r#"{"imei":"123456789012345","request_id":"req-1","command":"getinfo","timeout_ms":null}"#,
        )
        .unwrap();

        assert_eq!(command.attempts, 0);
        assert_eq!(command.expires_at_ms, None);
        assert!(!command.is_expired());
    }

    #[test]
    fn expires_commands_past_their_deadline() {
        assert!(queued(Some(now_ms() - 1)).is_expired());
        assert!(!queued(Some(now_ms() + 60_000)).is_expired());
    }

    #[test]
    fn serializes_failure_responses_with_an_error_code() {
        let payload = CommandResponsePayload::failure(&queued(None), CommandError::MaxAttempts);

        assert_eq!(
            serde_json::to_string(&payload).unwrap(),
            r#"{"request_id":"req-1","imei":"123456789012345","command":"getinfo","response":"","ok":false,"error":"max_attempts"}"#
        );
    }

    #[test]
    fn serializes_success_responses_with_a_null_error() {
        let payload = CommandResponsePayload::success(&queued(None), "OK".to_string());

        assert_eq!(
            serde_json::to_string(&payload).unwrap(),
            r#"{"request_id":"req-1","imei":"123456789012345","command":"getinfo","response":"OK","ok":true,"error":null}"#
        );
    }

    #[test]
    fn parses_command_subject() {
        let imei = parse_subject("units.command.123456789012345").unwrap();

        assert_eq!(imei, "123456789012345");
    }

    #[test]
    fn parses_legacy_units_command_subject() {
        let imei = parse_subject("units.command.123456789012345").unwrap();

        assert_eq!(imei, "123456789012345");
    }

    #[test]
    fn rejects_unknown_subject_prefix() {
        let err = parse_subject("units.avl.123456789012345").unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn falls_back_to_units_command_subject() {
        assert_eq!(command_subject(), "units.command.*");
    }
}
