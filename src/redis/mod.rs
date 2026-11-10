use std::io;

use crate::config::aws::secrets_manager::var as config_var;

pub type RedisClient = redis::Client;

pub fn redis_connect() -> io::Result<RedisClient> {
    let redis_url = config_var("REDIS_URL").expect("REDIS_URL has to be defined.");
    redis::Client::open(redis_url).map_err(io::Error::other)
}
