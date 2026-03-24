mod db;
mod imap;
mod parser;
mod web;

use anyhow::Result;
use dotenvy::dotenv;
use sqlx::sqlite::SqlitePoolOptions;
use std::env;
use std::sync::Arc;
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .init();

    info!("Starting dmarc-explorer");

    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;
    
    sqlx::migrate!("./migrations").run(&pool).await?;
    
    let imap_host = env::var("IMAP_HOST").expect("IMAP_HOST must be set");
    let imap_user = env::var("IMAP_USER").expect("IMAP_USER must be set");
    let imap_pass = env::var("IMAP_PASS").expect("IMAP_PASS must be set");
    let imap_dest = env::var("IMAP_DEST_FOLDER").unwrap_or_else(|_| "DMARC".to_string());

    let pool_arc = Arc::new(pool.clone());
    let pool_for_imap = pool_arc.clone();

    tokio::spawn(async move {
        if let Err(e) = imap::run_imap_client(
            &imap_host,
            &imap_user,
            &imap_pass,
            &imap_dest,
            pool_for_imap,
        ).await {
            error!("IMAP client error: {:?}", e);
        }
    });

    let app = web::router(pool);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    info!("Web server listening on 0.0.0.0:3000");
    axum::serve(listener, app).await?;

    Ok(())
}
