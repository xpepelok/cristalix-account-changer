use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use crate::clients::list_clients;
use crate::host::Account;

const ANIMATION_KEYS: [&str; 17] = [
    "ofAnimatedWater",
    "ofAnimatedLava",
    "ofAnimatedFire",
    "ofAnimatedPortal",
    "ofAnimatedRedstone",
    "ofAnimatedExplosion",
    "ofAnimatedFlame",
    "ofAnimatedSmoke",
    "ofVoidParticles",
    "ofWaterParticles",
    "ofRainSplash",
    "ofPortalParticles",
    "ofPotionParticles",
    "ofFireworkParticles",
    "ofDrippingWaterLava",
    "ofAnimatedTerrain",
    "ofAnimatedTextures",
];

pub(crate) const MINIMAL_OPTIONS: [(&str, &str); 12] = [
    ("renderDistance", "2"),
    ("maxFps", "5"),
    ("particles", "2"),
    ("fancyGraphics", "false"),
    ("ao", "0"),
    ("renderClouds", "false"),
    ("mipmapLevels", "0"),
    ("entityShadows", "false"),
    ("enableVsync", "false"),
    ("biomeBlendRadius", "0"),
    ("fboEnable", "true"),
    ("useVbo", "true"),
];

const MINIMAL_OPTIONSOF: [(&str, &str); 27] = [
    ("ofFastRender", "true"),
    ("ofFastMath", "true"),
    ("ofClouds", "3"),
    ("ofTrees", "1"),
    ("ofDroppedItems", "0"),
    ("ofRain", "3"),
    ("ofBetterGrass", "1"),
    ("ofConnectedTextures", "1"),
    ("ofWeather", "false"),
    ("ofSky", "false"),
    ("ofStars", "false"),
    ("ofSunMoon", "false"),
    ("ofVignette", "1"),
    ("ofChunkUpdates", "1"),
    ("ofSmoothFps", "false"),
    ("ofFogType", "3"),
    ("ofDynamicLights", "0"),
    ("ofDynamicFov", "false"),
    ("ofNaturalTextures", "false"),
    ("ofEmissiveTextures", "false"),
    ("ofCustomSky", "false"),
    ("ofCustomColors", "false"),
    ("ofBetterSnow", "false"),
    ("ofClearWater", "false"),
    ("ofAaLevel", "0"),
    ("ofAfLevel", "1"),
    ("ofShowFps", "false"),
];

pub(crate) fn minimal_optionsof() -> BTreeMap<&'static str, &'static str> {
    MINIMAL_OPTIONSOF
        .iter()
        .copied()
        .chain(ANIMATION_KEYS.iter().map(|key| (*key, "0")))
        .collect()
}

fn update_options_file<K: AsRef<str>, V: AsRef<str>>(path: &Path, updates: &BTreeMap<K, V>) {
    if updates.is_empty() {
        return;
    }
    let mut lines: Vec<String> = match fs::read(path) {
        Ok(data) => String::from_utf8_lossy(&data)
            .replace("\r\n", "\n")
            .split('\n')
            .map(str::to_owned)
            .collect(),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(_) => return,
    };
    let lookup: BTreeMap<&str, &str> = updates
        .iter()
        .map(|(key, value)| (key.as_ref(), value.as_ref()))
        .collect();
    let mut seen = Vec::with_capacity(lookup.len());
    for line in &mut lines {
        let Some((key, _)) = line.split_once(':') else {
            continue;
        };
        if let Some((key, value)) = lookup.get_key_value(key) {
            *line = format!("{key}:{value}");
            seen.push(*key);
        }
    }
    lines.extend(
        lookup
            .iter()
            .filter(|(key, _)| !seen.contains(key))
            .map(|(key, value)| format!("{key}:{value}")),
    );
    let _ = fs::write(path, lines.join("\n"));
}

fn apply_minimal_options(client_dir: &Path) {
    let options: BTreeMap<&str, &str> = MINIMAL_OPTIONS.iter().copied().collect();
    update_options_file(&client_dir.join("options.txt"), &options);
    update_options_file(&client_dir.join("optionsof.txt"), &minimal_optionsof());
}

fn apply_client_options(client_dir: &Path, account: &Account) {
    let mut options = BTreeMap::new();
    if account.render_distance > 0 {
        options.insert("renderDistance", account.render_distance.to_string());
    }
    if account.max_fps > 0 {
        options.insert("maxFps", account.max_fps.to_string());
    }
    update_options_file(&client_dir.join("options.txt"), &options);

    let mut optionsof = BTreeMap::new();
    match account.fast_render {
        1 => {
            optionsof.insert("ofFastRender", "false");
        }
        2 => {
            optionsof.insert("ofFastRender", "true");
        }
        _ => {}
    }
    if account.animations != 0 {
        let value = if account.animations == 2 { "2" } else { "0" };
        optionsof.extend(ANIMATION_KEYS.iter().map(|key| (*key, value)));
    }
    update_options_file(&client_dir.join("optionsof.txt"), &optionsof);
}

pub(crate) fn apply_minimal_options_all(updates: &Path) {
    for client in list_clients(updates) {
        apply_minimal_options(&updates.join(client));
    }
}

pub(crate) fn apply_client_options_all(updates: &Path, account: &Account) {
    for client in list_clients(updates) {
        apply_client_options(&updates.join(client), account);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_keys_are_replaced_in_place_and_new_ones_appended() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("options.txt");
        fs::write(
            &path,
            "fov:70\r\nrenderDistance:12\r\nnoColon\r\nmaxFps:260",
        )
        .unwrap();
        let updates = BTreeMap::from([("renderDistance", "4"), ("maxFps", "60"), ("ao", "0")]);
        update_options_file(&path, &updates);
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "fov:70\nrenderDistance:4\nnoColon\nmaxFps:60\nao:0"
        );
    }

    #[test]
    fn empty_updates_leave_file_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("options.txt");
        update_options_file::<&str, &str>(&path, &BTreeMap::new());
        assert!(!path.exists());
    }

    #[test]
    fn client_options_follow_account_toggles() {
        let dir = tempfile::tempdir().unwrap();
        let account = Account {
            render_distance: 6,
            fast_render: 1,
            animations: 2,
            ..Account::default()
        };
        apply_client_options(dir.path(), &account);
        assert_eq!(
            fs::read_to_string(dir.path().join("options.txt")).unwrap(),
            "renderDistance:6"
        );
        let of = fs::read_to_string(dir.path().join("optionsof.txt")).unwrap();
        assert!(of.contains("ofFastRender:false"));
        assert!(of.contains("ofAnimatedWater:2"));
        assert_eq!(of.lines().count(), ANIMATION_KEYS.len() + 1);
    }

    #[test]
    fn minimal_options_cover_every_client() {
        let dir = tempfile::tempdir().unwrap();
        for client in ["A", "B"] {
            fs::create_dir_all(dir.path().join(client)).unwrap();
            fs::write(dir.path().join(client).join("minecraft.jar"), b"").unwrap();
        }
        apply_minimal_options_all(dir.path());
        for client in ["A", "B"] {
            let options = fs::read_to_string(dir.path().join(client).join("options.txt")).unwrap();
            assert!(options.contains("maxFps:5"));
            let of = fs::read_to_string(dir.path().join(client).join("optionsof.txt")).unwrap();
            assert!(of.contains("ofAnimatedTextures:0"));
        }
    }
}
