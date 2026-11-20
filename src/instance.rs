//! Per-instance installation state and exact variant synchronisation.
//!
//! Mirrors `minecraft_event/instance_state.py` and `utils/instance_sync.py`:
//! every instance records which API assets it installed and which default
//! configs it applied, then a sync plan makes the instance match the event.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::data::{ApiConfigFile, ApiEvent, ApiMod};
use crate::download;
use crate::manifest::{self, AssetRecord};
use crate::paths;

pub const SCHEMA_VERSION: u64 = 1;
pub const KINDS: [&str; 3] = ["mods", "resource_packs", "shader_packs"];

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EventInfo {
    pub id: String,
    pub version: String,
    pub loader: String,
    pub fabric_version: Option<String>,
    pub forge_version: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigRecord {
    pub source_file_name: Option<String>,
    pub source_file_url: Option<String>,
    pub source_sha256: Option<String>,
    pub applied_sha256: Option<String>,
    pub applied_at: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigState {
    pub event_version: Option<String>,
    pub files: BTreeMap<String, ConfigRecord>,
    pub skipped_edited: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct InstanceState {
    pub schema: u64,
    pub event: EventInfo,
    pub assets: BTreeMap<String, BTreeMap<String, AssetRecord>>,
    pub configs: ConfigState,
}

pub fn empty_state() -> InstanceState {
    let mut assets = BTreeMap::new();
    for kind in KINDS {
        assets.insert(kind.to_string(), BTreeMap::new());
    }
    InstanceState {
        schema: SCHEMA_VERSION,
        event: EventInfo::default(),
        assets,
        configs: ConfigState::default(),
    }
}

pub fn load_state(instance_dir: &Path) -> InstanceState {
    let path = paths::instance_state_path(instance_dir);
    let Ok(text) = fs::read_to_string(path) else { return empty_state() };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else { return empty_state() };
    if value.get("schema").and_then(|v| v.as_u64()) != Some(SCHEMA_VERSION) {
        return empty_state();
    }
    let mut state: InstanceState = serde_json::from_value(value).unwrap_or_default();
    state.schema = SCHEMA_VERSION;
    for kind in KINDS {
        state.assets.entry(kind.to_string()).or_default();
    }
    state
}

pub fn save_state(state: &InstanceState, instance_dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(instance_dir)?;
    let path = paths::instance_state_path(instance_dir);
    let text = serde_json::to_string_pretty(state)?;
    let tmp = instance_dir.join(".instance-state.tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, &path)
}

pub fn update_event_info(state: &mut InstanceState, event: &ApiEvent) {
    state.event = EventInfo {
        id: event.id.clone(),
        version: event.version.clone(),
        loader: event.loader_family().to_string(),
        fabric_version: (!event.fabric_version.is_empty()).then(|| event.fabric_version.clone()),
        forge_version: (!event.forge_version.is_empty()).then(|| event.forge_version.clone()),
    };
}

pub fn record_asset(state: &mut InstanceState, kind: &str, asset_id: &str, file_name: &str, file_url: Option<&str>) {
    state.assets.entry(kind.to_string()).or_default().insert(
        asset_id.to_string(),
        AssetRecord {
            file_name: file_name.to_string(),
            file_url: file_url.map(str::to_string),
            installed_at: Some(crate::time::now_iso8601()),
        },
    );
}

pub fn get_asset_record<'a>(state: &'a InstanceState, kind: &str, asset_id: &str) -> Option<&'a AssetRecord> {
    state.assets.get(kind).and_then(|k| k.get(asset_id))
}

pub fn replace_assets(state: &mut InstanceState, kind: &str, records: BTreeMap<String, AssetRecord>) {
    state.assets.insert(kind.to_string(), records);
}

pub fn get_config_record<'a>(state: &'a InstanceState, target_path: &str) -> Option<&'a ConfigRecord> {
    state.configs.files.get(&key(target_path))
}

pub fn record_config_applied(
    state: &mut InstanceState,
    event_version: Option<&str>,
    target_path: &str,
    source_file_name: Option<&str>,
    source_file_url: Option<&str>,
    source_sha256: Option<&str>,
    applied_sha256: Option<&str>,
) {
    if let Some(version) = event_version {
        state.configs.event_version = Some(version.to_string());
    }
    state.configs.files.insert(
        key(target_path),
        ConfigRecord {
            source_file_name: source_file_name.map(str::to_string),
            source_file_url: source_file_url.map(str::to_string),
            source_sha256: source_sha256.map(str::to_string),
            applied_sha256: applied_sha256.map(str::to_string),
            applied_at: Some(crate::time::now_iso8601()),
        },
    );
    let key = key(target_path);
    state.configs.skipped_edited.retain(|item| item != &key);
}

