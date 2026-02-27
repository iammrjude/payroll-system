use axum::{routing::get, Router};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{info, Level};
use tracing_subscriber::EnvFilter;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod auth;
mod config;
mod errors;
mod handlers;
mod models;
mod openapi;
mod routes;
mod services;
mod state;

use config::Config;
use handlers::general::{health_handler, root_handler};
use openapi::ApiDoc;
use routes::api_routes;
use state::AppState;

#[tokio::main]
async fn main() {
    // ─── Logging ──────────────────────────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("payroll_system=debug,tower_http=info")),
        )
        .with_max_level(Level::TRACE)
        .init();

    // ─── Config ───────────────────────────────────────────────────────────────
    let config = Config::from_env();
    let addr = config.server_addr();

    // ─── Database ─────────────────────────────────────────────────────────────
    let db = PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.database_url)
        .await
        .expect("Failed to connect to Postgres");

    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("Failed to run database migrations");

    info!("Database connected and migrations applied ✓");

    // ─── App State ────────────────────────────────────────────────────────────
    let state = AppState::new(db, config);

    // ─── Router ───────────────────────────────────────────────────────────────
    let app = Router::new()
        .route("/", get(root_handler))
        .route("/health", get(health_handler))
        .nest("/api/v1", api_routes())
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    // ─── Start Server ─────────────────────────────────────────────────────────
    info!("🚀 Payroll System API listening on http://{}", addr);
    info!("📖 Swagger UI:  http://{}/docs", addr);
    info!("❤️  Health:      http://{}/health", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind to address");

    axum::serve(listener, app)
        .await
        .expect("Server failed");
}