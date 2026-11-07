use crate::{
    Account, ClientToggle, ImportItem, ImportProgress, ImportStatus, PlayerInfo, Presence, Rank,
    UpdateProgress, msg, normalize_label, snap_ram,
};

#[test]
fn ram_snaps_to_step_and_bounds() {
    assert_eq!(snap_ram(0), 1024);
    assert_eq!(snap_ram(2300), 2048);
    assert_eq!(snap_ram(2400), 2560);
    assert_eq!(snap_ram(99_999), 32768);
}

#[test]
fn labels_keep_eight_chars() {
    assert_eq!(normalize_label("  ПрофильДлинный "), "ПрофильД");
    assert_eq!(normalize_label("main"), "main");
}

#[test]
fn account_round_trips_go_json() {
    let json = r#"{"uuid":"0123456789abcdef","name":"xpepelok","launchable":true,"expired":false,
        "expires":10,"lastLaunched":5,"lastOnline":4,"running":false,"launching":true,"label":"main",
        "pinned":true,"client":"","profile":"","ram":4096,"minGraphics":true,"fullscreen":false,
        "discordRPC":true,"autoEnter":false,"debugMode":false,"renderDistance":8,"maxFps":120,
        "animations":2,"fastRender":1,"firstSeen":3}"#;
    let account: Account = serde_json::from_str(json).expect("go account json");
    assert_eq!(account.launch.ram, 4096);
    assert!(account.launch.discord_rpc);
    assert_eq!(account.launch.animations, ClientToggle::On);
    assert_eq!(account.launch.fast_render, ClientToggle::Off);
    assert_eq!(account.short_id(), "01234567");
}

#[test]
fn ranks_follow_staff_then_donate() {
    let staff = PlayerInfo {
        staff: "DEV".into(),
        color: "#3b82e8".into(),
        donate: "VIP".into(),
        ..PlayerInfo::default()
    };
    assert_eq!(
        staff.primary_rank(),
        Some(Rank {
            label: "DEV".into(),
            color: "#3b82e8".into()
        })
    );
    assert_eq!(staff.ranks().len(), 2);
    let player = PlayerInfo {
        staff: "PLAYER".into(),
        donate: "NO".into(),
        ..PlayerInfo::default()
    };
    assert_eq!(player.primary_rank(), None);
    assert_eq!(Rank::player().label, "PLAYER");
}

#[test]
fn unknown_presence_parses() {
    let info: PlayerInfo = serde_json::from_str(r#"{"online":"away"}"#).expect("player json");
    assert_eq!(info.online, Presence::Unknown);
}

#[test]
fn import_progress_counts() {
    let item = |status| ImportItem {
        login: "a".into(),
        status,
        text: String::new(),
        name: String::new(),
    };
    let progress = ImportProgress::from_items(
        true,
        vec![
            item(ImportStatus::Ok),
            item(ImportStatus::Err),
            item(ImportStatus::Pending),
            item(ImportStatus::Canceled),
        ],
    );
    assert_eq!((progress.total, progress.done), (4, 3));
    assert_eq!((progress.ok, progress.fail, progress.skip), (1, 1, 0));
    assert!(progress.has_results());
}

#[test]
fn update_percent_is_bounded() {
    let progress = UpdateProgress {
        downloaded: 50,
        total: 200,
        ..UpdateProgress::default()
    };
    assert_eq!(progress.percent(), 25);
    assert_eq!(UpdateProgress::default().percent(), 0);
}

#[test]
fn import_nothing_messages_match_go() {
    assert_eq!(msg::import_nothing(0, 0), "список пуст");
    assert_eq!(msg::import_nothing(0, 2), "в списке только дубликаты");
    assert_eq!(msg::import_nothing(3, 0), "все аккаунты уже сохранены (3)");
    assert_eq!(
        msg::import_nothing(3, 1),
        "нечего импортировать: уже сохранено 3, дублей 1"
    );
}