pub fn mark_config_skipped(state: &mut InstanceState, event_version: Option<&str>, target_path: &str) {
    if let Some(version) = event_version {
        state.configs.event_version = Some(version.to_string());
    }
    let key = key(target_path);
    if !state.configs.skipped_edited.contains(&key) {
        state.configs.skipped_edited.push(key);
    }
}

fn key(target_path: &str) -> String {
    target_path.replace('\\', "/")
}

// Sync plan ---------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadReason {
    Missing,
    ChangedUrl,
}

#[derive(Clone, Debug)]
pub struct AssetDownload {
    pub kind: String,
    pub asset: ApiMod,
    pub asset_id: String,
    pub file_name: String,
    pub reason: DownloadReason,
}

impl AssetDownload {
    pub fn force(&self) -> bool {
        self.reason == DownloadReason::ChangedUrl
    }
}

#[derive(Clone, Debug)]
pub struct StaleFile {
    pub kind: String,
    pub file_name: String,
    pub path: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigReason {
    Missing,
    VersionChanged,
    SourceChanged,
}

#[derive(Clone, Debug)]
pub struct ConfigAction {
    pub config: ApiConfigFile,
    pub target_path: PathBuf,
    pub target_relative: String,
    pub reason: ConfigReason,
    pub current_sha256: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct SyncPlan {
    pub event_id: Option<String>,
    pub event_version: String,
    pub game_dir: PathBuf,
    pub downloads: Vec<AssetDownload>,
    pub removals: Vec<StaleFile>,
    pub config_actions: Vec<ConfigAction>,
    pub preserved_configs: Vec<String>,
    pub managed_kinds: Vec<String>,
}

impl SyncPlan {
    pub fn pending(&self) -> bool {
        !self.downloads.is_empty() || !self.removals.is_empty() || !self.config_actions.is_empty()
    }
    pub fn download_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for kind in KINDS {
            counts.insert(kind.to_string(), 0);
        }
        for item in &self.downloads {
            *counts.entry(item.kind.clone()).or_insert(0) += 1;
        }
        counts
    }
    pub fn removal_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for kind in KINDS {
            counts.insert(kind.to_string(), 0);
        }
        for item in &self.removals {
            *counts.entry(item.kind.clone()).or_insert(0) += 1;
        }
        counts
    }
}

#[derive(Clone, Debug, Default)]
pub struct SyncResult {
    pub downloaded: usize,
    pub failed: usize,
    pub removed: Vec<String>,
    pub configs_applied: usize,
    pub configs_preserved: usize,
    pub configs_failed: usize,
    pub skipped_kinds: Vec<String>,
}

impl SyncResult {
    pub fn ok(&self) -> bool {
        self.failed == 0 && self.configs_failed == 0
    }
}

fn assets_for(event: &ApiEvent, kind: &str) -> Option<Vec<ApiMod>> {
    let value = match kind {
        "mods" => Some(event.mods.clone()),
        "resource_packs" => Some(event.resource_packs.clone()),
        "shader_packs" => event.shader_packs.clone(),
        _ => None,
    }?;
    Some(value.into_iter().filter(|a| !a.file_name.is_empty()).collect())
}

fn list_files(folder: &Path) -> Vec<String> {
    paths::local_file_names(folder)
}

pub fn build_sync_plan(event: &ApiEvent, game_dir: &Path, state: &InstanceState) -> SyncPlan {
    build_sync_plan_at(event, game_dir, state, &paths::manifest_path())
}

