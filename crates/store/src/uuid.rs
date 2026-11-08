use sqlx::SqlitePool;

use crate::error::Result;

pub fn canonical_uuid(value: &str) -> Option<String> {
    let hex: String = value.chars().filter(|ch| *ch != '-').collect();
    let dashes = value.len() - hex.len();
    if hex.len() != 32
        || !(dashes == 0 || dashes == 4)
        || !hex.chars().all(|ch| ch.is_ascii_hexdigit())
    {
        return None;
    }
    let hex = hex.to_ascii_lowercase();
    Some(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

pub(crate) async fn repair(pool: &SqlitePool) -> Result<()> {
    let stray: Vec<String> = sqlx::query_scalar(
        "SELECT uuid FROM accounts \
         UNION SELECT uuid FROM forgotten \
         UNION SELECT uuid FROM game_launches \
         UNION SELECT uuid FROM log_clients",
    )
    .fetch_all(pool)
    .await?;
    let moves: Vec<(String, String)> = stray
        .into_iter()
        .filter_map(|uuid| {
            let canonical = canonical_uuid(&uuid)?;
            (canonical != uuid).then_some((uuid, canonical))
        })
        .collect();
    if moves.is_empty() {
        return Ok(());
    }
    let mut tx = pool.begin().await?;
    for (stray, canonical) in &moves {
        merge_account(&mut tx, stray, canonical).await?;
        sqlx::query("UPDATE OR IGNORE forgotten SET uuid = ?2 WHERE uuid = ?1")
            .bind(stray)
            .bind(canonical)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM forgotten WHERE uuid = ?1")
            .bind(stray)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE OR IGNORE game_launches SET uuid = ?2 WHERE uuid = ?1")
            .bind(stray)
            .bind(canonical)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM game_launches WHERE uuid = ?1")
            .bind(stray)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT OR IGNORE INTO log_clients (uuid, name) SELECT ?2, name FROM log_clients WHERE uuid = ?1")
            .bind(stray)
            .bind(canonical)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE log_sessions SET uuid = ?2 WHERE uuid = ?1")
            .bind(stray)
            .bind(canonical)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM log_clients WHERE uuid = ?1")
            .bind(stray)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn merge_account(
    tx: &mut sqlx::SqliteConnection,
    stray: &str,
    canonical: &str,
) -> Result<()> {
    let present: Option<i64> = sqlx::query_scalar("SELECT 1 FROM accounts WHERE uuid = ?1")
        .bind(stray)
        .fetch_optional(&mut *tx)
        .await?;
    if present.is_none() {
        return Ok(());
    }
    let inserted = sqlx::query(
        "INSERT OR IGNORE INTO accounts (uuid, name, token, label, pinned, client, profile, ram, \
             min_graphics, fullscreen, discord_rpc, auto_enter, debug_mode, render_distance, max_fps, \
             animations, fast_render, minimal, expires, first_seen, last_seen, last_launched, last_online) \
         SELECT ?2, name, token, label, pinned, client, profile, ram, \
             min_graphics, fullscreen, discord_rpc, auto_enter, debug_mode, render_distance, max_fps, \
             animations, fast_render, minimal, expires, first_seen, last_seen, last_launched, last_online \
         FROM accounts WHERE uuid = ?1",
    )
    .bind(stray)
    .bind(canonical)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if inserted == 0 {
        sqlx::query(
            "UPDATE accounts SET \
                 token = CASE WHEN stray.expires > accounts.expires THEN stray.token ELSE accounts.token END, \
                 expires = MAX(accounts.expires, stray.expires), \
                 name = CASE WHEN accounts.name = '' OR (stray.name <> '' AND stray.last_seen > accounts.last_seen) \
                             THEN stray.name ELSE accounts.name END, \
                 label = CASE WHEN accounts.label = '' THEN stray.label ELSE accounts.label END, \
                 pinned = MAX(accounts.pinned, stray.pinned), \
                 first_seen = CASE WHEN accounts.first_seen = 0 \
                                    OR (stray.first_seen > 0 AND stray.first_seen < accounts.first_seen) \
                                   THEN stray.first_seen ELSE accounts.first_seen END, \
                 last_seen = MAX(accounts.last_seen, stray.last_seen), \
                 last_launched = MAX(accounts.last_launched, stray.last_launched), \
                 last_online = MAX(accounts.last_online, stray.last_online) \
             FROM (SELECT * FROM accounts WHERE uuid = ?1) AS stray \
             WHERE accounts.uuid = ?2",
        )
        .bind(stray)
        .bind(canonical)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query(
        "INSERT OR IGNORE INTO group_members (group_id, account_uuid, position) \
         SELECT group_id, ?2, position FROM group_members WHERE account_uuid = ?1",
    )
    .bind(stray)
    .bind(canonical)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM accounts WHERE uuid = ?1")
        .bind(stray)
        .execute(&mut *tx)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::canonical_uuid;

    #[test]
    fn canonicalizes_dashless_and_upper_case() {
        let canonical = Some("e376c8f4-72ec-11e9-8374-1cb72caa35fd".to_owned());
        assert_eq!(
            canonical_uuid("e376c8f472ec11e983741cb72caa35fd"),
            canonical
        );
        assert_eq!(
            canonical_uuid("E376C8F4-72EC-11E9-8374-1CB72CAA35FD"),
            canonical
        );
        assert_eq!(
            canonical_uuid("e376c8f4-72ec-11e9-8374-1cb72caa35fd"),
            canonical
        );
    }

    #[test]
    fn rejects_other_values() {
        for value in [
            "",
            "abc",
            "e376c8f4-72ec-11e9-8374-1cb72caa35f",
            "e376c8f472ec11e983741cb72caa35fg",
            "e3-76c8f472ec11e983741cb72caa35fd",
        ] {
            assert_eq!(canonical_uuid(value), None, "{value}");
        }
    }
}
