mod types;

use std::{
    io,
    time::{Duration, Instant},
};

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use moka::{Expiry, future::Cache};
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

const KNOWN_UNIT_TTL: Duration = Duration::from_secs(10 * 60);
const UNKNOWN_UNIT_TTL: Duration = Duration::from_secs(60);
const UNIT_CACHE_CAPACITY: u64 = 100_000;

/// Caches IMEI -> make lookups so reconnect storms (deploys, NLB failover)
/// don't queue every handshake behind the small Postgres pool. Unknown IMEIs
/// are cached briefly so a newly enrolled unit is picked up within a minute;
/// lookup errors are never cached.
#[derive(Clone)]
pub struct UnitCache {
    pool: DbPool,
    cache: Cache<String, Option<UnitMake>>,
}

impl UnitCache {
    pub fn new(pool: DbPool) -> Self {
        let cache = Cache::builder()
            .max_capacity(UNIT_CACHE_CAPACITY)
            .expire_after(UnitTtl)
            .build();

        Self { pool, cache }
    }

    pub async fn get_unit_make(&self, imei: &str) -> io::Result<Option<UnitMake>> {
        let imei = imei.trim();
        if imei.is_empty() {
            return Ok(None);
        }

        // Concurrent lookups for the same IMEI share a single query.
        self.cache
            .try_get_with(imei.to_string(), get_unit_make(&self.pool, imei))
            .await
            .map_err(|err| io::Error::new(err.kind(), err.to_string()))
    }
}

struct UnitTtl;

impl Expiry<String, Option<UnitMake>> for UnitTtl {
    fn expire_after_create(
        &self,
        _imei: &String,
        make: &Option<UnitMake>,
        _created_at: Instant,
    ) -> Option<Duration> {
        Some(if make.is_some() {
            KNOWN_UNIT_TTL
        } else {
            UNKNOWN_UNIT_TTL
        })
    }
}

// !! If you remove this check you will be excuted at dawn !!
// We dropped FKs in avl data cuz we have this. Do not muck about with this.
// Cached through `UnitCache`, which must keep expiring entries so deleted
// units stop being accepted.
async fn get_unit_make(pool: &DbPool, imei: &str) -> io::Result<Option<UnitMake>> {
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