fn build_sync_plan_at(event: &ApiEvent, game_dir: &Path, state: &InstanceState, manifest_path: &Path) -> SyncPlan {
    let mut plan = SyncPlan {
        event_id: (!event.id.is_empty()).then(|| event.id.clone()),
        event_version: event.version.clone(),
        game_dir: game_dir.to_path_buf(),
        ..Default::default()
    };

    let manifest = manifest::load_manifest_at(manifest_path);
    let empty = BTreeMap::new();
    let global_records = plan
        .event_id
        .as_ref()
        .and_then(|id| manifest.events.get(id))
        .unwrap_or(&empty);

    for kind in KINDS {
        let Some(assets) = assets_for(event, kind) else { continue };
        plan.managed_kinds.push(kind.to_string());

        let folder = paths::asset_folder(kind, game_dir);
        let expected: BTreeSet<String> = assets.iter().map(|a| a.file_name.clone()).collect();
        let recorded_kind = global_records.get(kind);

        let mut seen = BTreeSet::new();
        for asset in &assets {
            let file_name = asset.file_name.clone();
            if !seen.insert(file_name.clone()) {
                continue;
            }
            let asset_id = manifest::asset_id_for(asset, 0);
            let target = paths::safe_asset_target(game_dir, kind, &file_name);
            let record = get_asset_record(state, kind, &asset_id).or_else(|| {
                recorded_kind.and_then(|k| k.get(&asset_id))
            });

            if !target.as_ref().map(|p| p.exists()).unwrap_or(false) {
                plan.downloads.push(AssetDownload {
                    kind: kind.to_string(),
                    asset: asset.clone(),
                    asset_id,
                    file_name,
                    reason: DownloadReason::Missing,
                });
                continue;
            }

            let recorded_url = record.and_then(|r| r.file_url.as_deref()).unwrap_or("");
            let advertised_url = asset.file_url.as_str();
            if let Some(record) = record {
                if record.file_name == file_name
                    && !recorded_url.is_empty()
                    && !advertised_url.is_empty()
                    && recorded_url != advertised_url
                {
                    plan.downloads.push(AssetDownload {
                        kind: kind.to_string(),
                        asset: asset.clone(),
                        asset_id,
                        file_name,
                        reason: DownloadReason::ChangedUrl,
                    });
                }
            }
        }

        for local_name in list_files(&folder) {
            if !expected.contains(&local_name) {
                plan.removals.push(StaleFile {
                    kind: kind.to_string(),
                    file_name: local_name.clone(),
                    path: folder.join(&local_name),
                });
            }
        }
    }

    let (actions, preserved) = plan_config_actions(event, game_dir, state);
    plan.config_actions = actions;
    plan.preserved_configs = preserved;
    plan
}

/// Resolve a config `target_path` inside `game_dir`, rejecting escapes.
pub fn safe_config_target(game_dir: &Path, target_path: &str) -> Option<(String, PathBuf)> {
    let relative = target_path.trim().replace('\\', "/");
    if relative.is_empty() || relative.starts_with('/') || relative.contains(':') {
        return None;
    }
    let parts: Vec<&str> = relative
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    if parts.is_empty() || parts.iter().any(|part| !paths::safe_file_name(part)) {
        return None;
    }
    let normalised = parts.join("/");
    let target = game_dir.join(parts.iter().collect::<PathBuf>());
    paths::is_within_directory(game_dir, &target).then_some((normalised, target))
}

pub fn plan_config_actions(event: &ApiEvent, game_dir: &Path, state: &InstanceState) -> (Vec<ConfigAction>, Vec<String>) {
    let Some(configs) = event.config_files.as_ref() else { return (Vec::new(), Vec::new()) };
    let event_version = event.version.clone();
    let previous_version = state.configs.event_version.clone();
    let version_changed = previous_version.as_deref().map(|v| !v.is_empty() && v != event_version).unwrap_or(false);

    let mut actions = Vec::new();
    let mut preserved = Vec::new();
    let mut seen_targets = BTreeSet::new();

    for config in configs {
        let Some((relative, target_path)) = safe_config_target(game_dir, &config.target_path) else {
            continue;
        };
        if !seen_targets.insert(relative.clone()) {
            continue;
        }
        let record = get_config_record(state, &relative);
        let exists = target_path.is_file();
        let current_sha = if exists { crate::sha256::hex_file(&target_path) } else { None };

        if !exists {
            actions.push(ConfigAction {
                config: config.clone(),
                target_path,
                target_relative: relative,
                reason: ConfigReason::Missing,
                current_sha256: None,
            });
            continue;
        }

        let Some(record) = record else { continue };

        let applied_sha = record.applied_sha256.as_deref().unwrap_or("");
        if !applied_sha.is_empty() && current_sha.as_deref() != Some(applied_sha) {
            preserved.push(relative);
            continue;
        }

        if version_changed {
            actions.push(ConfigAction {
                config: config.clone(),
                target_path,
                target_relative: relative,
                reason: ConfigReason::VersionChanged,
                current_sha256: current_sha,
            });
            continue;
        }

        let recorded_url = record.source_file_url.as_deref().unwrap_or("");
        let advertised_url = config.file_url.as_str();
        if !recorded_url.is_empty() && !advertised_url.is_empty() && recorded_url != advertised_url {
            actions.push(ConfigAction {
                config: config.clone(),
                target_path,
                target_relative: relative,
                reason: ConfigReason::SourceChanged,
                current_sha256: current_sha,
            });
        }
    }
    (actions, preserved)
}

