use std::io;

pub type RedisClient = redis::Client;

pub fn redis_connect() -> io::Result<RedisClient> {
    let redis_url = dotenvy::var("REDIS_URL").expect("REDIS_URL has to be defined.");
    redis::Client::open(redis_url).map_err(io::Error::other)
}
