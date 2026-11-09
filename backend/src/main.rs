use axum::{ServiceExt, extract::Request};
use backend::VERSION;
use std::{env::var, process::ExitCode};
use tokio::signal;

// Use jemalloc instead of the system allocator to curb RSS growth from glibc
// malloc arena retention/fragmentation under our threaded, bursty workload.
#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[tokio::main]
async fn main() -> ExitCode {
    let telemetry = backend::telemetry::init().await;
    let result = run().await;
    let exit = match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(
                name: "trainstatus.failed",
                { error.message = %format_args!("{error:#}") },
                "Backend failed"
            );
            ExitCode::FAILURE
        }
    };
    telemetry.shutdown().await;
    exit
}

async fn run() -> anyhow::Result<()> {
    tracing::info!(name: "trainstatus.started", version = VERSION, "Starting Train Status API");
    let application = backend::app::build().await?;

    let app = backend::app::http_service(application.state.clone());

    let listener =
        tokio::net::TcpListener::bind(var("ADDRESS").unwrap_or_else(|_| "127.0.0.1:3055".into()))
            .await?;
    tracing::info!(name: "trainstatus.listening", address = %listener.local_addr()?, "Listening");

    axum::serve(listener, ServiceExt::<Request>::into_make_service(app))
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            tracing::info!(name: "trainstatus.stopping", "Stopping Train Status API");
            application.shutdown();
        })
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
