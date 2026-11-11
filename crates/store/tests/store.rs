use std::sync::Arc;

use ac_platform::{Paths, Secrets};
use ac_store::{
    Claims, GameLaunch, LaunchOpts, LauncherKind, LegacyPaths, PlayerCacheEntry, Store, Theme, jwt,
};
use tempfile::TempDir;

struct Fixture {
    _dir: TempDir,
    paths: Paths,
    secrets: Arc<Secrets>,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_dirs(dir.path(), dir.path().join("data"));
        paths.ensure().unwrap();
        let secrets = Arc::new(Secrets::new(&paths));
        Self {
            _dir: dir,
            paths,
            secrets,
        }
    }

    fn legacy(&self) -> LegacyPaths {
        LegacyPaths {
            vault: self.paths.vault.clone(),
            config: self.paths.config.clone(),
            players: self.paths.players.clone(),
            session: self.paths.session.clone(),
            logs: self.paths.logs.clone(),
        }
    }

    async fn open(&self) -> Store {
        Store::open(&self.paths.db, self.secrets.clone(), self.legacy())
            .await
            .unwrap()
    }
}

fn claims(uuid: &str, exp: i64) -> Claims {
    Claims {
        uuid: uuid.to_owned(),
        exp,
        ..Claims::default()
    }
}

fn token(uuid: &str, exp: i64) -> String {
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let payload = format!(r#"{{"uuid":"{uuid}","exp":{exp}}}"#);
    format!("h.{}.s", URL_SAFE_NO_PAD.encode(payload.as_bytes()))
}

#[tokio::test]
async fn upsert_and_list_sort() {
    let fx = Fixture::new();
    let store = fx.open().await;
    assert!(
        store
            .upsert_token("Steve", "t-steve", &claims("u-steve", 100))
            .await
            .unwrap()
    );
    assert!(
        store
            .upsert_token("Alex", "t-alex", &claims("u-alex", 100))
            .await
            .unwrap()
    );
    store.set_pinned("u-alex", true).await.unwrap();

    let list = store.list();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].uuid, "u-alex");
    assert_eq!(store.get("u-steve").unwrap().token, "t-steve");
}

#[tokio::test]
async fn upsert_never_replaces_with_earlier_expiry() {
    let fx = Fixture::new();
    let store = fx.open().await;
    store
        .upsert_token("Steve", "new", &claims("u", 200))
        .await
        .unwrap();
    let changed = store
        .upsert_token("Steve", "old", &claims("u", 100))
        .await
        .unwrap();
    assert!(!changed);
    assert_eq!(store.get("u").unwrap().token, "new");
    assert_eq!(store.get("u").unwrap().expires, 200);
}

#[tokio::test]
async fn forget_tombstones_and_blocks_reimport() {
    let fx = Fixture::new();
    let store = fx.open().await;
    store
        .upsert_token("Steve", "t", &claims("u", 100))
        .await
        .unwrap();
    let group = store.create_group("Team").await.unwrap().unwrap();
    store.add_group_member(&group.id, "u").await.unwrap();

    store.forget("u").await.unwrap();
    assert!(store.get("u").is_none());
    assert_eq!(store.group_members(&group.id).unwrap().0.len(), 0);

    let readded = store
        .upsert_token("Steve", "t", &claims("u", 100))
        .await
        .unwrap();
    assert!(
        !readded,
        "forgotten token with same value must not reappear"
    );
    let fresh = store
        .upsert_token("Steve", "t2", &claims("u", 100))
        .await
        .unwrap();
    assert!(fresh);
}

#[tokio::test]
async fn launch_settings_and_revision() {
    let fx = Fixture::new();
    let store = fx.open().await;
    store
        .upsert_token("Steve", "t", &claims("u", 100))
        .await
        .unwrap();
    let mut rev = store.subscribe();
    let before = *rev.borrow_and_update();

    let opts = LaunchOpts {
        ram: 4096,
        fullscreen: true,
        max_fps: 240,
        ..LaunchOpts::default()
    };
    store
        .set_launch_settings(&["u".to_owned()], opts)
        .await
        .unwrap();
    let account = store.get("u").unwrap();
    assert_eq!(account.ram, 4096);
    assert!(account.fullscreen);
    assert_eq!(account.max_fps, 240);
    assert!(rev.has_changed().unwrap());
    assert!(*rev.borrow() > before);
}