// Apply ------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct SyncCallbacks<'a> {
    pub asset: &'a (dyn Fn(&ApiMod, bool, &str) + Sync),
    pub file: &'a (dyn Fn(&ApiMod, u64, Option<u64>) + Sync),
    pub config: &'a (dyn Fn(&ApiConfigFile, Option<bool>, &str) + Sync),
}

impl<'a> Default for SyncCallbacks<'a> {
    fn default() -> Self {
        fn no_asset(_: &ApiMod, _: bool, _: &str) {}
        fn no_file(_: &ApiMod, _: u64, _: Option<u64>) {}
        fn no_config(_: &ApiConfigFile, _: Option<bool>, _: &str) {}
        Self {
            asset: &no_asset,
            file: &no_file,
            config: &no_config,
        }
    }
}

pub fn apply_event_sync(
    event: &ApiEvent,
    game_dir: &Path,
    plan: &SyncPlan,
    callbacks: SyncCallbacks<'_>,
) -> SyncResult {
    apply_event_sync_at(event, game_dir, plan, callbacks, &paths::manifest_path())
}

fn apply_event_sync_at(
    event: &ApiEvent,
    game_dir: &Path,
    plan: &SyncPlan,
    callbacks: SyncCallbacks<'_>,
    manifest_path: &Path,
) -> SyncResult {
    let _ = fs::create_dir_all(game_dir);
    let mut state = load_state(game_dir);
    let event_id = plan.event_id.clone().unwrap_or_else(|| event.id.clone());
    let mut result = SyncResult {
        skipped_kinds: KINDS
            .iter()
            .filter(|kind| !plan.managed_kinds.iter().any(|m| m == *kind))
            .map(|kind| kind.to_string())
            .collect(),
        ..Default::default()
    };

    let failed_assets = std::sync::Mutex::new(BTreeSet::new());
    for kind in KINDS {
        if !plan.managed_kinds.iter().any(|m| m == kind) {
            continue;
        }
        let folder = paths::asset_folder(kind, game_dir);
        let _ = fs::create_dir_all(&folder);

        let downloads: Vec<&AssetDownload> = plan.downloads.iter().filter(|d| d.kind == kind).collect();
        if !downloads.is_empty() {
            let force_names: BTreeSet<String> = downloads
                .iter()
                .filter(|d| d.force())
                .map(|d| d.file_name.clone())
                .collect();
            let assets: Vec<ApiMod> = downloads.iter().map(|d| d.asset.clone()).collect();
            let asset_cb = |asset: &ApiMod, ok: bool, status: &str| {
                if !ok {
                    failed_assets.lock().unwrap().insert((kind.to_string(), asset.file_name.clone()));
                }
                (callbacks.asset)(asset, ok, status);
            };
            let (successful, failed) = download::download_assets_at(
                &assets,
                kind,
                game_dir,
                &event_id,
                &force_names,
                &asset_cb,
                callbacks.file,
                manifest_path,
            );
            result.downloaded += successful;
            result.failed += failed;
        }

        let expected: BTreeSet<String> = assets_for(event, kind)
            .unwrap_or_default()
            .into_iter()
            .map(|a| a.file_name)
            .collect();
        let present: BTreeSet<String> = list_files(&folder).into_iter().collect();
        if expected.is_subset(&present) {
            for item in plan.removals.iter().filter(|r| r.kind == kind) {
                if !paths::is_within_directory(game_dir, &item.path) {
                    result.failed += 1;
                    continue;
                }
                match fs::remove_file(&item.path) {
                    Ok(()) => result.removed.push(item.path.to_string_lossy().to_string()),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => result.failed += 1,
                }
            }
        }
    }

    apply_config_actions(
        &plan.config_actions,
        game_dir,
        event,
        &mut state,
        &mut result,
        callbacks,
    );
    for relative in &plan.preserved_configs {
        mark_config_skipped(&mut state, Some(&plan.event_version), relative);
        result.configs_preserved += 1;
    }

    update_event_info(&mut state, event);
    record_instance_assets(event, game_dir, &mut state, &event_id, &failed_assets.into_inner().unwrap(), manifest_path);
    if save_state(&state, game_dir).is_err() {
        result.failed += 1;
    }
    result
}

