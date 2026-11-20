//! Minecraft version resolution, Java selection, installation and launch.
//!
//! The original delegates to `minecraft-launcher-lib`; this port uses
//! `mc-launcher-core` for the same vanilla/Fabric/Forge work and keeps the
//! original version-id heuristics so existing installs are recognised.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use mc_launcher_core::prelude::{
    Account, InstallRequest, JavaInstallPolicy, LaunchOptions, Launcher, LoaderSpec, LoaderVersion,
    ProgressEvent,
};
use mc_launcher_core::progress::InstallStage;

use crate::data::{ApiEvent, SettingsFile};
use crate::paths;

fn normalize_minecraft_version(value: &str) -> Option<String> {
    if value.starts_with("1.") { Some(value.to_string()) } else { None }
}

fn split_forge_version(forge_version: &str, minecraft_version: Option<&str>) -> (Option<String>, Option<String>) {
    if !forge_version.is_empty() {
        if let Some((mc, loader)) = forge_version.split_once('-') {
            return (Some(mc.to_string()), Some(loader.to_string()));
        }
        if let Some(mc) = minecraft_version.filter(|v| !v.is_empty()) {
            return (Some(mc.to_string()), Some(forge_version.to_string()));
        }
    }
    (minecraft_version.map(str::to_string), None)
}

fn extract_minecraft_version_from_fabric(fabric_version: &str) -> Option<String> {
    if fabric_version.is_empty() {
        return None;
    }
    fabric_version
        .split('-')
        .rev()
        .find(|part| part.starts_with("1."))
        .map(str::to_string)
}

fn parse_fabric_loader_version(fabric_version: &str, minecraft_version: Option<&str>) -> Option<String> {
    if fabric_version.is_empty() {
        return None;
    }
    if let Some(mc) = minecraft_version {
        if fabric_version == mc {
            return None;
        }
    }
    if fabric_version.starts_with("1.")
        && fabric_version.replace('.', "").chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }

    let mut value = fabric_version.to_string();
    if let Some(pos) = value.find("fabric-loader-") {
        value = value[pos + "fabric-loader-".len()..].to_string();
    }
    if let Some(mc) = minecraft_version {
        let needle = format!("-{mc}");
        if let Some(pos) = value.find(&needle) {
            value = value[..pos].to_string();
        }
    }
    if value.contains('-') {
        let mut parts: Vec<&str> = value.split('-').collect();
        if parts.last().map(|p| p.starts_with("1.")).unwrap_or(false) {
            parts.pop();
            value = parts.join("-");
            if value.is_empty() {
                value = fabric_version.to_string();
            }
        }
    }
    if let Some(mc) = minecraft_version {
        if value == mc {
            return None;
        }
    }
    (!value.is_empty()).then_some(value)
}

fn choose_minecraft_version_for_fabric(minecraft_version: Option<&str>, fabric_version: &str) -> Option<String> {
    let extracted = extract_minecraft_version_from_fabric(fabric_version);
    let minecraft_version = minecraft_version.and_then(normalize_minecraft_version);
    if let Some(ref extracted) = extracted {
        if minecraft_version.is_none() {
            return Some(extracted.clone());
        }
        if minecraft_version.as_deref() != Some(extracted.as_str())
            && !fabric_version.contains(minecraft_version.as_deref().unwrap_or(""))
        {
            return Some(extracted.clone());
        }
    }
    minecraft_version.or(extracted)
}

pub fn installed_version_ids() -> Vec<String> {
    let versions = paths::root().join("versions");
    match fs::read_dir(versions) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect(),
        Err(_) => Vec::new(),
    }
}

pub fn is_version_installed(version: &str) -> bool {
    paths::root().join("versions").join(version).exists()
}

fn installed_fabric_id(mc: Option<&str>, loader: Option<&str>) -> Option<String> {
    let ids = installed_version_ids();
    ids.into_iter().find(|id| {
        id.contains("fabric")
            && mc.map(|mc| id.contains(mc)).unwrap_or(true)
            && loader.map(|loader| id.contains(loader)).unwrap_or(true)
    })
}