#[tokio::test]
async fn persistence_round_trip() {
    let fx = Fixture::new();
    {
        let store = fx.open().await;
        store
            .upsert_token("Steve", "secret", &claims("u", 100))
            .await
            .unwrap();
        store.set_label("u", "main").await.unwrap();
        let group = store.create_group("Team").await.unwrap().unwrap();
        store.add_group_member(&group.id, "u").await.unwrap();
    }
    let raw = std::fs::read(&fx.paths.db).unwrap();
    assert!(
        !raw.windows(6).any(|chunk| chunk == b"secret"),
        "token stored in plaintext"
    );

    let store = fx.open().await;
    let account = store.get("u").unwrap();
    assert_eq!(account.token, "secret");
    assert_eq!(account.label, "main");
    assert_eq!(store.sorted_groups().len(), 1);
    assert_eq!(store.sorted_groups()[0].members, vec!["u".to_owned()]);
}

#[tokio::test]
async fn settings_validation() {
    let fx = Fixture::new();
    let store = fx.open().await;
    assert_eq!(store.launcher(), LauncherKind::Jar);
    assert!(store.auto_play());
    assert!(!store.set_launcher(LauncherKind::Custom).await.unwrap());
    assert!(store.set_custom_launcher("/opt/launcher").await.unwrap());
    assert_eq!(store.launcher(), LauncherKind::Custom);

    store
        .set_background(Some("background.png".to_owned()))
        .await
        .unwrap();
    store.set_background_dim(200).await.unwrap();
    store.set_background_blur(99).await.unwrap();
    assert_eq!(store.background().as_deref(), Some("background.png"));
    assert_eq!(store.background_dim(), 100);
    assert_eq!(store.background_blur(), 40);

    assert_eq!(store.theme(), Theme::Dark);
    assert!(store.sound());
    store.set_theme(Theme::Light).await.unwrap();
    store.set_sound(false).await.unwrap();

    let reopened = fx.open().await;
    assert_eq!(reopened.background_dim(), 100);
    assert_eq!(reopened.launcher(), LauncherKind::Custom);
    assert_eq!(reopened.theme(), Theme::Light);
    assert!(!reopened.sound());
}

#[tokio::test]
async fn batched_log_lines_keep_order_and_cap() {
    let fx = Fixture::new();
    let store = fx.open().await;
    let session = store.begin_log_session("u", "Steve").await.unwrap();
    store.append_log_line(session, "first").await.unwrap();
    let batch: Vec<String> = (0..ac_store::MAX_LOG_LINES + 10)
        .map(|i| format!("batch {i}"))
        .collect();
    store.append_log_lines(session, &batch).await.unwrap();
    store.append_log_lines(session, &[]).await.unwrap();
    let lines = store.log_lines("u", Some(session)).await.unwrap().unwrap();
    assert_eq!(lines.len() as i64, ac_store::MAX_LOG_LINES);
    assert_eq!(lines[0].text, "batch 10");
    assert_eq!(
        lines.last().unwrap().text,
        format!("batch {}", ac_store::MAX_LOG_LINES + 9)
    );
}