fn apply_config_actions(
    actions: &[ConfigAction],
    instance_dir: &Path,
    event: &ApiEvent,
    state: &mut InstanceState,
    result: &mut SyncResult,
    callbacks: SyncCallbacks<'_>,
) {
    if actions.is_empty() {
        if event.config_files.is_some() {
            state.configs.event_version = Some(event.version.clone());
        }
        return;
    }

    let event_version = event.version.clone();
    let mut failures = 0usize;

    for action in actions {
        let config = &action.config;
        (callbacks.config)(config, None, "started");

        let temp_path = PathBuf::from(format!("{}.mewing-default.part", action.target_path.to_string_lossy()));
        let download_part = PathBuf::from(format!("{}.part", temp_path.to_string_lossy()));
        let safe = || {
            safe_config_target(instance_dir, &action.target_relative)
                .map(|(_, target)| target == action.target_path).unwrap_or(false)
                && paths::is_within_directory(instance_dir, &temp_path)
                && paths::is_within_directory(instance_dir, &download_part)
        };
        if !safe() {
            failures += 1;
            result.configs_failed += 1;
            (callbacks.config)(config, Some(false), "unsafe target");
            continue;
        }
        if let Some(parent) = action.target_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let ok = download::download_file(
            &config.file_url,
            &temp_path,
            &mut |downloaded, total| (callbacks.file)(&ApiMod::default(), downloaded, total),
            &mut |message, attempt, max| {
                (callbacks.config)(config, None, &format!("Retry {attempt}/{max}: {message}"));
            },
        );
        if !ok {
            failures += 1;
            result.configs_failed += 1;
            (callbacks.config)(config, Some(false), "download failed");
            continue;
        }

        if !safe() {
            failures += 1;
            result.configs_failed += 1;
            (callbacks.config)(config, Some(false), "unsafe target");
            continue;
        }
        let source_sha = crate::sha256::hex_file(&temp_path);
        let current_sha = if action.target_path.is_file() {
            crate::sha256::hex_file(&action.target_path)
        } else {
            None
        };

        // The file changed while the default was downloading: the user's edit
        // wins and the downloaded default is discarded.
        if current_sha != action.current_sha256
            || (action.current_sha256.is_none() && action.target_path.is_file())
        {
            let _ = fs::remove_file(&temp_path);
            result.configs_preserved += 1;
            mark_config_skipped(state, Some(&event_version), &action.target_relative);
            (callbacks.config)(config, Some(true), "preserved (edited by user)");
            continue;
        }

        let installed = if source_sha.is_none() {
            false
        } else if current_sha.is_some() && current_sha == source_sha {
            let _ = fs::remove_file(&temp_path);
            true
        } else {
            fs::rename(&temp_path, &action.target_path).is_ok()
        };
        if !installed {
            let _ = fs::remove_file(&temp_path);
            failures += 1;
            result.configs_failed += 1;
            (callbacks.config)(config, Some(false), "install failed");
            continue;
        }

        record_config_applied(
            state,
            Some(&event_version),
            &action.target_relative,
            Some(&config.file_name),
            Some(&config.file_url),
            source_sha.as_deref(),
            source_sha.as_deref(),
        );
        result.configs_applied += 1;
        (callbacks.config)(config, Some(true), "applied");
    }

    if failures == 0 {
        state.configs.event_version = Some(event_version);
    }
}

fn record_instance_assets(event: &ApiEvent, game_dir: &Path, state: &mut InstanceState, event_id: &str, failed_assets: &BTreeSet<(String, String)>, manifest_path: &Path) {
    let global = manifest::load_manifest_at(manifest_path);
    for kind in KINDS {
        let Some(assets) = assets_for(event, kind) else { continue };
        let mut records = BTreeMap::new();
        for asset in assets {
            if !paths::safe_asset_target(game_dir, kind, &asset.file_name).map(|p| p.is_file()).unwrap_or(false) {
                continue;
            }
            let asset_id = manifest::asset_id_for(&asset, 0);
            if failed_assets.contains(&(kind.to_string(), asset.file_name.clone())) {
                // An old file surviving a failed forced download is not the
                // advertised revision. Keep its old URL so the next sync retries.
                let previous = get_asset_record(state, kind, &asset_id).or_else(|| {
                    global.events.get(event_id).and_then(|e| e.get(kind)).and_then(|k| k.get(&asset_id))
                });
                if let Some(record) = previous.filter(|r| r.file_name == asset.file_name) {
                    records.insert(asset_id, record.clone());
                }
                continue;
            }
            record_asset(state, kind, &asset_id, &asset.file_name, Some(&asset.file_url));
            if let Some(record) = get_asset_record(state, kind, &asset_id) {
                records.insert(asset_id.clone(), record.clone());
            }
            if !event_id.is_empty() {
                let _ = manifest::record_installed_at(manifest_path, event_id, kind, &asset_id, &asset.file_name, Some(&asset.file_url));
            }
        }
        replace_assets(state, kind, records);
    }
}

