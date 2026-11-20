use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use rune::{AppHandle, Image, ImageCache, TextEdit};

use crate::data::{ApiEvent, ApiMod, SettingsFile};
use crate::{data, instance, manifest, minecraft, paths};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Route { Home, Settings }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab { Mods, Shaders, Resources }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModalKind { Logs, Downloads, Variant, ConfirmSync }

#[derive(Clone, Debug, Default)]
pub struct DownloadItem {
    pub received: u64,
    pub total: Option<u64>,
    pub done: Option<bool>,
    pub status: String,
}

pub struct AppState {
    pub route: Route,
    pub tab: Tab,
    pub search: TextEdit,
    pub username: TextEdit,
    pub launching: bool,
    pub busy: bool,
    pub loading: bool,
    pub api_available: bool,
    pub status: String,
    pub modal: Option<ModalKind>,
    pub max_ram: usize,
    pub min_ram: usize,
    pub hide_on_start: bool,
    pub ram_menu: Option<bool>,
    pub hero: Option<Image>,
    pub hero_hover: bool,
    pub hero_cursor: Option<[f32; 2]>,
    pub icon: Option<Image>,
    pub cache: Option<ImageCache>,
    pub logs: Vec<String>,
    pub log_autoscroll: bool,
    pub downloads: BTreeMap<String, DownloadItem>,
    pub event: ApiEvent,
    pub events: Vec<ApiEvent>,
    pub thumbnails: Vec<Option<Image>>,
    pub resource_thumbnails: Vec<Option<Image>>,
    pub server_online: bool,
    pub settings: SettingsFile,
    pub viewport: [f32; 2],
    pub handle: Option<AppHandle<AppState>>,
    pub stop: Option<Sender<()>>,
    pub sync_plan: instance::SyncPlan,
    pub updates: Vec<manifest::AssetUpdate>,
    pub loader_installed: bool,
    pub legacy_files: Vec<String>,
    pub presence: Option<crate::discord::Presence>,
    pub update: Option<data::LauncherRelease>,
    pub updating: bool,
}

impl Default for AppState {
    fn default() -> Self {
        let settings = data::load_settings();
        let username = settings.username.clone().unwrap_or_else(|| "PlayerOne".into());
        let mut edit = TextEdit::new(username);
        edit.set_single_line(true);
        let ram_index = |value: &Option<String>, default| value.as_deref()
            .and_then(|v| data::RAM_OPTIONS.iter().position(|o| *o == v)).unwrap_or(default);
        Self {
            route: Route::Home, tab: Tab::Mods, search: TextEdit::new(""), username: edit,
            launching: false, busy: false, loading: false, api_available: false,
            status: String::new(), modal: None,
            max_ram: ram_index(&settings.maxRam, 3), min_ram: ram_index(&settings.minRam, 1),
            hide_on_start: settings.hideLauncherOnStart.unwrap_or(false), ram_menu: None,
            hero: None, hero_hover: false, hero_cursor: None, icon: None, cache: None, logs: Vec::new(), log_autoscroll: true,
            downloads: BTreeMap::new(), event: data::fallback_event(), events: Vec::new(),
            thumbnails: Vec::new(), resource_thumbnails: Vec::new(), server_online: false,
            settings, viewport: [1536.0, 864.0], handle: None, stop: None,
            sync_plan: instance::SyncPlan::default(), updates: Vec::new(), loader_installed: false,
            legacy_files: Vec::new(), presence: None, update: None, updating: false,
        }
    }
}

impl AppState {
    pub fn log(&mut self, message: impl Into<String>) {
        self.logs.push(message.into());
        if self.logs.len() > 1000 { self.logs.drain(..self.logs.len() - 1000); }
    }

    pub fn save_username(&mut self) {
        self.settings.username = Some(self.username.text.clone());
        data::save_settings(&self.settings);
    }

    pub fn save_ram(&mut self, is_max: bool) {
        if is_max { self.min_ram = self.min_ram.min(self.max_ram); }
        else { self.max_ram = self.max_ram.max(self.min_ram); }
        self.settings.maxRam = Some(data::RAM_OPTIONS[self.max_ram].into());
        self.settings.minRam = Some(data::RAM_OPTIONS[self.min_ram].into());
        data::save_settings(&self.settings);
    }

