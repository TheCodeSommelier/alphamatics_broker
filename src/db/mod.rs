mod types;

use std::{collections::HashMap, io, sync::Arc};

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

/// Snapshot of known units loaded once at startup. Changes to Unit records
/// take effect when the broker restarts.
// AVL data has no unit foreign key; this snapshot determines which devices
// are accepted. Restart the broker after deleting a unit.
#[derive(Clone)]
pub struct UnitCache {
    units: Arc<HashMap<String, UnitMake>>,
}

impl UnitCache {
    pub async fn load(pool: DbPool) -> io::Result<Self> {
        let client = pool.get().await.map_err(other)?;
        let rows = client
            .query(r#"SELECT imei, make::text FROM "Unit""#, &[])
            .await
            .map_err(|e| pg_to_io("unit snapshot lookup err: ", e))?;

        let mut units = HashMap::with_capacity(rows.len());
        for row in rows {
            let imei: String = row.get(0);
            let make: String = row.get(1);
            if let Some(make) = UnitMake::from_db(&make) {
                units.insert(imei, make);
            }
        }

        tracing::info!(count = units.len(), "loaded units from database");
        Ok(Self {
            units: Arc::new(units),
        })
    }

    pub fn get_unit_make(&self, imei: &str) -> Option<UnitMake> {
        self.units.get(imei.trim()).copied()
    }
}