pub fn get_sync_label(plan: &SyncPlan) -> Option<&'static str> {
    if !plan.pending() {
        return None;
    }
    let has_downloads = !plan.downloads.is_empty();
    let has_removals = !plan.removals.is_empty();
    let has_configs = !plan.config_actions.is_empty();

    if has_configs && !has_downloads && !has_removals {
        return Some("Apply configs");
    }
    if has_configs {
        return Some(if has_downloads { "Install files & configs" } else { "Sync files & configs" });
    }
    if has_downloads {
        return Some("Install files");
    }
    Some("Sync files")
}

#[cfg(test)]
mod tests {
    use super::*;
    use paths::tests::TempDir;

    fn config_event(url: String) -> ApiEvent {
        ApiEvent {
            id: "isolated-event".into(), version: "2".into(),
            config_files: Some(vec![ApiConfigFile {
                file_name: "default.json".into(), file_url: url,
                target_path: "config/default.json".into(),
            }]), ..Default::default()
        }
    }

    #[test]
    fn config_targets_reject_lexical_and_existing_link_escapes() {
        let temp = TempDir::new();
        let game = temp.0.join("game");
        let outside = temp.0.join("outside");
        fs::create_dir_all(&game).unwrap();
        fs::create_dir_all(&outside).unwrap();
        for target in ["../escape", "config/../../escape", "C:\\escape", "\\\\server\\share", "/escape", "config/file:ads", "config/NUL.json", "config/.. /escape"] {
            assert!(safe_config_target(&game, target).is_none(), "accepted {target}");
        }
        assert_eq!(safe_config_target(&game, "./config\\nested//default.json").unwrap().0, "config/nested/default.json");
        paths::tests::directory_link(&outside, &game.join("config"));
        assert!(safe_config_target(&game, "config/default.json").is_none());
        assert!(safe_config_target(&game, "config/missing/default.json").is_none());
        let event = config_event(String::new());
        assert!(plan_config_actions(&event, &game, &empty_state()).0.is_empty());
    }

    #[test]
    fn config_apply_rechecks_links_created_after_planning() {
        let temp = TempDir::new();
        let game = temp.0.join("game");
        let outside = temp.0.join("outside");
        fs::create_dir_all(&game).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let event = config_event(String::new());
        let mut state = empty_state();
        let (actions, _) = plan_config_actions(&event, &game, &state);
        assert_eq!(actions.len(), 1);
        paths::tests::directory_link(&outside, &game.join("config"));
        let mut result = SyncResult::default();
        apply_config_actions(&actions, &game, &event, &mut state, &mut result, SyncCallbacks::default());
        assert_eq!(result.configs_failed, 1);
        assert_eq!(result.configs_applied, 0);
        assert!(state.configs.files.is_empty());
        assert!(fs::read_dir(&outside).unwrap().next().is_none());
    }

    #[test]
    fn failed_config_rename_is_not_recorded_as_applied() {
        let temp = TempDir::new();
        let game = temp.0.join("game");
        fs::create_dir_all(game.join("config/default.json")).unwrap();
        let (url, server) = download::tests::serve_once(b"default contents");
        let event = config_event(url);
        let mut state = empty_state();
        let (actions, _) = plan_config_actions(&event, &game, &state);
        let mut result = SyncResult::default();
        let statuses = std::sync::Mutex::new(Vec::new());
        let config_cb = |_: &ApiConfigFile, ok, message: &str| { statuses.lock().unwrap().push((ok, message.to_string())); };
        apply_config_actions(&actions, &game, &event, &mut state, &mut result,
            SyncCallbacks { config: &config_cb, ..Default::default() });
        server.join().unwrap();
        assert_eq!(result.configs_failed, 1);
        assert_eq!(result.configs_applied, 0);
        assert!(!result.ok());
        assert!(state.configs.files.is_empty());
        assert!(state.configs.event_version.is_none());
        assert!(statuses.into_inner().unwrap().contains(&(Some(false), "install failed".into())));
        assert!(game.join("config/default.json").is_dir());
    }