    pub fn save_hide(&mut self) {
        self.settings.hideLauncherOnStart = Some(self.hide_on_start);
        data::save_settings(&self.settings);
    }

    pub fn refresh_installation(&mut self) {
        self.legacy_files = if paths::migration_marker_path().exists() { Vec::new() } else {
            std::fs::read_dir(paths::root()).into_iter().flatten().filter_map(Result::ok)
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|name| !paths::SHARED_ROOT_ENTRIES.contains(&name.as_str())).collect()
        };
        let game = paths::instance_dir(&self.event.id);
        self.sync_plan = instance::build_sync_plan(&self.event, &game, &instance::load_state(&game));
        self.updates = manifest::get_asset_update_plan(&self.event, &game);
        self.loader_installed = if self.event.is_fabric {
            minecraft::is_fabric_version_installed(&self.event)
        } else { minecraft::is_forge_version_installed(&self.event) };
    }

    pub fn action_caption(&self) -> &str {
        if self.loading { return "Loading…"; }
        if self.busy { return "Working…"; }
        if self.launching { return "Stop"; }
        if let Some(label) = manifest::get_update_button_label(&self.updates) { return label; }
        let missing_mods = self.sync_plan.downloads.iter().any(|d| d.kind == "mods");
        match (missing_mods, self.loader_installed, self.event.is_fabric) {
            (true, false, true) => "Install mods & fabric",
            (true, false, false) => "Install mods & forge",
            (true, true, _) => "Install mods",
            _ => instance::get_sync_label(&self.sync_plan).unwrap_or(if self.loader_installed {
                "Launch"
            } else if self.event.is_fabric { "Install fabric" } else { "Install forge" }),
        }
    }

    pub fn switch_variant(&mut self, id: &str) {
        if self.busy || self.launching || self.loading { return; }
        self.modal = None;
        self.load_event(Some(id.to_string()));
    }

    pub fn load_event(&mut self, selected: Option<String>) {
        let Some(handle) = self.handle.clone() else { return; };
        self.loading = true;
        self.status = "Loading event from API".into();
        self.log(self.status.clone());
        std::thread::spawn(move || {
            let events = data::fetch_events().unwrap_or_default();
            let chosen = data::choose_event(&events, selected.as_deref()).map(|e| e.id.clone());
            let event = data::fetch_event(selected.as_deref().or(chosen.as_deref()))
                .or_else(|| data::fetch_event(None));
            let available = event.is_some();
            let event = event.unwrap_or_else(data::fallback_event);
            let art_event = event.clone();
            handle.update(move |s, _| {
                s.event = event;
                s.events = events;
                s.api_available = available;
                s.loading = false;
                s.status = if available { "Ready" } else { "API unavailable; showing offline preview" }.into();
                if available {
                    s.settings.selectedEventId = Some(s.event.id.clone());
                    data::save_settings(&s.settings);
                }
                s.search = TextEdit::new("");
                if s.tab == Tab::Shaders && s.event.shader_packs_list().is_empty() { s.tab = Tab::Mods; }
                s.hero_hover = false;
                s.hero_cursor = None;
                s.thumbnails = vec![None; s.event.mods.len()];
                s.resource_thumbnails = vec![None; s.event.resource_packs.len()];
                s.refresh_installation();
                s.log(s.status.clone());
            });
            load_art(&handle, &art_event);
        });
    }

    pub fn check_update(&mut self) {
        let Some(handle) = self.handle.clone() else { return; };
        std::thread::spawn(move || {
            let Some(release) = data::fetch_launcher_release() else { return; };
            let newer = match (data::parse_version(&release.version), data::parse_version(data::APP_VERSION)) {
                (Some(latest), Some(current)) => latest > current,
                _ => false,
            };
            if !newer { return; }
            handle.update(move |s, _| {
                s.log(format!("Update available: v{}", release.version));
                s.update = Some(release);
            });
        });
    }

    pub fn install_update(&mut self) {
        let Some(handle) = self.handle.clone() else { return; };
        let Some(update) = self.update.clone() else { return; };
        if self.updating || self.busy || self.launching { return; }
        self.updating = true;
        self.status = format!("Downloading update v{}", update.version);
        self.log(self.status.clone());
        std::thread::spawn(move || {
            match download_installer(&update) {
                Ok(path) => {
                    let spawned = std::process::Command::new(&path)
                        .args(["/SILENT", "/SUPPRESSMSGBOXES", "/CLOSEAPPLICATIONS", "/NORESTART"])
                        .spawn();
                    match spawned {
                        Ok(_) => std::process::exit(0),
                        Err(error) => handle.update(move |s, _| {
                            s.updating = false;
                            s.fail(format!("Could not start the installer: {error}"));
                        }),
                    }
                }
                Err(error) => handle.update(move |s, _| { s.updating = false; s.fail(error); }),
            }
        });
    }

    pub fn primary_action(&mut self) {
        if self.loading || self.busy { return; }
        if self.launching {
            if let Some(stop) = &self.stop { let _ = stop.send(()); }
            self.busy = true;
            self.status = "Stopping Minecraft".into();
            return;
        }
        if !self.api_available {
            self.fail("API unavailable. Installation is disabled to protect local files.");
            return;
        }
        self.refresh_installation();
        if !self.legacy_files.is_empty() {
            self.modal = Some(ModalKind::ConfirmSync);
            return;
        }
        if self.sync_plan.pending() || !self.loader_installed || !self.updates.is_empty() {
            if !self.sync_plan.removals.is_empty() {
                self.modal = Some(ModalKind::ConfirmSync);
            } else { self.install(); }
        } else { self.launch(); }
    }

    pub fn fail(&mut self, message: impl Into<String>) {
        self.status = message.into();
        self.log(format!("Error: {}", self.status));
        self.modal = Some(ModalKind::Logs);
    }

    pub fn confirm_sync(&mut self) {
        if self.legacy_files.is_empty() { self.install(); return; }
        let Some(handle) = self.handle.clone() else { return; };
        if self.busy || self.launching || self.loading { return; }
        self.busy = true;
        self.modal = None;
        let id = self.event.id.clone();
        std::thread::spawn(move || {
            let result = paths::migrate_legacy_instance(&id);
            handle.update(move |s, _| {
                s.busy = false;
                s.log(format!("Legacy migration: {} moved, {} merged, {} conflicts", result.moved.len(), result.merged.len(), result.conflicts.len()));
                s.refresh_installation();
                if !result.conflicts.is_empty() { s.fail("Migration conflicts were preserved in the original folder. Inspect the launcher logs."); }
                else { s.primary_action(); }
            });
        });
    }

    pub fn install(&mut self) {
        let Some(handle) = self.handle.clone() else { return; };
        if self.busy || self.launching || !self.api_available { return; }
        self.modal = Some(ModalKind::Downloads);
        self.busy = true;
        self.status = "Synchronizing instance".into();
        let event = self.event.clone();
        let plan = self.sync_plan.clone();
        let loader_installed = self.loader_installed;
        std::thread::spawn(move || {
            let game = paths::ensure_instance(&event.id);
            let asset = |asset: &ApiMod, ok, status: &str| report_asset(&handle, asset, Some(ok), status);
            let file = |asset: &ApiMod, received, total| report_progress(&handle, &asset.file_name, received, total);
            let config = |config: &data::ApiConfigFile, done, status: &str| {
                report_named(&handle, &config.target_path, done, status);
            };
            let result = instance::apply_event_sync(&event, &game, &plan, instance::SyncCallbacks {
                asset: &asset, file: &file, config: &config,
            });
            let outcome = if !result.ok() {
                Err(format!("Synchronization failed: {} file errors, {} config errors", result.failed, result.configs_failed))
            } else if !loader_installed {
                minecraft::install_event(&event,
                    &|status| report_named(&handle, "Minecraft", None, status),
                    &|received, total| report_progress(&handle, "Minecraft", received, total))
                    .map(|_| ())
            } else { Ok(()) };
            let success = outcome.is_ok();
            report_named(&handle, "Minecraft", Some(success), if success { "Ready" } else { "Failed" });
            handle.update(move |s, _| {
                s.busy = false;
                s.refresh_installation();
                match outcome {
                    Ok(()) => { s.status = "Installation complete. Click Launch to play.".into(); s.log(s.status.clone()); }
                    Err(error) => s.fail(error),
                }
            });
        });
    }

    pub fn reinstall(&mut self, tab: Tab, index: usize) {
        if self.busy || self.launching || self.loading || !self.api_available { return; }
        let (assets, kind): (&[ApiMod], &'static str) = match tab {
            Tab::Mods => (&self.event.mods, "mods"),
            Tab::Shaders => (self.event.shader_packs_list(), "shader_packs"),
            Tab::Resources => (&self.event.resource_packs, "resource_packs"),
        };
        let Some(asset) = assets.get(index).cloned() else { return; };
        let Some(handle) = self.handle.clone() else { return; };
        let event_id = self.event.id.clone();
        self.busy = true;
        self.modal = Some(ModalKind::Downloads);
        std::thread::spawn(move || {
            let force = [asset.file_name.clone()].into_iter().collect();
            let game = paths::ensure_instance(&event_id);
            let (_, mut failed) = crate::download::download_assets(std::slice::from_ref(&asset), kind,
                &game, &event_id, &force,
                &|asset, ok, status| report_asset(&handle, asset, Some(ok), status),
                &|asset, received, total| report_progress(&handle, &asset.file_name, received, total));
            if failed == 0 {
                let mut state = instance::load_state(&game);
                instance::record_asset(&mut state, kind, &manifest::asset_id_for(&asset, index), &asset.file_name, Some(&asset.file_url));
                if instance::save_state(&state, &game).is_err() { failed += 1; }
            }
            handle.update(move |s, _| {
                s.busy = false;
                s.refresh_installation();
                if failed > 0 { s.fail("File download failed; previous file was preserved."); }
                else { s.status = "File installed".into(); }
            });
        });
    }

    pub fn open_folder(&mut self) {
        let folder = paths::ensure_instance(&self.event.id);
        let program = if cfg!(windows) { "explorer.exe" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        let folder = if cfg!(windows) {
            std::path::PathBuf::from(folder.to_string_lossy().replace('/', "\\"))
        } else { folder };
        if let Err(error) = std::process::Command::new(program).arg(folder).spawn() { self.fail(error.to_string()); }
    }

    fn launch(&mut self) {
        if self.username.text.trim().is_empty() { self.fail("Enter a username before launching."); return; }
        let Some(handle) = self.handle.clone() else { return; };
        self.busy = true;
        self.status = "Starting Minecraft".into();
        let event = self.event.clone();
        let settings = self.settings.clone();
        let username = self.username.text.trim().to_string();
        let hide = self.hide_on_start;
        std::thread::spawn(move || {
            let game = paths::ensure_instance(&event.id);
            let version = minecraft::resolve_version_id(&event, None);
            let prepared = minecraft::prepare_launch(&event, &settings, &username, &version, &game);
            let mut child = match prepared.and_then(|p| minecraft::spawn_process(&p)) {
                Ok(child) => child,
                Err(error) => { handle.update(move |s, _| { s.busy = false; s.fail(error); }); return; }
            };
            let (stop, receiver) = mpsc::channel();
            let title = event.title.clone();
            handle.update(move |s, _| {
                s.logs.clear(); s.log("Minecraft process started");
                s.stop = Some(stop); s.launching = true; s.busy = false; s.status = "Minecraft running".into();
                if let Some(presence) = &s.presence { presence.playing(&title); }
            });
            if hide { handle.hide(); }
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();
            let readers: Vec<_> = [stdout.map(|v| Box::new(v) as Box<dyn std::io::Read + Send>),
                stderr.map(|v| Box::new(v) as Box<dyn std::io::Read + Send>)].into_iter().flatten().map(|pipe| {
                let handle = handle.clone();
                std::thread::spawn(move || {
                    for line in BufReader::new(pipe).lines() {
                        let Ok(line) = line else { break; };
                        if !line.is_empty() { handle.update(move |s, _| s.log(line)); }
                    }
                })
            }).collect();
            let outcome = loop {
                match child.try_wait() {
                    Ok(Some(status)) => break Ok(status),
                    Err(error) => break Err(error.to_string()),
                    Ok(None) => {}
                }
                if receiver.recv_timeout(Duration::from_millis(150)).is_ok() {
                    if let Err(error) = child.kill() { break Err(error.to_string()); }
                    break child.wait().map_err(|e| e.to_string());
                }
            };
            for reader in readers { let _ = reader.join(); }
            if hide { handle.show(); }
            handle.update(move |s, _| {
                s.stop = None; s.launching = false; s.busy = false;
                if let Some(presence) = &s.presence { presence.home(); }
                match outcome {
                    Ok(code) => { s.status = format!("Minecraft exited ({code})"); s.log(s.status.clone()); }
                    Err(error) => s.fail(error),
                }
            });
        });
    }
}

