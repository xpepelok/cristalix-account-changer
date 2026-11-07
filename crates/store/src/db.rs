use std::path::Path;
use std::sync::Arc;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{SqlitePool, migrate::Migrator};

use ac_platform::Secrets;

use crate::error::Result;

pub(crate) static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub(crate) async fn connect(path: &Path) -> Result<SqlitePool> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

pub(crate) fn seal(secrets: &Arc<Secrets>, token: &str) -> Result<Option<Vec<u8>>> {
    if token.is_empty() {
        return Ok(None);
    }
    Ok(Some(secrets.encrypt(token.as_bytes())?))
}

pub(crate) fn unseal(secrets: &Arc<Secrets>, blob: Option<Vec<u8>>) -> Result<String> {
    let Some(blob) = blob else {
        return Ok(String::new());
    };
    if blob.is_empty() {
        return Ok(String::new());
    }
    let plain = secrets.decrypt(&blob)?;
    String::from_utf8(plain).map_err(|err| crate::error::StoreError::Corrupt(err.to_string()))
}

pub(crate) fn pack_pids(pids: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(pids.len() * 4);
    for pid in pids {
        out.extend_from_slice(&pid.to_le_bytes());
    }
    out
}

pub(crate) fn unpack_pids(blob: &[u8]) -> Vec<u32> {
    let (chunks, _) = blob.as_chunks::<4>();
    chunks.iter().copied().map(u32::from_le_bytes).collect()
}