    #[test]
    fn configs_created_or_edited_during_download_are_preserved() {
        for initially_exists in [false, true] {
            let temp = TempDir::new();
            let game = temp.0.join("game");
            let target = game.join("config/default.json");
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            let (url, server) = download::tests::serve_once(b"new default");
            let event = config_event(url);
            let mut state = empty_state();
            if initially_exists {
                fs::write(&target, b"old default").unwrap();
                let sha = crate::sha256::hex_file(&target).unwrap();
                record_config_applied(&mut state, Some("1"), "config/default.json", None, Some("old-url"), Some(&sha), Some(&sha));
            }
            let previous = get_config_record(&state, "config/default.json").map(|r| r.applied_sha256.clone());
            let (actions, _) = plan_config_actions(&event, &game, &state);
            assert_eq!(actions.len(), 1);
            let file_cb = |_: &ApiMod, _: u64, _: Option<u64>| { fs::write(&target, b"user config").unwrap(); };
            let mut result = SyncResult::default();
            apply_config_actions(&actions, &game, &event, &mut state, &mut result,
                SyncCallbacks { file: &file_cb, ..Default::default() });
            server.join().unwrap();
            assert_eq!(fs::read(&target).unwrap(), b"user config");
            assert_eq!(result.configs_preserved, 1);
            assert_eq!(result.configs_applied, 0);
            assert_eq!(result.configs_failed, 0);
            assert_eq!(get_config_record(&state, "config/default.json").map(|r| r.applied_sha256.clone()), previous);
            assert_eq!(state.configs.skipped_edited, ["config/default.json"]);
        }
    }

    #[test]
    fn missing_config_is_installed_and_hashed() {
        let temp = TempDir::new();
        let (url, server) = download::tests::serve_once(b"new default");
        let event = config_event(url.clone());
        let mut state = empty_state();
        let (actions, _) = plan_config_actions(&event, &temp.0, &state);
        let mut result = SyncResult::default();
        apply_config_actions(&actions, &temp.0, &event, &mut state, &mut result, SyncCallbacks::default());
        server.join().unwrap();
        assert_eq!(result.configs_applied, 1);
        assert!(result.ok());
        let record = get_config_record(&state, "config/default.json").unwrap();
        assert_eq!(record.source_file_url.as_deref(), Some(url.as_str()));
        assert_eq!(record.applied_sha256, crate::sha256::hex_file(&temp.0.join("config/default.json")));
    }

    #[test]
    fn stale_removal_and_state_save_errors_make_sync_fail() {
        for fail_save in [false, true] {
            let temp = TempDir::new();
            let game = temp.0.join("game");
            let manifest_path = temp.0.join("manifest.json");
            fs::create_dir_all(game.join("mods")).unwrap();
            let event = ApiEvent { id: "event".into(), ..Default::default() };
            if !fail_save {
                fs::write(game.join("mods/stale.jar"), b"stale").unwrap();
            }
            let plan = build_sync_plan_at(&event, &game, &empty_state(), &manifest_path);
            if fail_save {
                fs::create_dir(game.join(paths::INSTANCE_STATE_FILENAME)).unwrap();
            } else {
                // Simulate a stale file becoming an unremovable directory after planning.
                fs::remove_file(game.join("mods/stale.jar")).unwrap();
                fs::create_dir(game.join("mods/stale.jar")).unwrap();
            }
            let result = apply_event_sync_at(&event, &game, &plan, SyncCallbacks::default(), &manifest_path);
            assert!(!result.ok());
            assert_eq!(result.failed, 1);
            assert!(result.removed.is_empty());
        }
    }

