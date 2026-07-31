use std::{io, sync::Arc, time::Duration};

use async_nats::jetstream::{
    consumer::{self, AckPolicy},
    Context,
};
use futures_util::StreamExt;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use crate::redis::{RedisConnection, redis_connect};

#[derive(Debug, Clone, Deserialize)]
pub struct CommandPayload {
    pub request_id: String,
    pub command: String,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedCommand {
    pub imei: String,
    pub request_id: String,
    pub command: String,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandResponsePayload {
    pub request_id: String,
    pub imei: String,
    pub command: String,
    pub response: String,
    pub ok: bool,
}

#[derive(Clone)]
pub struct CommandQueue {
    redis: RedisConnection,
    notify: Arc<Notify>,
}

impl CommandQueue {
    pub async fn connect() -> io::Result<Self> {
        Ok(Self {
            redis: redis_connect().await?,
            notify: Arc::new(Notify::new()),
        })
    }

    pub async fn enqueue(&self, command: QueuedCommand) -> usize {
        let mut redis = self.redis.clone();
        let payload_key = command_payload_key(&command.imei, &command.request_id);
        let queue_key = command_queue_key(&command.imei);
        let payload = serde_json::to_string(&command).expect("queued command to serialize");
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
            .expect("redis enqueue to succeed");
        let len: usize = redis.llen(&queue_key).await.expect("redis llen to succeed");
        self.notify.notify_waiters();
        len
    }

    pub async fn peek(&self, imei: &str) -> Option<QueuedCommand> {
        let mut redis = self.redis.clone();
        let queue_key = command_queue_key(imei);
        let req_id: Option<String> = redis
            .lindex(&queue_key, 0)
            .await
            .expect("redis lindex to succeed");
        let req_id = req_id?;
        let payload_key = command_payload_key(imei, &req_id);
        let payload: Option<String> = redis.get(&payload_key).await.expect("redis get to succeed");
        payload.and_then(|payload| serde_json::from_str(&payload).ok())
    }

    pub async fn remove_front(&self, imei: &str) -> Option<QueuedCommand> {
        let command = self.peek(imei).await?;
        let mut redis = self.redis.clone();
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
            .expect("redis remove_front to succeed");
        Some(command)
    }

    pub async fn has_pending(&self, imei: &str) -> bool {
        let mut redis = self.redis.clone();
        let len: usize = redis
            .llen(command_queue_key(imei))
            .await
            .expect("redis llen to succeed");
        len > 0
    }

    pub async fn wait_for_command(&self, imei: &str) {
        while !self.has_pending(imei).await {
            self.notify.notified().await;
        }
    }
}

fn command_payload_key(imei: &str, req_id: &str) -> String {
    return format!("command.{imei}.{req_id}");
}

fn command_queue_key(imei: &str) -> String {
    return format!("command.queue.{imei}");
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

    println!("Subscribed to JetStream command subject {subject} on consumer {consumer_name}");

    while let Some(message) = messages.next().await {
        let message = message.map_err(io::Error::other)?;

        if let Err(err) =
            handle_message(&command_queue, message.subject.as_str(), &message.payload).await
        {
            sentry::capture_error(&err);
            eprintln!("failed to process command message on {}: {err}", message.subject);
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

    let queued_command = QueuedCommand {
        imei,
        request_id: payload.request_id,
        command: payload.command,
        timeout_ms: payload.timeout_ms,
    };

    let queue_len = command_queue.enqueue(queued_command.clone()).await;
    println!(
        "Queued command {} for IMEI {}; timeout_ms: {:?}; payload: {:?}; queue depth: {}",
        queued_command.request_id,
        queued_command.imei,
        queued_command.timeout_ms,
        queued_command.command,
        queue_len
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
    use super::{command_subject, parse_subject};

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