fn report_named(handle: &AppHandle<AppState>, name: &str, done: Option<bool>, status: &str) {
    let name = name.to_string();
    let status = status.to_string();
    handle.update(move |s, _| {
        s.status = status.clone();
        s.log(format!("{name}: {status}"));
        let item = s.downloads.entry(name).or_default();
        item.done = done; item.status = status;
    });
}

fn report_asset(handle: &AppHandle<AppState>, asset: &ApiMod, done: Option<bool>, status: &str) {
    report_named(handle, &asset.file_name, done, status);
}

fn report_progress(handle: &AppHandle<AppState>, name: &str, received: u64, total: Option<u64>) {
    let name = if name.is_empty() { "Config" } else { name }.to_string();
    handle.update(move |s, _| {
        let item = s.downloads.entry(name).or_default();
        item.received = received; item.total = total; item.done = None;
        item.status = "Downloading".into();
    });
}

fn load_art(handle: &AppHandle<AppState>, event: &ApiEvent) {
    let bytes = data::fetch_bytes(&event.background_image)
        .unwrap_or_else(|| include_bytes!("../assets/hero-final.png").to_vec());
    let id = event.id.clone();
    handle.update(move |s, _| {
        if s.event.id == id { if let Some(cache) = &s.cache { s.hero = Some(cache.load(bytes)); } }
    });
    let jobs: Vec<_> = event.mods.iter().enumerate().map(|(i, a)| (Tab::Mods, i, a.thumbnail.clone()))
        .chain(event.resource_packs.iter().enumerate().map(|(i, a)| (Tab::Resources, i, a.thumbnail.clone())))
        .filter(|(_, _, url)| !url.is_empty()).collect();
    std::thread::scope(|scope| {
        for batch in jobs.chunks(jobs.len().div_ceil(6).max(1)) {
            let id = &event.id;
            scope.spawn(move || {
                for (tab, index, url) in batch {
                    let Some(bytes) = data::fetch_bytes(url) else { continue; };
                    let id = id.clone(); let tab = *tab; let index = *index;
                    handle.update(move |s, _| {
                        if s.event.id != id { return; }
                        if let Some(cache) = &s.cache {
                            let images = if tab == Tab::Mods { &mut s.thumbnails } else { &mut s.resource_thumbnails };
                            if let Some(slot) = images.get_mut(index) { *slot = Some(cache.load(bytes)); }
                        }
                    });
                }
            });
        }
    });
}

