//! Launcher root, per-variant instances and the one-time legacy migration.
//!
//! Mirrors `minecraft_event/constants.py` and `minecraft_event/instances.py`:
//! the Minecraft runtime and launcher settings stay shared in the root while
//! game data lives in `instances/<event-id>`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub const INSTANCES_DIR_NAME: &str = "instances";
pub const SETTINGS_DIR_NAME: &str = "mewing-launcher-settings";
pub const INSTANCE_STATE_FILENAME: &str = "instance-state.json";
pub const MIGRATION_MARKER_FILENAME: &str = "legacy-migration.json";

/// Root entries shared by every instance (or launcher internals).
pub const SHARED_ROOT_ENTRIES: &[&str] = &[
    INSTANCES_DIR_NAME,
    SETTINGS_DIR_NAME,
    "versions",
    "libraries",
    "assets",
    "runtime",
];

/// Game folders created eagerly so a fresh instance can be launched/inspected.
pub const GAME_FOLDERS: &[&str] = &["mods", "resourcepacks", "shaderpacks", "config"];

pub fn root() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join(".mewing-launcher");
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".mewing-launcher")
}

pub fn settings_dir() -> PathBuf {
    root().join(SETTINGS_DIR_NAME)
}

pub fn settings_path() -> PathBuf {
    settings_dir().join("settings.json")
}

pub fn manifest_path() -> PathBuf {
    settings_dir().join("installed_assets.json")
}

pub fn instances_dir() -> PathBuf {
    root().join(INSTANCES_DIR_NAME)
}

pub fn safe_instance_name(event_id: &str) -> String {
    let mut text = String::new();
    let mut last_dash = false;
    for ch in event_id.trim().chars() {
        let safe = ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-');
        if safe {
            text.push(ch);
            last_dash = ch == '-';
        } else if !last_dash {
            text.push('-');
            last_dash = true;
        }
    }
    let trimmed = text.trim_matches(|c| c == '.' || c == '-');
    if trimmed.is_empty() { "default".into() } else { trimmed.to_string() }
}

pub fn instance_dir(event_id: &str) -> PathBuf {
    instances_dir().join(safe_instance_name(event_id))
}

pub fn ensure_instance(event_id: &str) -> PathBuf {
    let dir = instance_dir(event_id);
    let _ = fs::create_dir_all(&dir);
    for folder in GAME_FOLDERS {
        let _ = fs::create_dir_all(dir.join(folder));
    }
    dir
}

pub fn instance_state_path(instance: &Path) -> PathBuf {
    instance.join(INSTANCE_STATE_FILENAME)
}

pub fn migration_marker_path() -> PathBuf {
    instances_dir().join(MIGRATION_MARKER_FILENAME)
}

pub fn asset_folder(kind: &str, game_dir: &Path) -> PathBuf {
    let folder = match kind {
        "mods" => "mods",
        "resource_packs" => "resourcepacks",
        "shader_packs" => "shaderpacks",
        other => other,
    };
    game_dir.join(folder)
}

/// API asset names must be a single portable filename, including on Windows.
pub fn safe_file_name(name: &str) -> bool {
    if name.is_empty()
        || name.ends_with(['.', ' '])
        || name.chars().any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
    {
        return false;
    }
    let stem = name.split('.').next().unwrap_or("").trim_end_matches(' ').to_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$") {
        return false;
    }
    if let Some(number) = stem.strip_prefix("COM").or_else(|| stem.strip_prefix("LPT")) {
        if matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³") {
            return false;
        }
    }
    true
}

// Resolve the nearest existing ancestor, so missing leaf directories are safe
// too. Dangling symlinks and inaccessible ancestors fail closed.
fn resolved_path(path: &Path) -> Option<PathBuf> {
    match fs::symlink_metadata(path) {
        Ok(_) => fs::canonicalize(path).ok(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Some(resolved_path(path.parent()?)?.join(path.file_name()?))
        }
        Err(_) => None,
    }
}

pub(crate) fn is_within_directory(directory: &Path, target: &Path) -> bool {
    let Ok(directory) = std::path::absolute(directory) else { return false };
    let Ok(target) = std::path::absolute(target) else { return false };
    match (resolved_path(&directory), resolved_path(&target)) {
        (Some(root), Some(target)) => target.starts_with(root),
        _ => false,
    }
}

pub(crate) fn safe_asset_target(game_dir: &Path, kind: &str, name: &str) -> Option<PathBuf> {
    if !safe_file_name(name) {
        return None;
    }
    let target = asset_folder(kind, game_dir).join(name);
    is_within_directory(game_dir, &target).then_some(target)
}

pub fn local_file_names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = match fs::read_dir(folder) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect(),
        Err(_) => Vec::new(),
    };
    names.sort();
    names
}

#[derive(Clone, Debug, Default)]
pub struct MigrationResult {
    pub event_id: String,
    pub instance_directory: PathBuf,
    pub marker_path: PathBuf,
    pub already_migrated: bool,
    pub moved: Vec<String>,
    pub merged: Vec<String>,
    pub conflicts: Vec<String>,
    pub shared: Vec<String>,
}

impl MigrationResult {
    pub fn changed_anything(&self) -> bool {
        !self.moved.is_empty() || !self.merged.is_empty()
    }
}

fn read_marker(path: &Path) -> Option<serde_json::Value> {
    let text = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value.is_object().then_some(value)
}

fn write_marker(path: &Path, payload: &serde_json::Value) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(payload).unwrap_or_default();
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)
}

