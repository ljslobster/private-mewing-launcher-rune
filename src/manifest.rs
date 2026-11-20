//! Global installed-asset manifest and outdated-asset planning.
//!
//! Mirrors `utils/asset_manifest.py`. The API identifies an asset by a stable
//! `id` and versions it by changing `file_name`/`file_url`; the payload has no
//! hashes, so an update is detected as a change of the advertised file name.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::data::ApiMod;
use crate::paths;

pub const SCHEMA_VERSION: u64 = 1;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetRecord {
    pub file_name: String,
    pub file_url: Option<String>,
    pub installed_at: Option<String>,
}

pub type KindRecords = BTreeMap<String, AssetRecord>;
pub type EventRecords = BTreeMap<String, KindRecords>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(default = "default_schema")]
    pub schema: u64,
    #[serde(default)]
    pub events: BTreeMap<String, EventRecords>,
}

fn default_schema() -> u64 {
    SCHEMA_VERSION
}

impl Default for Manifest {
    fn default() -> Self {
        Self { schema: SCHEMA_VERSION, events: BTreeMap::new() }
    }
}

// One lock covers the complete read-modify-write transaction across all
// download workers, including the shared temporary filename.
static MANIFEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn load_manifest() -> Manifest {
    load_manifest_at(&paths::manifest_path())
}

pub(crate) fn load_manifest_at(path: &Path) -> Manifest {
    let _guard = MANIFEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    load_manifest_unlocked(path)
}

fn load_manifest_unlocked(path: &Path) -> Manifest {
    let Ok(text) = fs::read_to_string(path) else { return Manifest::default() };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else { return Manifest::default() };
    if value.get("schema").and_then(|v| v.as_u64()) != Some(SCHEMA_VERSION) {
        return Manifest::default();
    }
    let Some(events) = value.get("events") else { return Manifest::default() };
    if !events.is_object() {
        return Manifest::default();
    }
    serde_json::from_value::<Manifest>(value).unwrap_or_default()
}

pub fn save_manifest(manifest: &Manifest) -> std::io::Result<()> {
    let _guard = MANIFEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    save_manifest_unlocked(manifest, &paths::manifest_path())
}

fn save_manifest_unlocked(manifest: &Manifest, path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let payload = serde_json::json!({
        "schema": SCHEMA_VERSION,
        "events": &manifest.events,
    });
    let text = serde_json::to_string_pretty(&payload)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)
}

pub fn record_installed(event_id: &str, kind: &str, asset_id: &str, file_name: &str, file_url: Option<&str>) {
    let _ = record_installed_at(&paths::manifest_path(), event_id, kind, asset_id, file_name, file_url);
}

pub(crate) fn record_installed_at(path: &Path, event_id: &str, kind: &str, asset_id: &str, file_name: &str, file_url: Option<&str>) -> std::io::Result<()> {
    if event_id.is_empty() {
        return Ok(());
    }
    let _guard = MANIFEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut manifest = load_manifest_unlocked(path);
    manifest
        .events
        .entry(event_id.to_string())
        .or_default()
        .entry(kind.to_string())
        .or_default()
        .insert(
            asset_id.to_string(),
            AssetRecord {
                file_name: file_name.to_string(),
                file_url: file_url.map(str::to_string),
                installed_at: Some(crate::time::now_iso8601()),
            },
        );
    save_manifest_unlocked(&manifest, path)
}

pub fn get_recorded_file_name(event_id: &str, kind: &str, asset_id: &str) -> Option<String> {
    let manifest = load_manifest();
    manifest
        .events
        .get(event_id)
        .and_then(|e| e.get(kind))
        .and_then(|k| k.get(asset_id))
        .map(|r| r.file_name.clone())
        .filter(|s| !s.is_empty())
}

fn default_schema_value() -> u64 {
    SCHEMA_VERSION
}

pub fn asset_id_for(asset: &ApiMod, index: usize) -> String {
    if !asset.id.is_empty() {
        asset.id.clone()
    } else if !asset.file_name.is_empty() {
        asset.file_name.clone()
    } else {
        format!("#{index}")
    }
}

const NOISE_TOKENS: &[&str] = &[
    "fabric", "forge", "neoforge", "quilt", "liteloader", "rift", "common", "universal", "client",
    "server", "mod", "mods", "mc", "minecraft", "java", "jar", "zip", "beta", "alpha", "rc",
    "release", "snapshot", "hotfix", "dev", "v", "ver", "version", "x",
];

/// Reduce a file name to a comparable identity token (heuristic for legacy installs).
pub fn normalize_asset_key(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    let lower = value.to_lowercase();
    let text = match lower.rfind('.') {
        Some(pos) if pos > 0 && lower.len() - pos <= 6 => &lower[..pos],
        _ => &lower[..],
    };
    let mut kept = String::new();
    for token in text.split(|c: char| !c.is_ascii_alphanumeric()) {
        if token.is_empty() {
            continue;
        }
        if NOISE_TOKENS.contains(&token) {
            continue;
        }
        if token.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        kept.push_str(token);
    }
    kept
}

#[derive(Clone, Debug)]
pub struct AssetUpdate {
    pub asset: ApiMod,
    pub asset_id: String,
    pub kind: String,
    pub new_file_name: String,
    pub stale_file_name: Option<String>,
    pub needs_download: bool,
    pub ambiguous: bool,
    pub migrated: bool,
}

