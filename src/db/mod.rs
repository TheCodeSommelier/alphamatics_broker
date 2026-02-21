use std::io;

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use openssl::ssl::{SslConnector, SslMethod};
use postgres_openssl::MakeTlsConnector;
use tokio_postgres::{Config, Error as PgError};

fn pg_to_io(context: &'static str, e: PgError) -> io::Error {
    eprintln!("{context}: {e:?}");

    if let Some(db) = e.as_db_error() {
        eprintln!("  pg code: {:?}", db.code());
        eprintln!("  pg msg : {}", db.message());
        if let Some(detail) = db.detail() {
            eprintln!("  detail : {detail}");
        }
        if let Some(hint) = db.hint() {
            eprintln!("  hint   : {hint}");
        }
    }
    io::Error::new(io::ErrorKind::Other, e.to_string())
}

fn other<E: std::fmt::Display>(e: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e.to_string())
}

pub type DbPool = Pool;

pub fn build_pool() -> io::Result<DbPool> {
    let db_url = dotenvy::var("DATABASE_URL").map_err(other)?;

    let pg_cfg: Config = db_url.parse().map_err(other)?;

    let builder = SslConnector::builder(SslMethod::tls()).map_err(other)?;
    let connector = MakeTlsConnector::new(builder.build());

    let mgr_config = ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    };

    let mgr = Manager::from_config(pg_cfg, connector, mgr_config);

    Pool::builder(mgr).max_size(8).build().map_err(other)
}

pub async fn imei_allowed(pool: &DbPool, imei: &str) -> io::Result<bool> {
    let imei = imei.trim();
    if imei.is_empty() {
        return Ok(false);
    }

    let client = pool.get().await.map_err(other)?;

    let row = client
        .query_one(
            r#"SELECT EXISTS (SELECT 1 FROM "Unit" WHERE imei = $1)"#,
            &[&imei],
        )
        .await
        .map_err(|e| pg_to_io("imei err: ", e))?;

    Ok(row.get(0))
}