/// Move a directory into an existing one without overwriting anything.
fn merge_move(source: &Path, destination: &Path, merged: &mut Vec<String>, conflicts: &mut Vec<String>, relative: &str) {
    let entries = match fs::read_dir(source) {
        Ok(entries) => entries.filter_map(|e| e.ok()).collect::<Vec<_>>(),
        Err(_) => return,
    };
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let child_source = source.join(&name);
        let child_destination = destination.join(&name);
        let child_relative = if relative.is_empty() { name.clone() } else { format!("{relative}/{name}") };
        if !child_destination.exists() {
            if fs::rename(&child_source, &child_destination).is_ok() {
                merged.push(child_relative);
            } else {
                conflicts.push(child_relative);
            }
            continue;
        }
        if child_source.is_dir() && child_destination.is_dir() {
            merge_move(&child_source, &child_destination, merged, conflicts, &child_relative);
            continue;
        }
        conflicts.push(child_relative);
    }
    if fs::read_dir(source).map(|mut d| d.next().is_none()).unwrap_or(false) {
        let _ = fs::remove_dir(source);
    }
}

/// Move legacy root game data into `instances/<event_id>` exactly once.
pub fn migrate_legacy_instance(event_id: &str) -> MigrationResult {
    let root_dir = root();
    let marker_path = migration_marker_path();
    let shared: BTreeSet<String> = SHARED_ROOT_ENTRIES.iter().map(|s| s.to_string()).collect();

    if let Some(previous) = read_marker(&marker_path) {
        let migrated_event = previous
            .get("event_id")
            .and_then(|v| v.as_str())
            .unwrap_or(event_id)
            .to_string();
        let instance = previous
            .get("instance_directory")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| instance_dir(&migrated_event));
        let list = |key: &str| {
            previous
                .get(key)
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default()
        };
        return MigrationResult {
            event_id: migrated_event,
            instance_directory: instance,
            marker_path,
            already_migrated: true,
            moved: list("moved"),
            merged: list("merged"),
            conflicts: list("conflicts"),
            shared: list("shared"),
        };
    }

    let instance = ensure_instance(event_id);
    let mut result = MigrationResult {
        event_id: event_id.to_string(),
        instance_directory: instance.clone(),
        marker_path: marker_path.clone(),
        shared: shared.iter().cloned().collect(),
        ..Default::default()
    };

    let entries = fs::read_dir(&root_dir)
        .map(|entries| entries.filter_map(|e| e.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if shared.contains(&name) {
            continue;
        }
        let source = root_dir.join(&name);
        let destination = instance.join(&name);
        if !destination.exists() {
            if fs::rename(&source, &destination).is_ok() {
                result.moved.push(name);
            } else {
                result.conflicts.push(name);
            }
            continue;
        }
        if source.is_dir() && destination.is_dir() {
            merge_move(&source, &destination, &mut result.merged, &mut result.conflicts, &name);
            continue;
        }
        result.conflicts.push(name);
    }

    let payload = serde_json::json!({
        "schema": 1,
        "event_id": event_id,
        "instance_directory": instance.to_string_lossy(),
        "migrated_at": crate::time::now_iso8601(),
        "shared": result.shared,
        "moved": result.moved,
        "merged": result.merged,
        "conflicts": result.conflicts,
        "policy": "Game data moved from the legacy root into the full event instance; runtime folders and launcher settings stay shared; existing destination files are never overwritten."
    });
    let _ = write_marker(&marker_path, &payload);
    result
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    pub(crate) struct TempDir(pub PathBuf);

    impl TempDir {
        pub(crate) fn new() -> Self {
            loop {
                let path = std::env::temp_dir().join(format!(
                    "mewing-backend-test-{}-{}",
                    std::process::id(), NEXT_DIR.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(e) => panic!("cannot create isolated test directory: {e}"),
                }
            }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    pub(crate) fn directory_link(source: &Path, link: &Path) {
        #[cfg(unix)]
        std::os::unix::fs::symlink(source, link).unwrap();
        #[cfg(windows)]
        {
            // Junctions exercise canonical containment without requiring the
            // Windows symlink privilege or changing process environment.
            let output = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link).arg(source).output().unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        }
    }

    #[test]
    fn asset_names_are_portable_single_components() {
        for name in ["", ".", "..", "../evil.jar", "nested/mod.jar", "..\\evil.jar", "C:\\evil.jar", "/evil.jar", "file.jar:stream", "CON", "nul.jar", "COM1.zip", "lpt9.jar", "COM¹.jar", "CON .txt", "CONOUT$", "mod.jar.", "mod.jar ", "bad\n.jar", "a?b.jar"] {
            assert!(!safe_file_name(name), "accepted {name:?}");
        }
        for name in ["mod.jar", "ATM x MSD [v5.0].zip", "COM10.jar", "cónfig.json", ".settings", "a+b-1.2.jar"] {
            assert!(safe_file_name(name), "rejected {name:?}");
        }
    }

    #[test]
    fn missing_children_cannot_escape_through_existing_directory_links() {
        let temp = TempDir::new();
        let game = temp.0.join("game");
        let outside = temp.0.join("outside");
        fs::create_dir_all(&game).unwrap();
        fs::create_dir_all(&outside).unwrap();
        directory_link(&outside, &game.join("config"));
        assert!(!is_within_directory(&game, &game.join("config/missing/default.json")));
        assert!(is_within_directory(&game, &game.join("new/missing/default.json")));
        assert!(safe_asset_target(&game, "mods", "../outside.jar").is_none());
    }
}
