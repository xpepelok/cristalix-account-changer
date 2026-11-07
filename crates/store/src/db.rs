use std::path::Path;
use std::sync::Arc;

use sha2::{Digest, Sha384};
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
    reconcile_line_endings(&pool).await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

async fn reconcile_line_endings(pool: &SqlitePool) -> Result<()> {
    let tracked: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_optional(pool)
    .await?;
    if tracked.is_none() {
        return Ok(());
    }
    let applied: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("SELECT version, checksum FROM _sqlx_migrations")
            .fetch_all(pool)
            .await?;
    for migration in MIGRATOR.iter() {
        let Some((_, stored)) = applied
            .iter()
            .find(|(version, _)| *version == migration.version)
        else {
            continue;
        };
        if stored.as_slice() == &*migration.checksum {
            continue;
        }
        if stored.as_slice() == Sha384::digest(flip_line_endings(migration.sql.as_str())).as_slice()
        {
            sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = ?")
                .bind(&*migration.checksum)
                .bind(migration.version)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

fn flip_line_endings(sql: &str) -> String {
    if sql.contains("\r\n") {
        sql.replace("\r\n", "\n")
    } else {
        sql.replace('\n', "\r\n")
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn accepts_migrations_with_other_line_endings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.db");
        let pool = connect(&path).await.unwrap();
        let first = MIGRATOR.iter().next().unwrap();
        let foreign = Sha384::digest(flip_line_endings(first.sql.as_str())).to_vec();
        sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = ?")
            .bind(&foreign)
            .bind(first.version)
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;

        let pool = connect(&path).await.unwrap();
        let stored: Vec<u8> =
            sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version = ?")
                .bind(first.version)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(stored, first.checksum.to_vec());
    }
}