#[tokio::test]
async fn player_cache_round_trip() {
    let fx = Fixture::new();
    let store = fx.open().await;
    store
        .upsert_player("Steve", r#"{"name":"Steve"}"#, 1234)
        .await
        .unwrap();
    let cache = store.load_player_cache().await.unwrap();
    assert_eq!(cache.len(), 1);
    assert_eq!(cache[0].name, "steve");
    assert_eq!(cache[0].fetched_at, 1234);
    store.prune_players(2000).await.unwrap();
    assert!(store.load_player_cache().await.unwrap().is_empty());
}

#[tokio::test]
async fn player_cache_batch_upsert_overwrites() {
    let fx = Fixture::new();
    let store = fx.open().await;
    store.upsert_player("steve", "{}", 1).await.unwrap();
    store
        .upsert_players(&[
            PlayerCacheEntry {
                name: "steve".to_owned(),
                info_json: r#"{"name":"Steve"}"#.to_owned(),
                fetched_at: 5,
            },
            PlayerCacheEntry {
                name: "alex".to_owned(),
                info_json: r#"{"name":"Alex"}"#.to_owned(),
                fetched_at: 6,
            },
        ])
        .await
        .unwrap();
    let mut cache = store.load_player_cache().await.unwrap();
    cache.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(cache.len(), 2);
    assert_eq!(cache[1].name, "steve");
    assert_eq!(cache[1].fetched_at, 5);
    assert_eq!(cache[1].info_json, r#"{"name":"Steve"}"#);
}

#[tokio::test]
async fn touch_online_and_bulk_settings_persist() {
    let fx = Fixture::new();
    {
        let store = fx.open().await;
        store
            .upsert_token("Steve", "t-steve", &claims("u-steve", 100))
            .await
            .unwrap();
        store
            .upsert_token("Alex", "t-alex", &claims("u-alex", 100))
            .await
            .unwrap();
        store
            .set_ram(&["u-steve".to_owned(), "u-alex".to_owned()], 2048)
            .await
            .unwrap();
        store.touch_online(&["u-steve".to_owned()]).await.unwrap();
    }
    let store = fx.open().await;
    let steve = store.get("u-steve").unwrap();
    assert!(steve.last_online > 0);
    assert_eq!(steve.token, "t-steve");
    assert_eq!(steve.ram, 2048);
    assert_eq!(store.get("u-alex").unwrap().ram, 2048);
    assert_eq!(store.get("u-alex").unwrap().last_online, 0);
}

#[tokio::test]
async fn game_launches_round_trip() {
    let fx = Fixture::new();
    let store = fx.open().await;
    let launches = vec![
        GameLaunch {
            uuid: "u1".to_owned(),
            launched_at: 10,
            pid: 4321,
            seen_at: Some(12),
            before_pids: vec![1, 2, 3],
        },
        GameLaunch {
            uuid: "u2".to_owned(),
            launched_at: 20,
            pid: 0,
            seen_at: None,
            before_pids: vec![],
        },
    ];
    store.save_launches(&launches).await.unwrap();
    assert_eq!(store.load_launches().await.unwrap(), launches);
}

#[tokio::test]
async fn log_sessions_and_caps() {
    let fx = Fixture::new();
    let store = fx.open().await;
    let session = store.begin_log_session("u", "Steve").await.unwrap();
    for i in 0..(ac_store::MAX_LOG_LINES + 50) {
        store
            .append_log_line(session, &format!("line {i}"))
            .await
            .unwrap();
    }
    let lines = store.log_lines("u", Some(session)).await.unwrap().unwrap();
    assert_eq!(lines.len() as i64, ac_store::MAX_LOG_LINES);
    assert_eq!(
        lines.last().unwrap().text,
        format!("line {}", ac_store::MAX_LOG_LINES + 49)
    );
    store.finish_log_session(session).await.unwrap();

    let summaries = store.log_summaries().await.unwrap();
    assert_eq!(summaries.len(), 1);
    assert!(!summaries[0].active);

    let stats = store.log_stats().await.unwrap();
    assert_eq!(stats.total_sessions, 1);
    assert_eq!(stats.total_lines, ac_store::MAX_LOG_LINES);

    store.clear_all_logs().await.unwrap();
    assert!(store.log_summaries().await.unwrap().is_empty());
}

#[tokio::test]
async fn migrates_legacy_vault_and_config() {
    let fx = Fixture::new();
    let data = format!(
        r#"{{"accounts":{{"u":{{"uuid":"u","name":"Steve","token":"{}","expires":100,"pinned":true}}}},"forgotten":{{}},"groups":[{{"id":"g1","name":"Team","members":["u"],"order":0}}]}}"#,
        token("u", 100)
    );
    let sealed = fx.secrets.encrypt(data.as_bytes()).unwrap();
    std::fs::write(&fx.paths.vault, &sealed).unwrap();
    std::fs::write(&fx.paths.config, br#"{"launcher":"new","autoPlay":false}"#).unwrap();

    let store = fx.open().await;
    let account = store.get("u").unwrap();
    assert_eq!(account.name, "Steve");
    assert_eq!(account.token, token("u", 100));
    assert!(account.pinned);
    assert_eq!(store.sorted_groups()[0].members, vec!["u".to_owned()]);
    assert_eq!(store.launcher(), LauncherKind::New);
    assert!(!store.auto_play());

    assert!(!fx.paths.vault.exists(), "legacy vault should be renamed");
    assert!(fx.paths.vault.with_extension("dat.migrated").exists());
    assert!(fx.paths.config.with_extension("json.migrated").exists());
}

#[tokio::test]
async fn merges_accounts_stored_without_dashes() {
    const DASHED: &str = "e376c8f4-72ec-11e9-8374-1cb72caa35fd";
    const BARE: &str = "e376c8f472ec11e983741cb72caa35fd";
    const ALONE: &str = "3e7839e2a26911edb93f1cb72cb014ae";
    let fx = Fixture::new();
    {
        let store = fx.open().await;
        store
            .upsert_token("Dreams", "older", &claims(DASHED, 100))
            .await
            .unwrap();
        store.set_label(DASHED, "main").await.unwrap();
        store
            .upsert_token("Dreams", "newer", &claims(BARE, 200))
            .await
            .unwrap();
        store.set_pinned(BARE, true).await.unwrap();
        store
            .upsert_token("Solo", "solo", &claims(ALONE, 100))
            .await
            .unwrap();
        let group = store.create_group("Team").await.unwrap().unwrap();
        store.add_group_member(&group.id, DASHED).await.unwrap();
        store.add_group_member(&group.id, BARE).await.unwrap();
        store.add_group_member(&group.id, ALONE).await.unwrap();
        store.begin_log_session(BARE, "Dreams").await.unwrap();
    }

    let store = fx.open().await;
    assert!(store.get(BARE).is_none());
    assert!(store.get(ALONE).is_none());
    let merged = store.get(DASHED).unwrap();
    assert_eq!(merged.token, "newer");
    assert_eq!(merged.expires, 200);
    assert_eq!(merged.label, "main");
    assert!(merged.pinned);
    let renamed = store.get("3e7839e2-a269-11ed-b93f-1cb72cb014ae").unwrap();
    assert_eq!(renamed.token, "solo");
    let mut members = store.sorted_groups()[0].members.clone();
    members.sort();
    assert_eq!(
        members,
        vec![
            "3e7839e2-a269-11ed-b93f-1cb72cb014ae".to_owned(),
            DASHED.to_owned()
        ]
    );
    let summaries = store.log_summaries().await.unwrap();
    assert!(summaries.iter().all(|summary| summary.uuid != BARE));
}

#[test]
fn jwt_uuid_is_canonical() {
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;

    let payload = r#"{"uuid":"E376C8F472EC11E983741CB72CAA35FD","exp":7}"#;
    let token = format!("h.{}.s", URL_SAFE_NO_PAD.encode(payload));
    assert_eq!(
        jwt::parse(&token).unwrap().uuid,
        "e376c8f4-72ec-11e9-8374-1cb72caa35fd"
    );
}

#[tokio::test]
async fn keeps_unreadable_legacy_vault() {
    let fx = Fixture::new();
    std::fs::write(&fx.paths.vault, b"not a sealed vault").unwrap();
    std::fs::write(&fx.paths.config, br#"{"launcher":"new"}"#).unwrap();

    let store = fx.open().await;
    assert!(store.get("u").is_none());
    assert!(
        fx.paths.vault.exists(),
        "unreadable vault must stay in place"
    );
    assert!(fx.paths.config.with_extension("json.migrated").exists());
}

#[test]
fn jwt_parses() {
    let parsed = jwt::parse(&token("abc", 7)).unwrap();
    assert_eq!(parsed.uuid, "abc");
    assert_eq!(parsed.exp, 7);
}
