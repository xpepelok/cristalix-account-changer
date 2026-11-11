use sqlx::{Row, SqliteExecutor};

use crate::error::Result;
use crate::model::PlayerCacheEntry;
use crate::store::Store;

impl Store {
    pub async fn load_player_cache(&self) -> Result<Vec<PlayerCacheEntry>> {
        let rows = sqlx::query("SELECT name, info, fetched_at FROM player_cache")
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|row| {
                Ok(PlayerCacheEntry {
                    name: row.try_get("name")?,
                    info_json: row.try_get("info")?,
                    fetched_at: row.try_get("fetched_at")?,
                })
            })
            .collect()
    }

    pub async fn upsert_player(&self, name: &str, info_json: &str, fetched_at: i64) -> Result<()> {
        upsert_player(&self.pool, name, info_json, fetched_at).await
    }

    pub async fn upsert_players(&self, entries: &[PlayerCacheEntry]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for entry in entries {
            upsert_player(&mut *tx, &entry.name, &entry.info_json, entry.fetched_at).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_player(&self, name: &str) -> Result<()> {
        sqlx::query("DELETE FROM player_cache WHERE name = ?1")
            .bind(name.to_lowercase())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn prune_players(&self, older_than: i64) -> Result<()> {
        sqlx::query("DELETE FROM player_cache WHERE fetched_at < ?1")
            .bind(older_than)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

async fn upsert_player<'e>(
    executor: impl SqliteExecutor<'e>,
    name: &str,
    info_json: &str,
    fetched_at: i64,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO player_cache (name, info, fetched_at) VALUES (?1, ?2, ?3) \
         ON CONFLICT (name) DO UPDATE SET info = ?2, fetched_at = ?3",
    )
    .bind(name.to_lowercase())
    .bind(info_json)
    .bind(fetched_at)
    .execute(executor)
    .await?;
    Ok(())
}
