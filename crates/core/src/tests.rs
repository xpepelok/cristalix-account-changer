use std::sync::Arc;

use ac_api::{Caps, LauncherKind, Os};
use ac_launcher::{Host, Presence};
use ac_platform::{Paths, Secrets};
use ac_player::PlayerService;
use ac_store::{LegacyPaths, Store};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use tokio::sync::mpsc;

use crate::convert::accounts;
use crate::core::launcher_allowed;
use crate::host::{StoreHost, is_staff_rank};

fn windows_caps() -> Caps {
    Caps {
        os: Os::Windows,
        credential_import: true,
        auto_play: true,
        exe_launchers: true,
        tray: true,
    }
}

fn linux_caps() -> Caps {
    Caps {
        os: Os::Linux,
        credential_import: false,
        auto_play: false,
        exe_launchers: false,
        tray: false,
    }
}

#[test]
fn exe_launchers_are_gated_by_platform() {
    for kind in LauncherKind::ALL {
        assert!(launcher_allowed(windows_caps(), kind));
    }
    assert!(launcher_allowed(linux_caps(), LauncherKind::Jar));
    assert!(launcher_allowed(linux_caps(), LauncherKind::Custom));
    assert!(!launcher_allowed(linux_caps(), LauncherKind::Normal));
    assert!(!launcher_allowed(linux_caps(), LauncherKind::New));
}

#[test]
fn staff_detection_ignores_plain_players() {
    assert!(is_staff_rank(Some("DEV")));
    assert!(!is_staff_rank(Some("PLAYER")));
    assert!(!is_staff_rank(Some("player")));
    assert!(!is_staff_rank(Some("")));
    assert!(!is_staff_rank(None));
}

#[test]
fn accounts_report_running_launching_and_expired() {
    let now = crate::convert::unix_now();
    let account = |uuid: &str, expires: i64| ac_store::Account {
        uuid: uuid.to_owned(),
        name: format!("name-{uuid}"),
        token: "tok".to_owned(),
        expires,
        ..ac_store::Account::default()
    };
    let list = vec![
        account("run", now + 1000),
        account("pending", now + 1000),
        account("stale", now - 10),
        ac_store::Account {
            uuid: "notoken".to_owned(),
            name: "ghost".to_owned(),
            ..ac_store::Account::default()
        },
    ];
    let mut presence = Presence::default();
    presence.running.insert("run".to_owned(), 42);
    presence.launching.insert("pending".to_owned());
    let mapped = accounts(list, &presence);
    assert_eq!(mapped.len(), 3);
    let run = mapped.iter().find(|a| a.uuid == "run").unwrap();
    assert!(run.running && !run.launching);
    let pending = mapped.iter().find(|a| a.uuid == "pending").unwrap();
    assert!(!pending.running && pending.launching);
    let stale = mapped.iter().find(|a| a.uuid == "stale").unwrap();
    assert!(stale.expired);
}

fn token(uuid: &str, exp: i64) -> String {
    let payload = format!(r#"{{"uuid":"{uuid}","exp":{exp}}}"#);
    format!("h.{}.s", URL_SAFE_NO_PAD.encode(payload.as_bytes()))
}

async fn open_store(dir: &std::path::Path) -> (Arc<Store>, Paths) {
    let paths = Paths::from_dirs(dir, dir.join("data"));
    paths.ensure().unwrap();
    let secrets = Arc::new(Secrets::new(&paths));
    let legacy = LegacyPaths {
        vault: paths.vault.clone(),
        config: paths.config.clone(),
        players: paths.players.clone(),
        session: paths.session.clone(),
        logs: paths.logs.clone(),
    };
    let store = Store::open(&paths.db, secrets, legacy).await.unwrap();
    (Arc::new(store), paths)
}

#[tokio::test]
async fn watcher_capture_upserts_token_from_launcher_config() {
    let dir = tempfile::tempdir().unwrap();
    let (store, paths) = open_store(dir.path()).await;
    let players = Arc::new(PlayerService::new().unwrap());
    let (signals, _receiver) = mpsc::unbounded_channel();
    let host = StoreHost {
        store: Arc::clone(&store),
        players,
        signals,
    };

    std::fs::create_dir_all(paths.launcher_cfg.parent().unwrap()).unwrap();
    let config = format!(
        r#"{{"accounts":{{"Steve":"{}"}},"currentAccount":"Steve"}}"#,
        token("uuid-steve", crate::convert::unix_now() + 10_000)
    );
    std::fs::write(&paths.launcher_cfg, config).unwrap();

    ac_launcher::capture_config(&host, &paths.launcher_cfg).await;

    let account = store.get("uuid-steve").expect("token captured");
    assert_eq!(account.name, "Steve");
    assert!(!account.token.is_empty());
    assert!(host.account("uuid-steve").is_some());
}
