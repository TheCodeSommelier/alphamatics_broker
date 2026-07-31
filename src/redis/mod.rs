use std::io;

pub type RedisConnection = redis::aio::MultiplexedConnection;

pub async fn redis_connect() -> io::Result<RedisConnection> {
    let redis_url = dotenvy::var("REDIS_URL").expect("REDIS_URL has to be defined.");
    let client = redis::Client::open(redis_url).map_err(io::Error::other)?;

    client
        .get_multiplexed_async_connection()
        .await
        .map_err(io::Error::other)
}
