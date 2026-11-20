//! Live event payload from the launcher API, plus the local settings file.
//! The Flet original reads the same endpoints in `utils/launcher.py`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const APP_NAME: &str = "Private Mewing Launcher";
pub const APP_VERSION: &str = "2.0.1";
pub const EVENTS_URL: &str = "https://private-mewing-launcher-api.vercel.app/events";
pub const LAUNCHER_LATEST_URL: &str = "https://private-mewing-launcher-api.vercel.app/launcher/latest";

/// Latest Windows installer published on GitHub Releases, relayed by the API.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct LauncherRelease {
    pub version: String,
    pub url: String,
    pub sha256: String,
}

pub fn parse_version(value: &str) -> Option<(u64, u64, u64)> {
    let mut parts = value.trim().trim_start_matches(['v', 'V']).split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0")
        .split(|c: char| !c.is_ascii_digit()).next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

pub fn fetch_launcher_release() -> Option<LauncherRelease> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build().ok()?;
    let release: LauncherRelease = client.get(LAUNCHER_LATEST_URL)
        .send().ok()?.error_for_status().ok()?.json().ok()?;
    (!release.version.is_empty() && !release.url.is_empty()).then_some(release)
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ApiMod {
    pub id: String,
    pub file_name: String,
    #[serde(default)]
    pub file_url: String,
    #[serde(default)]
    pub thumbnail: String,
}

/// Optional default config shipped by the API (`config_files`).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ApiConfigFile {
    pub file_name: String,
    pub file_url: String,
    pub target_path: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ApiEvent {
    pub id: String,
    pub title: String,
    pub description: String,
    pub version: String,
    pub is_fabric: bool,
    pub fabric_version: String,
    pub forge_version: String,
    pub current_event: bool,
    pub background_image: String,
    pub server_ip: String,
    pub mods: Vec<ApiMod>,
    pub resource_packs: Vec<ApiMod>,
    pub data_packs: Vec<ApiMod>,
    /// `None` when the payload omits the key (leave the folder alone);
    /// `Some([])` is authoritative and clears it.
    pub shader_packs: Option<Vec<ApiMod>>,
    /// `None` when the payload omits the key.
    pub config_files: Option<Vec<ApiConfigFile>>,
}

impl ApiEvent {
    pub fn loader_version(&self) -> &str {
        if self.is_fabric { &self.fabric_version } else { &self.forge_version }
    }
    pub fn loader_family(&self) -> &'static str {
        if self.is_fabric { "fabric" } else { "forge" }
    }
    pub fn shader_packs_list(&self) -> &[ApiMod] {
        self.shader_packs.as_deref().unwrap_or(&[])
    }
    pub fn config_files_list(&self) -> &[ApiConfigFile] {
        self.config_files.as_deref().unwrap_or(&[])
    }
    pub fn family_key(&self) -> String {
        if self.id.is_empty() { family_key(&self.title) } else { family_key(&self.id) }
    }
}

/// Variant-independent identity of a title or id, like `variants.family_key`.
pub fn family_key(value: &str) -> String {
    let mut text = value.trim().to_lowercase();
    for token in ["lite", "optimizado", "optimized"] {
        text = text.replace(token, " ");
    }
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    text.trim_matches(|c: char| " -_:/()[]".contains(c) || c == ' ').to_string()
}

/// True for the LITE/optimized variant, derived from the payload only.
pub fn is_lite_variant(event: &ApiEvent) -> bool {
    let haystack = format!("{} {} {}", event.id, event.title, event.version).to_lowercase();
    ["lite", "optimizado", "optimized"].iter().any(|token| haystack.contains(token))
}

/// The selected pack plus its sibling builds (its LITE variant).
pub fn variant_family<'a>(events: &'a [ApiEvent], selected: &'a ApiEvent) -> Vec<&'a ApiEvent> {
    let events: Vec<&ApiEvent> = events.iter().filter(|e| !e.id.is_empty()).collect();
    if events.is_empty() {
        return vec![selected];
    }
    let selected_id = selected.id.as_str();
    let keys: Vec<String> = [family_key(&selected.id), family_key(&selected.title)]
        .into_iter()
        .filter(|k| !k.is_empty())
        .collect();
    if keys.is_empty() {
        return vec![selected];
    }
    let matches = |event: &ApiEvent| -> bool {
        if !selected_id.is_empty() && event.id == selected_id {
            return true;
        }
        let candidates = [family_key(&event.id), family_key(&event.title)];
        keys.iter().any(|base| {
            candidates.iter().any(|candidate| {
                candidate == base
                    || (candidate.starts_with(base.as_str())
                        && candidate[base.len()..].starts_with(|c: char| " -_:/()[]".contains(c)))
            })
        })
    };
    let mut family: Vec<&ApiEvent> = events.iter().copied().filter(|e| matches(e)).collect();
    if family.is_empty() {
        return events;
    }
    family.sort_by_key(|event| if event.id == selected_id { 0 } else { 1 });
    family
}

