use std::io;

use diesel::prelude::QueryableByName;
use diesel::sql_types::{Bool, Text};
use diesel_async::pooled_connection::deadpool::Pool;
use diesel_async::pooled_connection::{AsyncDieselConnectionManager, ManagerConfig};
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use rustls::{ClientConfig, RootCertStore};
use tokio_postgres_rustls::MakeRustlsConnect;

pub type DbPool = Pool<AsyncPgConnection>;

#[derive(QueryableByName)]
struct ExistsRow {
    #[diesel(sql_type = Bool)]
    exists: bool,
}

fn bad_conn<E: std::fmt::Display>(err: E) -> diesel::ConnectionError {
    diesel::ConnectionError::BadConnection(err.to_string())
}

pub fn build_pool(database_url: &str) -> io::Result<DbPool> {
    let mut mgr_cfg = ManagerConfig::<AsyncPgConnection>::default();
    mgr_cfg.custom_setup = Box::new(|url| {
        Box::pin(async move {
            let mut roots = RootCertStore::empty();
            let certs = rustls_native_certs::load_native_certs();
            for cert in certs.certs {
                roots.add(cert).map_err(bad_conn)?;
            }
            if !certs.errors.is_empty() {
                eprintln!(
                    "warning: {} native cert(s) failed to load",
                    certs.errors.len()
                );
            }

            let tls = MakeRustlsConnect::new(
                ClientConfig::builder()
                    .with_root_certificates(roots)
                    .with_no_client_auth(),
            );

            let (client, connection) = tokio_postgres::connect(url, tls).await.map_err(bad_conn)?;

            AsyncPgConnection::try_from_client_and_connection(client, connection).await
        })
    });

    let config =
        AsyncDieselConnectionManager::<AsyncPgConnection>::new_with_config(database_url, mgr_cfg);
    Pool::builder(config)
        .build()
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
}

pub async fn imei_allowed(pool: &DbPool, imei: &str) -> io::Result<bool> {
    let mut conn = pool
        .get()
        .await
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;

    let row: ExistsRow =
        diesel::sql_query("SELECT EXISTS (SELECT 1 FROM ImeiAllowlist WHERE imei = $1)")
            .bind::<Text, _>(imei)
            .get_result(&mut conn)
            .await
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;

    Ok(row.exists)
}