fn installed_forge_id(mc: Option<&str>, loader: Option<&str>) -> Option<String> {
    let ids = installed_version_ids();
    ids.into_iter().find(|id| {
        id.contains("forge")
            && mc.map(|mc| id.contains(mc)).unwrap_or(true)
            && loader.map(|loader| id.contains(loader)).unwrap_or(true)
    })
}

pub fn resolve_version_id(event: &ApiEvent, fallback_version: Option<&str>) -> String {
    let minecraft_version = normalize_minecraft_version(&event.version);
    if event.is_fabric {
        let mc = choose_minecraft_version_for_fabric(minecraft_version.as_deref(), &event.fabric_version);
        let loader = parse_fabric_loader_version(&event.fabric_version, mc.as_deref());
        if let Some(found) = installed_fabric_id(mc.as_deref(), loader.as_deref()) {
            return found;
        }
        if let (Some(mc), Some(loader)) = (mc.as_deref(), loader.as_deref()) {
            return format!("fabric-loader-{loader}-{mc}");
        }
        return fallback_version
            .filter(|v| !v.is_empty())
            .map(str::to_string)
            .or_else(|| (!event.fabric_version.is_empty()).then(|| event.fabric_version.clone()))
            .or(mc)
            .unwrap_or_default();
    }

    let (mc, loader) = split_forge_version(&event.forge_version, minecraft_version.as_deref());
    if let Some(found) = installed_forge_id(mc.as_deref(), loader.as_deref()) {
        return found;
    }
    if let (Some(mc), Some(loader)) = (mc.as_deref(), loader.as_deref()) {
        return format!("{mc}-forge-{loader}");
    }
    fallback_version
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .or_else(|| (!event.forge_version.is_empty()).then(|| event.forge_version.clone()))
        .or(mc)
        .unwrap_or_default()
}

pub fn is_fabric_version_installed(event: &ApiEvent) -> bool {
    if !event.is_fabric {
        return true;
    }
    let mc = choose_minecraft_version_for_fabric(
        normalize_minecraft_version(&event.version).as_deref(),
        &event.fabric_version,
    );
    let loader = parse_fabric_loader_version(&event.fabric_version, mc.as_deref());
    if let (Some(mc), Some(loader)) = (mc.as_deref(), loader.as_deref()) {
        let expected = format!("fabric-loader-{loader}-{mc}");
        if is_version_installed(&expected) {
            return true;
        }
    }
    installed_fabric_id(mc.as_deref(), loader.as_deref())
        .map(|id| is_version_installed(&id))
        .unwrap_or(false)
}

pub fn is_forge_version_installed(event: &ApiEvent) -> bool {
    if event.is_fabric {
        return true;
    }
    let (mc, loader) = split_forge_version(
        &event.forge_version,
        normalize_minecraft_version(&event.version).as_deref(),
    );
    if let (Some(mc), Some(loader)) = (mc.as_deref(), loader.as_deref()) {
        let expected = format!("{mc}-forge-{loader}");
        if is_version_installed(&expected) {
            return true;
        }
    }
    installed_forge_id(mc.as_deref(), loader.as_deref())
        .map(|id| is_version_installed(&id))
        .unwrap_or(false)
}

fn runtime_executable(runtime_name: &str) -> Option<PathBuf> {
    mc_launcher_core::runtime::get_executable_path(runtime_name, paths::root())
        .filter(|p| p.is_file())
}

/// Resolve the Mojang runtime for `minecraft_version`, installing it if needed.
fn minecraft_runtime_java(minecraft_version: &str, status: &dyn Fn(&str)) -> Option<PathBuf> {
    let info = mc_launcher_core::runtime::get_version_runtime_information(minecraft_version, paths::root())?;
    if let Some(path) = runtime_executable(&info.name) {
        return Some(path);
    }
    status("Installing Java runtime");
    let callback = mc_launcher_core::types::CallbackDict {
        set_status: None,
        set_progress: None,
        set_max: None,
    };
    if mc_launcher_core::runtime::install_jvm_runtime(&info.name, paths::root(), &callback).is_err() {
        return None;
    }
    runtime_executable(&info.name)
}