/// Choose the saved variant when it still exists, else the current event.
pub fn choose_event<'a>(events: &'a [ApiEvent], saved_id: Option<&str>) -> Option<&'a ApiEvent> {
    let usable: Vec<&ApiEvent> = events.iter().filter(|e| !e.id.is_empty()).collect();
    if usable.is_empty() {
        return None;
    }
    if let Some(saved) = saved_id {
        if let Some(found) = usable.iter().find(|e| e.id == saved) {
            return Some(found);
        }
    }
    usable
        .iter()
        .find(|e| e.current_event)
        .copied()
        .or_else(|| usable.first().copied())
}

/// Payload used when the API is unreachable: the snapshot behind the reference screenshot.
pub fn fallback_event() -> ApiEvent {
    let names = [
        ("Cobbreeding", "Cobbreeding-fabric-2.4.1.jar"),
        ("AppleSkin", "appleskin-fabric-mc1.21-3.0.6.jar"),
        ("Athena", "athena-fabric-1.21.1-4.0.6.jar"),
        ("Melody", "melody_fabric_1.0.10_MC_1.21.jar"),
        ("Fzzy Config", "fzzy_config-0.7.7+1.21.jar"),
        ("Cloth Config API", "cloth-config-15.0.140-fabric.jar"),
    ];
    ApiEvent {
        id: "cobblepass-5".into(),
        title: "CobblePass 5".into(),
        description: "cp5".into(),
        version: "1.0.0".into(),
        is_fabric: true,
        fabric_version: "1.21.1".into(),
        current_event: true,
        mods: names.iter().map(|(id, file)| ApiMod {
            id: (*id).into(), file_name: (*file).into(), file_url: String::new(), thumbnail: String::new(),
        }).collect(),
        resource_packs: vec![ApiMod {
            id: "ATM x MSD [v5.0].zip".into(),
            file_name: "ATM x MSD [v5.0].zip".into(),
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// `GET /events/` — the selector is fed by this list, never hardcoded.
pub fn fetch_events() -> Option<Vec<ApiEvent>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build().ok()?;
    let list: Vec<ApiEvent> = client.get(format!("{EVENTS_URL}/")).send().ok()?.json().ok()?;
    (!list.is_empty()).then_some(list)
}

/// `GET /events/<id>`, or `currentEvent` when `id` is `None`.
pub fn fetch_event(id: Option<&str>) -> Option<ApiEvent> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build().ok()?;
    let mut url = reqwest::Url::parse(EVENTS_URL).ok()?;
    url.path_segments_mut().ok()?.push(id.unwrap_or("currentEvent"));
    let payload: serde_json::Value = client.get(url).send().ok()?.error_for_status().ok()?.json().ok()?;
    if !payload.get("mods")?.is_array() || !payload.get("resource_packs")?.is_array() {
        return None;
    }
    let event: ApiEvent = serde_json::from_value(payload).ok()?;
    (!event.id.is_empty() && !event.title.is_empty()).then_some(event)
}

pub fn fetch_bytes(url: &str) -> Option<Vec<u8>> {
    if url.is_empty() { return None; }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build().ok()?;
    let response = client.get(url).send().ok()?;
    if !response.status().is_success() { return None; }
    Some(response.bytes().ok()?.to_vec())
}

// Local settings, at the same path the Flet launcher uses -----------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
#[allow(non_snake_case)] // keys are the exact ones used by settings.json
pub struct SettingsFile {
    pub username: Option<String>,
    pub maxRam: Option<String>,
    pub minRam: Option<String>,
    pub hideLauncherOnStart: Option<bool>,
    pub selectedEventId: Option<String>,
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

pub fn settings_path() -> PathBuf {
    let appdata = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs_fallback());
    appdata.join(".mewing-launcher").join("mewing-launcher-settings").join("settings.json")
}

fn dirs_fallback() -> PathBuf {
    std::env::var("USERPROFILE").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."))
}

pub fn load_settings() -> SettingsFile {
    let path = settings_path();
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_settings(settings: &SettingsFile) {
    if let Some(parent) = settings_path().parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(settings_path(), json);
    }
}

pub const RAM_OPTIONS: &[&str] = &[
    "512MB", "1024MB", "1536MB", "2048MB", "2560MB", "3072MB", "3584MB", "4096MB",
    "5120MB", "6144MB", "7168MB", "8192MB", "10240MB", "12288MB", "16384MB",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_compared_numerically() {
        assert_eq!(parse_version("v2.0.0"), Some((2, 0, 0)));
        assert_eq!(parse_version("2.10"), Some((2, 10, 0)));
        assert!(parse_version("2.0.10") > parse_version("2.0.9"));
        assert!(parse_version("2.0.0") > parse_version("1.99.99"));
        assert_eq!(parse_version("latest"), None);
    }
}
