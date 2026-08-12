mod types;

use std::io;

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use openssl::ssl::{SslConnector, SslMethod};
use postgres_openssl::MakeTlsConnector;
use tokio_postgres::{Config, Error as PgError};

pub use self::types::UnitMake;

fn pg_to_io(context: &'static str, e: PgError) -> io::Error {
    io::Error::other(format!("{context}: {e}"))
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

// !! If you remove this check you will be excuted at dawn !!
// We dropped FKs in avl data cuz we have this. Do not muck about with this.
pub async fn get_unit_make(pool: &DbPool, imei: &str) -> io::Result<Option<UnitMake>> {
    let imei = imei.trim();
    if imei.is_empty() {
        return Ok(None);
    }

    let client = pool.get().await.map_err(other)?;

    let row_opt = client
        .query_opt(r#"SELECT make::text FROM "Unit" WHERE imei = $1"#, &[&imei])
        .await
        .map_err(|e| pg_to_io("imei/make lookup err: ", e))?;

    let Some(row) = row_opt else {
        return Ok(None);
    };

    let make_str: String = row.get(0);
    Ok(UnitMake::from_db(&make_str))
}