fn java_from_home() -> Option<PathBuf> {
    let home = std::env::var_os("JAVA_HOME")?;
    let name = if cfg!(windows) { "java.exe" } else { "java" };
    let path = PathBuf::from(home).join("bin").join(name);
    path.is_file().then_some(path)
}

fn java_from_path() -> Option<PathBuf> {
    let name = if cfg!(windows) { "java.exe" } else { "java" };
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn java_from_common_dirs() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let name = "java.exe";
    let roots = [
        r"C:\Program Files\Java",
        r"C:\Program Files\Eclipse Adoptium",
        r"C:\Program Files\Microsoft",
        r"C:\Program Files\Zulu",
        r"C:\Program Files\BellSoft",
        r"C:\Program Files\Amazon Corretto",
    ];
    for root in roots {
        let Ok(entries) = fs::read_dir(root) else { continue };
        for entry in entries.filter_map(|e| e.ok()) {
            let candidate = entry.path().join("bin").join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn system_java() -> Option<PathBuf> {
    java_from_home().or_else(java_from_path).or_else(java_from_common_dirs)
}

pub fn resolve_java_executable(minecraft_version: &str, status: &dyn Fn(&str)) -> Result<PathBuf, String> {
    if let Some(path) = minecraft_runtime_java(minecraft_version, status) {
        return Ok(path);
    }
    if let Some(path) = system_java() {
        return Ok(path);
    }
    Err("Java runtime not found. Install Java 21 or let the launcher download the Minecraft runtime, then try installing the loader again.".into())
}

fn with_java_on_path<T>(java: &Path, f: impl FnOnce() -> T) -> T {
    let old_path = std::env::var_os("PATH");
    if let Some(bin) = java.parent() {
        let mut parts: Vec<PathBuf> = vec![bin.to_path_buf()];
        if let Some(old) = &old_path {
            parts.extend(std::env::split_paths(old));
        }
        if let Ok(joined) = std::env::join_paths(parts) {
            std::env::set_var("PATH", joined);
        }
    }
    let result = f();
    if let Some(old) = old_path {
        std::env::set_var("PATH", old);
    } else { std::env::remove_var("PATH"); }
    result
}

fn stage_label(stage: &InstallStage) -> &'static str {
    match stage {
        InstallStage::ResolveVersion => "Resolving version",
        InstallStage::DownloadLibraries => "Downloading libraries",
        InstallStage::DownloadAssets => "Downloading assets",
        InstallStage::InstallRuntime => "Installing Java runtime",
        InstallStage::ExtractNatives => "Extracting natives",
        InstallStage::LoaderInstall => "Installing loader",
        InstallStage::Verify => "Verifying files",
    }
}

/// Install vanilla plus the event's loader into the shared runtime.
pub fn install_event(
    event: &ApiEvent,
    on_status: &dyn Fn(&str),
    on_bytes: &dyn Fn(u64, Option<u64>),
) -> Result<String, String> {
    let launcher = Launcher::new(paths::root());
    let mut reporter = |progress: ProgressEvent| match progress {
        ProgressEvent::StageStarted { stage } => on_status(stage_label(&stage)),
        ProgressEvent::BytesReceived { received, total, .. } => on_bytes(received, total),
        _ => {}
    };

    if event.is_fabric {
        let minecraft_version = choose_minecraft_version_for_fabric(
            normalize_minecraft_version(&event.version).as_deref(),
            &event.fabric_version,
        )
        .ok_or_else(|| "Minecraft version is required to install Fabric".to_string())?;
        let loader = parse_fabric_loader_version(&event.fabric_version, Some(&minecraft_version));
        let loader_version = match loader {
            Some(version) => LoaderVersion::Exact(version),
            None => LoaderVersion::LatestStable,
        };
        let result = launcher
            .install_with_progress(
                InstallRequest {
                    minecraft_version,
                    loader: Some(LoaderSpec::Fabric { version: loader_version }),
                    java: JavaInstallPolicy::Never,
                },
                &mut reporter,
            )
            .map_err(|e| e.to_string())?;
        return Ok(result.version_id);
    }

    let (minecraft_version, loader_version) = split_forge_version(
        &event.forge_version,
        normalize_minecraft_version(&event.version).as_deref(),
    );
    let minecraft_version = minecraft_version.ok_or_else(|| "Minecraft version is required to install Forge".to_string())?;
    let loader_version = loader_version.ok_or_else(|| "Forge version is required".to_string())?;
    on_status("Installing Java runtime");
    let java = resolve_java_executable(&minecraft_version, on_status)?;
    on_status("Installing Minecraft version");
    let combined = format!("{minecraft_version}-{loader_version}");
    let result = with_java_on_path(&java, || {
        launcher.install_with_progress(
            InstallRequest {
                minecraft_version: minecraft_version.clone(),
                loader: Some(LoaderSpec::Forge { version: LoaderVersion::Exact(combined) }),
                java: JavaInstallPolicy::Never,
            },
            &mut reporter,
        )
    })
    .map_err(|e| e.to_string())?;
    Ok(result.version_id)
}

fn normalize_ram(value: Option<&str>, default: &str) -> String {
    let Some(value) = value else { return default.to_string() };
    let value = value.trim().to_uppercase();
    if value.is_empty() {
        return default.to_string();
    }
    if value.ends_with("MB") {
        return format!("{}M", &value[..value.len() - 2]);
    }
    if value.ends_with('M') || value.ends_with('G') {
        return value;
    }
    if value.chars().all(|c| c.is_ascii_digit()) {
        return format!("{value}G");
    }
    default.to_string()
}

pub fn get_ram(settings: &SettingsFile) -> (String, String) {
    (
        normalize_ram(settings.minRam.as_deref(), "1G"),
        normalize_ram(settings.maxRam.as_deref(), "2G"),
    )
}

#[derive(Clone, Debug)]
pub struct PreparedLaunch {
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    pub env: Vec<(String, String)>,
}

/// Build the Java command for one instance, prepending the launcher's `-Xmx`/`-Xms`.
pub fn prepare_launch(
    event: &ApiEvent,
    settings: &SettingsFile,
    username: &str,
    version_id: &str,
    game_dir: &Path,
) -> Result<PreparedLaunch, String> {
    let (min_ram, max_ram) = get_ram(settings);
    let minecraft_version = if event.is_fabric {
        choose_minecraft_version_for_fabric(normalize_minecraft_version(&event.version).as_deref(), &event.fabric_version)
    } else {
        split_forge_version(&event.forge_version, normalize_minecraft_version(&event.version).as_deref()).0
    }.ok_or_else(|| "Minecraft version is missing".to_string())?;
    let java = resolve_java_executable(&minecraft_version, &|_| {})?;

    let launcher = Launcher::new(paths::root());
    let version = launcher.load_version(version_id).map_err(|e| e.to_string())?;
    let command = launcher
        .build_launch_command_from_version(
            &version,
            LaunchOptions {
                account: Account::offline(username),
                java_executable: Some(java),
                game_directory: Some(game_dir.to_path_buf()),
                launcher_name: crate::data::APP_NAME.to_string(),
                launcher_version: crate::data::APP_VERSION.to_string(),
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;

    let mut args = Vec::with_capacity(command.args.len() + 2);
    args.push(format!("-Xmx{max_ram}"));
    args.push(format!("-Xms{min_ram}"));
    args.extend(command.args);

    Ok(PreparedLaunch {
        executable: command.executable,
        args,
        working_dir: command.working_dir,
        env: command.env,
    })
}

/// Spawn the prepared Java command with a hidden console and piped output.
pub fn spawn_process(launch: &PreparedLaunch) -> Result<Child, String> {
    let mut command = Command::new(&launch.executable);
    command
        .args(&launch.args)
        .envs(launch.env.iter().cloned())
        .current_dir(&launch.working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    command.spawn().map_err(|e| e.to_string())
}

/// Whether a version profile's Java component is available locally.
pub fn local_java_version(minecraft_version: &str) -> Option<String> {
    let path = paths::root()
        .join("versions")
        .join(minecraft_version)
        .join(format!("{minecraft_version}.json"));
    let text = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value
        .get("javaVersion")
        .and_then(|v| v.get("component"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

/// All version ids the launcher knows about (for diagnostics and tests).
pub fn installed_versions() -> BTreeSet<String> {
    installed_version_ids().into_iter().collect()
}
