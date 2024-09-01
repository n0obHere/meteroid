use crate::compute::clients::usage::UsageClient;
use crate::errors::StoreError;
use crate::StoreResult;
use common_eventbus::{Event, EventBus};
use diesel::{ConnectionError, ConnectionResult};
use diesel_async::pooled_connection::deadpool::Object;
use diesel_async::pooled_connection::deadpool::Pool;
use diesel_async::pooled_connection::{AsyncDieselConnectionManager, ManagerConfig};
use diesel_async::scoped_futures::{ScopedBoxFuture, ScopedFutureExt};
use diesel_async::{AsyncConnection, AsyncPgConnection};
use error_stack::{Report, ResultExt};
use futures::future::BoxFuture;
use futures::FutureExt;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, Error, SignatureScheme};
use std::str::FromStr;
use std::sync::Arc;
use tokio_postgres_rustls::MakeRustlsConnect;

pub type PgPool = Pool<AsyncPgConnection>;
pub type PgConn = Object<AsyncPgConnection>;

#[derive(Clone)]
pub struct Settings {
    pub crypt_key: secrecy::SecretString,
    pub jwt_secret: secrecy::SecretString,
    pub multi_organization_enabled: bool,
}

#[derive(Clone)]
pub struct Store {
    pub pool: PgPool,
    pub eventbus: Arc<dyn EventBus<Event>>,
    pub(crate) usage_client: Arc<dyn UsageClient>,
    pub(crate) settings: Settings,
    pub(crate) internal: StoreInternal,
}

/**
 * Share store logic while allowing cross-service transactions
 * TODO divide between Service & Repository instead ?
 * Service => Exact mapping of the API, + validations, setup conn, call repository
 * Repository is often pass-through to diesel_models after mapping, but not always (can multiple queries, insert multiple entities, etc)
 */
#[derive(Clone)]
pub struct StoreInternal {}

pub fn diesel_make_pg_pool(db_url: String) -> StoreResult<PgPool> {
    let config = tokio_postgres::Config::from_str(db_url.as_str()).unwrap();

    let mgr: AsyncDieselConnectionManager<AsyncPgConnection> =
        if config.get_ssl_mode() != tokio_postgres::config::SslMode::Disable {
            let mut config = ManagerConfig::default();
            // First we have to construct a connection manager with our custom `establish_connection`
            // function
            config.custom_setup = Box::new(establish_secure_connection);

            // From that connection we can then create a pool, here given with some example settings.
            //
            // This creates a TLS configuration that's equivalent to `libpq` `sslmode=verify-full`, which
            // means this will check whether the provided certificate is valid for the given database host.
            //
            // `libpq` does not perform these checks by default (https://www.postgresql.org/docs/current/libpq-connect.html)
            // If you hit a TLS error while connecting to the database double-check your certificates

            AsyncDieselConnectionManager::<AsyncPgConnection>::new_with_config(db_url, config)
        } else {
            AsyncDieselConnectionManager::<AsyncPgConnection>::new(db_url)
        };

    Pool::builder(mgr)
        .build()
        .map_err(Report::from)
        .change_context(StoreError::InitializationError)
        .attach_printable("Failed to create PostgreSQL connection pool")
}

fn establish_secure_connection(config: &str) -> BoxFuture<ConnectionResult<AsyncPgConnection>> {
    let fut = async {
        // We first set up the way we want rustls to work.
        let rustls_config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(DummyTlsVerifier))
            .with_no_client_auth();
        let tls = MakeRustlsConnect::new(rustls_config);
        let (client, conn) = tokio_postgres::connect(config, tls)
            .await
            .map_err(|e| ConnectionError::BadConnection(e.to_string()))?;
        tokio::spawn(async move {
            if let Err(e) = conn.await {
                eprintln!("Database connection: {e}");
            }
        });
        AsyncPgConnection::try_from(client).await
    };
    fut.boxed()
}

fn root_certs() -> rustls::RootCertStore {
    let mut roots = rustls::RootCertStore::empty();
    let certs = rustls_native_certs::load_native_certs().expect("Certs not loadable!");
    roots.add_parsable_certificates(certs);
    roots
}

impl Store {
    pub fn new(
        database_url: String,
        crypt_key: secrecy::SecretString,
        jwt_secret: secrecy::SecretString,
        multi_organization_enabled: bool,
        eventbus: Arc<dyn EventBus<Event>>,
        usage_client: Arc<dyn UsageClient>,
    ) -> StoreResult<Self> {
        let pool: PgPool = diesel_make_pg_pool(database_url)?;

        Ok(Store {
            pool,
            eventbus,
            usage_client,
            settings: Settings {
                crypt_key,
                jwt_secret,
                multi_organization_enabled,
            },
            internal: StoreInternal {},
        })
    }

    pub async fn get_conn(&self) -> StoreResult<PgConn> {
        self.pool
            .get()
            .await
            .map_err(Report::from)
            .change_context(StoreError::DatabaseConnectionError)
            .attach_printable("Failed to get a connection from the pool")
    }

    // Temporary, evaluating if this simplifies the handling of store + diesel interactions within a transaction

    pub(crate) async fn transaction<'a, R, F>(&self, callback: F) -> StoreResult<R>
    where
        F: for<'r> FnOnce(
                &'r mut PgConn,
            )
                -> ScopedBoxFuture<'a, 'r, error_stack::Result<R, StoreError>>
            + Send
            + 'a,
        R: Send + 'a,
    {
        let mut conn = self.get_conn().await?;

        self.transaction_with(&mut conn, callback).await
    }

    pub(crate) async fn transaction_with<'a, R, F>(
        &self,
        conn: &mut PgConn,
        callback: F,
    ) -> StoreResult<R>
    where
        F: for<'r> FnOnce(
                &'r mut PgConn,
            )
                -> ScopedBoxFuture<'a, 'r, error_stack::Result<R, StoreError>>
            + Send
            + 'a,
        R: Send + 'a,
    {
        let result = conn
            .transaction(|conn| {
                async move {
                    let res = callback(conn);
                    res.await.map_err(StoreError::TransactionStoreError)
                }
                .scope_boxed()
            })
            .await?;

        Ok(result)
    }
}

#[derive(Debug)]
// this is to ignore certificates for some providers like DO
struct DummyTlsVerifier;

impl ServerCertVerifier for DummyTlsVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer,
        _intermediates: &[CertificateDer],
        _server_name: &ServerName,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}
