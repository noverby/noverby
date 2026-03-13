//! HTTP server startup and subsystem wiring.
//!
//! The [`run_server`] function initializes the secrets manager, event notifier,
//! XRPC context, and application state, then starts the axum HTTP server with
//! graceful shutdown support.

use std::sync::Arc;

use tracing::{error, info};

use crate::config::{Config, SecretsProvider};
use crate::notifier::Notifier;
use crate::router::{AppState, build_router};

/// Errors that can occur during server startup or operation.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("secrets manager initialization failed: {0}")]
    Secrets(String),

    #[error("http server error: {0}")]
    Http(#[from] std::io::Error),
}

/// Start the HTTP server and run until shutdown.
///
/// Initializes all subsystems (secrets manager, notifier, XRPC context),
/// builds the axum router, and serves on the configured listen address.
///
/// Shuts down gracefully on SIGTERM or SIGINT (Ctrl-C).
pub async fn run_server(
    cfg: Config,
    db: Arc<spindle_db::Database>,
    rbac: spindle_rbac::SpindleEnforcer,
) -> Result<(), ServerError> {
    // Initialize secrets manager based on configuration.
    let secrets: Arc<dyn spindle_secrets::Manager + Send + Sync> = match cfg.secrets.provider {
        SecretsProvider::Sqlite => {
            // For SQLite secrets, derive an encryption key from the auth token.
            // In production, a dedicated secrets key should be configured.
            let key_material = ring::digest::digest(&ring::digest::SHA256, cfg.token.as_bytes());
            let master_key = key_material.as_ref();
            let db_path = cfg.db_path.with_extension("secrets.db");
            match spindle_secrets::SqliteManager::new(&db_path, master_key) {
                Ok(mgr) => Arc::new(mgr),
                Err(e) => return Err(ServerError::Secrets(e.to_string())),
            }
        }
        SecretsProvider::OpenBao => {
            Arc::new(spindle_secrets::OpenBaoManager::new(
                &cfg.secrets.openbao.proxy_addr,
                &cfg.secrets.openbao.mount,
                &cfg.token,
            ))
        }
    };

    // Create event notifier (broadcast channel).
    let notifier = Arc::new(Notifier::new(1024));

    // Create XRPC context.
    let xrpc = Arc::new(spindle_xrpc::XrpcContext {
        db: Arc::clone(&db),
        rbac,
        secrets: Arc::clone(&secrets),
        did_web: cfg.did_web.clone(),
        owner: cfg.owner.clone(),
        token: cfg.token.clone(),
        dev: cfg.dev,
    });

    // Build application state.
    let state = Arc::new(AppState {
        db,
        notifier,
        xrpc,
        log_dir: cfg.log_dir.clone(),
        hostname: cfg.hostname.clone(),
    });

    // Build router.
    let app = build_router(state);

    // Bind and serve.
    let listener = tokio::net::TcpListener::bind(&cfg.listen_addr).await?;
    let local_addr = listener.local_addr()?;

    info!(
        listen_addr = %local_addr,
        hostname = %cfg.hostname,
        "HTTP server listening"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("HTTP server shut down gracefully");
    Ok(())
}

/// Wait for a shutdown signal (SIGTERM or Ctrl-C).
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {
            info!("received Ctrl-C, shutting down");
        }
        () = terminate => {
            info!("received SIGTERM, shutting down");
        }
    }
}
