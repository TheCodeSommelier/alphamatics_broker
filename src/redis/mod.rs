use std::io;

use redis::{ConnectionAddr, ConnectionInfo, IntoConnectionInfo};

use crate::config::aws::secrets_manager::var as config_var;

pub type RedisClient = redis::Client;

pub fn redis_connect() -> io::Result<RedisClient> {
    let redis_url = config_var("REDIS_URL").map_err(io::Error::other)?;
    let username = config_var("REDIS_USER").ok();
    let password = config_var("REDIS_PASS").ok();
    let port = config_var("REDIS_PORT").ok();

    let info = connection_info(
        &redis_url,
        username.as_deref(),
        password.as_deref(),
        port.as_deref(),
    )?;
    redis::Client::open(info).map_err(io::Error::other)
}

fn connection_info(
    url: &str,
    username: Option<&str>,
    password: Option<&str>,
    port: Option<&str>,
) -> io::Result<ConnectionInfo> {
    let url_has_port = url::Url::parse(url)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "REDIS_URL must be a valid URL"))?
        .port()
        .is_some();
    let mut info = url.into_connection_info().map_err(io::Error::other)?;

    if let Some(port) = port.filter(|_| !url_has_port) {
        let port: u16 = port.parse().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "REDIS_PORT must be a valid TCP port",
            )
        })?;
        if port == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "REDIS_PORT must be a valid TCP port",
            ));
        }
        let addr = match info.addr().clone() {
            ConnectionAddr::Tcp(host, _) => ConnectionAddr::Tcp(host, port),
            ConnectionAddr::TcpTls {
                host,
                insecure,
                tls_params,
                ..
            } => ConnectionAddr::TcpTls {
                host,
                port,
                insecure,
                tls_params,
            },
            other => other,
        };
        info = info.set_addr(addr);
    }

    let mut settings = info.redis_settings().clone();
    if let Some(username) = username.filter(|value| !value.is_empty()) {
        settings = settings.set_username(username);
    }
    if let Some(password) = password.filter(|value| !value.is_empty()) {
        settings = settings.set_password(password);
    }

    Ok(info.set_redis_settings(settings))
}

#[cfg(test)]
mod tests {
    use redis::ConnectionAddr;

    use super::connection_info;

    #[test]
    fn uses_separate_redis_credentials_and_port() {
        let info = connection_info(
            "redis://redis.example.com",
            Some("broker"),
            Some("password:with@specials"),
            Some("18079"),
        )
        .unwrap();

        assert!(
            matches!(info.addr(), ConnectionAddr::Tcp(host, 18079) if host == "redis.example.com")
        );
        assert_eq!(info.redis_settings().username(), Some("broker"));
        assert_eq!(
            info.redis_settings().password(),
            Some("password:with@specials")
        );
    }

    #[test]
    fn preserves_credentials_already_in_the_url() {
        let info = connection_info(
            "redis://existing:secret@localhost:6380/2",
            None,
            None,
            Some("18079"),
        )
        .unwrap();

        assert!(matches!(info.addr(), ConnectionAddr::Tcp(host, 6380) if host == "localhost"));
        assert_eq!(info.redis_settings().username(), Some("existing"));
        assert_eq!(info.redis_settings().password(), Some("secret"));
        assert_eq!(info.redis_settings().db(), 2);
    }
}
