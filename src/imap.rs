use anyhow::Result;
use futures::StreamExt;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::db;
use crate::parser;

pub async fn run_imap_client(
    host: &str,
    user: &str,
    pass: &str,
    dest_folder: &str,
    pool: Arc<SqlitePool>,
    status: Arc<RwLock<String>>,
) -> Result<()> {
    loop {
        info!("Connecting to IMAP...");
        {
            let mut w = status.write().await;
            *w = "Connecting to IMAP...".to_string();
        }
        match connect_and_process(host, user, pass, dest_folder, &pool, &status).await {
            Ok(_) => {
                info!("IMAP session finished, reconnecting...");
                let mut w = status.write().await;
                *w = "Disconnected, reconnecting...".to_string();
            }
            Err(e) => {
                error!("IMAP error: {:?}", e);
                {
                    let mut w = status.write().await;
                    *w = format!("IMAP error, retrying in 10s...");
                }
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            }
        }
    }
}

async fn connect_and_process(
    host: &str,
    user: &str,
    pass: &str,
    dest_folder: &str,
    pool: &SqlitePool,
    status: &Arc<RwLock<String>>,
) -> Result<()> {
    use std::sync::Arc as StdArc;
    use tokio::net::TcpStream;
    use tokio_rustls::rustls;

    let tcp = TcpStream::connect((host, 993)).await?;
    let mut root_cert_store = rustls::RootCertStore::empty();
    root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_cert_store)
        .with_no_client_auth();

    let connector = tokio_rustls::TlsConnector::from(StdArc::new(config));
    let domain = rustls::pki_types::ServerName::try_from(host.to_string())?;
    let tls = connector.connect(domain, tcp).await?;

    let client = async_imap::Client::new(tls);
    let mut session = client.login(user, pass).await.map_err(|(e, _)| e)?;

    // Attempt to create the destination folder. We ignore the error because it might already exist.
    let _ = session.create(dest_folder).await;

    session.select("INBOX").await?;

    {
        let mut w = status.write().await;
        *w = "Searching for DMARC reports...".to_string();
    }

    info!("were here");

    let messages = session
        .search("UNSEEN OR SUBJECT \"Report domain:\" OR SUBJECT \"DMARC\" SUBJECT \"=?utf-8?B?\"")
        .await?;
    let total_messages = messages.len();

    if total_messages > 0 {
        let message = format!("Found {} DMARC reports", total_messages);
        let mut w = status.write().await;
        info!("{}", message.clone());
        *w = message;
    }

    for (i, seq) in messages.iter().enumerate() {
        {
            let mut w = status.write().await;
            let message = format!("Analyzing email {} of {}", i + 1, total_messages);
            info!("{}", message.clone());
            *w = message;
        }

        let mut body_bytes: Option<Vec<u8>> = None;
        {
            let mut fetches = session.fetch(seq.to_string(), "RFC822").await?;
            if let Some(Ok(msg)) = fetches.next().await {
                if let Some(body) = msg.body() {
                    body_bytes = Some(body.to_vec());
                }
            }
        }

        if let Some(body) = body_bytes {
            if let Ok(parsed_data) = parser::parse_email(&body) {
                for report in parsed_data {
                    db::save_report(pool, report).await?;
                }
                // Move message to dest folder
                session.copy(seq.to_string(), dest_folder).await?;
                // Mark as seen and deleted from INBOX
                let store_stream = session
                    .store(seq.to_string(), "+FLAGS (\\Seen \\Deleted)")
                    .await?;
                tokio::pin!(store_stream);
                while let Some(_) = store_stream.next().await {}
            } else {
                warn!("Couldnt Parse Email")
            }
        } else {
            warn!("No Body")
        }
    }
    {
        let expunge_stream = session.expunge().await?;
        tokio::pin!(expunge_stream);
        while let Some(_) = expunge_stream.next().await {}
    }

    {
        let mut w = status.write().await;
        *w = "Listening for new emails...".to_string();
    }

    // Idle
    let mut idle = session.idle();
    idle.init().await?;
    let (idle_wait, _interrupt) = idle.wait();
    if let Ok(_) = idle_wait.await {
        info!("New message arrived, waking up...");
    }

    Ok(())
}