fn download_installer(release: &data::LauncherRelease) -> Result<std::path::PathBuf, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(300))
        .build().map_err(|e| e.to_string())?;
    let response = client.get(&release.url).send().map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Update download failed (HTTP {})", response.status()));
    }
    let bytes = response.bytes().map_err(|e| e.to_string())?;
    if !release.sha256.is_empty() {
        let actual = crate::sha256::hex_bytes(&bytes);
        if !actual.eq_ignore_ascii_case(release.sha256.trim()) {
            return Err("Update checksum mismatch; the download was discarded.".into());
        }
    }
    let path = std::env::temp_dir()
        .join(format!("PrivateMewingLauncher-Setup-{}.exe", release.version));
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_preview_cannot_install_or_launch() {
        let mut state = AppState::default();
        state.primary_action();
        assert!(!state.busy && !state.launching);
        assert_eq!(state.modal, Some(ModalKind::Logs));
    }

    #[test]
    fn logs_are_bounded() {
        let mut state = AppState::default();
        for i in 0..1100 { state.log(i.to_string()); }
        assert_eq!(state.logs.len(), 1000);
        assert_eq!(state.logs[0], "100");
    }

    #[test]
    fn active_process_is_not_lost_on_navigation() {
        let mut state = AppState::default();
        let (stop, receiver) = mpsc::channel();
        state.stop = Some(stop); state.launching = true;
        state.route = Route::Settings;
        state.switch_variant("different");
        assert_eq!(state.event.id, "cobblepass-5");
        state.primary_action();
        assert!(receiver.try_recv().is_ok());
        assert!(state.busy);
    }
}