    #[test]
    fn failed_changed_url_download_keeps_old_record_and_retries() {
        for global_only in [false, true] {
            let temp = TempDir::new();
            let game = temp.0.join("game");
            let manifest_path = temp.0.join("manifest.json");
            fs::create_dir_all(game.join("mods")).unwrap();
            fs::write(game.join("mods/mod.jar"), b"old version").unwrap();
            let event = ApiEvent { id: "event".into(), mods: vec![ApiMod {
                id: "asset".into(), file_name: "mod.jar".into(), file_url: "invalid://new-url".into(), ..Default::default()
            }], ..Default::default() };
            let mut state = empty_state();
            if global_only {
                manifest::record_installed_at(&manifest_path, "event", "mods", "asset", "mod.jar", Some("old-url")).unwrap();
            } else {
                record_asset(&mut state, "mods", "asset", "mod.jar", Some("old-url"));
            }
            save_state(&state, &game).unwrap();
            let plan = build_sync_plan_at(&event, &game, &state, &manifest_path);
            assert_eq!(plan.downloads.len(), 1);
            assert!(plan.downloads[0].force());
            let result = apply_event_sync_at(&event, &game, &plan, SyncCallbacks::default(), &manifest_path);
            assert_eq!(result.failed, 1);
            assert!(!result.ok());
            assert_eq!(fs::read(game.join("mods/mod.jar")).unwrap(), b"old version");
            let state = load_state(&game);
            assert_eq!(get_asset_record(&state, "mods", "asset").unwrap().file_url.as_deref(), Some("old-url"));
            let global = manifest::load_manifest_at(&manifest_path);
            if global_only {
                assert_eq!(global.events["event"]["mods"]["asset"].file_url.as_deref(), Some("old-url"));
            } else {
                assert!(global.events.is_empty());
            }
            let retry = build_sync_plan_at(&event, &game, &state, &manifest_path);
            assert_eq!(retry.downloads.len(), 1);
            assert!(retry.downloads[0].force());
        }
    }

    #[test]
    fn successful_changed_url_download_records_new_url() {
        let temp = TempDir::new();
        let game = temp.0.join("game");
        let manifest_path = temp.0.join("manifest.json");
        fs::create_dir_all(game.join("mods")).unwrap();
        fs::write(game.join("mods/mod.jar"), b"old version").unwrap();
        let (url, server) = download::tests::serve_once(b"new version");
        let event = ApiEvent { id: "event".into(), mods: vec![ApiMod {
            id: "asset".into(), file_name: "mod.jar".into(), file_url: url.clone(), ..Default::default()
        }], ..Default::default() };
        let mut state = empty_state();
        record_asset(&mut state, "mods", "asset", "mod.jar", Some("old-url"));
        save_state(&state, &game).unwrap();
        let plan = build_sync_plan_at(&event, &game, &state, &manifest_path);
        let result = apply_event_sync_at(&event, &game, &plan, SyncCallbacks::default(), &manifest_path);
        server.join().unwrap();
        assert!(result.ok());
        assert_eq!(result.downloaded, 1);
        assert_eq!(fs::read(game.join("mods/mod.jar")).unwrap(), b"new version");
        let state = load_state(&game);
        assert_eq!(get_asset_record(&state, "mods", "asset").unwrap().file_url.as_deref(), Some(url.as_str()));
        assert_eq!(manifest::load_manifest_at(&manifest_path).events["event"]["mods"]["asset"].file_url.as_deref(), Some(url.as_str()));
        assert!(!build_sync_plan_at(&event, &game, &state, &manifest_path).pending());
    }

    #[test]
    fn optional_shader_and_failed_download_cleanup_semantics_are_preserved() {
        let temp = TempDir::new();
        let game = temp.0.join("game");
        let manifest_path = temp.0.join("manifest.json");
        fs::create_dir_all(game.join("shaderpacks")).unwrap();
        fs::create_dir_all(game.join("mods")).unwrap();
        fs::write(game.join("shaderpacks/user.zip"), b"shader").unwrap();
        fs::write(game.join("mods/stale.jar"), b"old asset").unwrap();
        let mut event = ApiEvent { id: "event".into(), mods: vec![ApiMod { file_name: "missing.jar".into(), ..Default::default() }], ..Default::default() };
        let plan = build_sync_plan_at(&event, &game, &empty_state(), &manifest_path);
        assert!(!plan.managed_kinds.contains(&"shader_packs".into()));
        assert!(!plan.managed_kinds.contains(&"data_packs".into()));
        let result = apply_event_sync_at(&event, &game, &plan, SyncCallbacks::default(), &manifest_path);
        assert_eq!(result.failed, 1);
        assert!(game.join("mods/stale.jar").exists());
        assert!(game.join("shaderpacks/user.zip").exists());
        event.mods.clear();
        event.shader_packs = Some(Vec::new());
        let plan = build_sync_plan_at(&event, &game, &load_state(&game), &manifest_path);
        let result = apply_event_sync_at(&event, &game, &plan, SyncCallbacks::default(), &manifest_path);
        assert!(result.ok());
        assert_eq!(result.removed.len(), 2);
        assert!(!game.join("mods/stale.jar").exists());
        assert!(!game.join("shaderpacks/user.zip").exists());
    }
}