/// Plan replacements for every mod and resource pack of an event.
pub fn plan_asset_updates(event_id: &str, kind: &str, assets: &[ApiMod], local_files: &[String]) -> Vec<AssetUpdate> {
    let manifest = load_manifest();
    let empty = KindRecords::new();
    let recorded = manifest
        .events
        .get(event_id)
        .and_then(|e| e.get(kind))
        .unwrap_or(&empty);

    let local_set: BTreeSet<&str> = local_files.iter().map(String::as_str).collect();
    let mut updates = Vec::new();
    let mut claimed: BTreeSet<String> = BTreeSet::new();

    for (index, asset) in assets.iter().enumerate() {
        if asset.file_name.is_empty() {
            continue;
        }
        let new_file_name = asset.file_name.clone();
        let asset_id = asset_id_for(asset, index);
        let needs_download = !local_set.contains(new_file_name.as_str());

        if let Some(record) = recorded.get(&asset_id) {
            let recorded_file = record.file_name.clone();
            let is_update = !recorded_file.is_empty() && recorded_file != new_file_name;
            let mut stale = None;
            let already_claimed = claimed.contains(&recorded_file);
            if is_update && local_set.contains(recorded_file.as_str()) && recorded_file != new_file_name {
                if !already_claimed {
                    stale = Some(recorded_file.clone());
                    claimed.insert(recorded_file.clone());
                }
            }
            if is_update {
                updates.push(AssetUpdate {
                    asset: asset.clone(),
                    asset_id,
                    kind: kind.to_string(),
                    new_file_name,
                    stale_file_name: stale.clone(),
                    needs_download,
                    ambiguous: stale.is_none() && already_claimed,
                    migrated: false,
                });
            }
            continue;
        }

        let new_key = normalize_asset_key(&new_file_name);
        if new_key.is_empty() {
            continue;
        }
        let candidates: Vec<&String> = local_files
            .iter()
            .filter(|name| **name != new_file_name && normalize_asset_key(name) == new_key)
            .collect();
        if candidates.is_empty() {
            continue;
        }
        let ambiguous = candidates.len() > 1 || candidates.iter().any(|name| claimed.contains(*name));
        let stale = if ambiguous { None } else { Some(candidates[0].clone()) };
        if let Some(name) = &stale {
            claimed.insert(name.clone());
        }
        updates.push(AssetUpdate {
            asset: asset.clone(),
            asset_id,
            kind: kind.to_string(),
            new_file_name,
            stale_file_name: stale,
            needs_download,
            ambiguous,
            migrated: true,
        });
    }
    updates
}

pub fn get_asset_update_plan(event: &crate::data::ApiEvent, game_dir: &Path) -> Vec<AssetUpdate> {
    let mut updates = Vec::new();
    if event.id.is_empty() {
        return updates;
    }
    for kind in ["mods", "resource_packs"] {
        let assets = match kind {
            "mods" => &event.mods,
            _ => &event.resource_packs,
        };
        if assets.is_empty() {
            continue;
        }
        let folder = paths::asset_folder(kind, game_dir);
        let local = paths::local_file_names(&folder);
        updates.extend(plan_asset_updates(&event.id, kind, assets, &local));
    }
    updates
}

pub fn get_update_button_label(updates: &[AssetUpdate]) -> Option<&'static str> {
    let mods = updates.iter().filter(|u| u.kind == "mods").count();
    let packs = updates.iter().filter(|u| u.kind == "resource_packs").count();
    match (mods > 0, packs > 0) {
        (true, false) => Some("Update mods"),
        (false, true) => Some("Update resource packs"),
        (true, true) => Some("Update mods & packs"),
        _ => None,
    }
}

#[allow(dead_code)]
fn _keep_default_schema() -> u64 {
    default_schema_value()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_records_preserve_every_worker_write() {
        let temp = paths::tests::TempDir::new();
        let path = temp.0.join("installed_assets.json");
        let barrier = std::sync::Barrier::new(24);
        std::thread::scope(|scope| {
            for worker in 0..24 {
                let path = &path;
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    for item in 0..4 {
                        let id = format!("{worker}-{item}");
                        record_installed_at(path, "test-event", "mods", &id, &format!("{id}.jar"), Some("test-url")).unwrap();
                    }
                });
            }
        });
        let manifest = load_manifest_at(&path);
        let records = &manifest.events["test-event"]["mods"];
        assert_eq!(records.len(), 96);
        for worker in 0..24 {
            for item in 0..4 {
                let id = format!("{worker}-{item}");
                assert_eq!(records[&id].file_name, format!("{id}.jar"));
                assert_eq!(records[&id].file_url.as_deref(), Some("test-url"));
            }
        }
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn normalization_matches_the_python_examples() {
        assert_eq!(normalize_asset_key("athena-fabric-1.21-4.0.3.jar"), "athena");
        assert_eq!(normalize_asset_key("athena-fabric-1.21.1-4.0.5.jar"), "athena");
        assert_eq!(normalize_asset_key("Cobblemon-fabric-1.8.4+1.21.1.jar"), "cobblemon");
    }
}
