//! Blocking file downloads with resume, retries and bounded concurrency.
//!
//! Mirrors `launcher.download_file` and the per-kind parallel downloaders:
//! `.part` files, `Range` resume, five attempts, retryable HTTP statuses and
//! 256 KiB chunks.

use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::blocking::Client;

use crate::data::ApiMod;
use crate::manifest;
use crate::paths;

const CHUNK_SIZE: usize = 256 * 1024;
const MAX_ATTEMPTS: u32 = 5;

pub fn http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(15))
        .user_agent("gmtib/private-mewing-launcher/0.1.0")
        .build()
        .unwrap_or_else(|_| Client::new())
}

fn retryable_status(status: u16) -> bool {
    matches!(status, 408 | 425 | 429 | 500 | 502 | 503 | 504)
}

fn describe(error: &reqwest::Error) -> String {
    if let Some(status) = error.status() {
        return format!("HTTP {}", status.as_u16());
    }
    if error.is_timeout() {
        return "request timed out".into();
    }
    let text = error.to_string();
    if text.is_empty() { "request failed".into() } else { text }
}

/// Download `url` to `dest`, resuming a `.part` file when the server allows it.
pub fn download_file(
    url: &str,
    dest: &Path,
    progress: &mut dyn FnMut(u64, Option<u64>),
    error_cb: &mut dyn FnMut(&str, u32, u32),
) -> bool {
    if url.is_empty() {
        error_cb("empty URL", 1, MAX_ATTEMPTS);
        return false;
    }
    let tmp_path = Path::new(&format!("{}.part", dest.to_string_lossy())).to_path_buf();
    if let Some(parent) = dest.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let client = http_client();

    for attempt in 1..=MAX_ATTEMPTS {
        let resume_from = fs::metadata(&tmp_path).map(|m| m.len()).unwrap_or(0);
        let mut request = client.get(url);
        if resume_from > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={resume_from}-"));
        }

        let result = (|| -> Result<(), String> {
            let mut response = request.send().map_err(|e| describe(&e))?;
            let status = response.status();
            if retryable_status(status.as_u16()) {
                return Err(format!("retryable HTTP {}", status.as_u16()));
            }
            if !status.is_success() {
                return Err(format!("HTTP {}", status.as_u16()));
            }

            let append = resume_from > 0 && status.as_u16() == 206;
            let mut start = resume_from;
            if resume_from > 0 && !append {
                start = 0;
                let _ = fs::remove_file(&tmp_path);
            }

            let mut total = response.content_length();
            if append {
                if let Some(length) = total {
                    total = Some(length + resume_from);
                }
            }

            let mut file = fs::OpenOptions::new()
                .create(true)
                .write(true)
                .append(append)
                .truncate(!append)
                .open(&tmp_path)
                .map_err(|e| e.to_string())?;

            let mut downloaded = start;
            let mut buffer = vec![0u8; CHUNK_SIZE];
            loop {
                let read = response.read(&mut buffer).map_err(|e| e.to_string())?;
                if read == 0 {
                    break;
                }
                file.write_all(&buffer[..read]).map_err(|e| e.to_string())?;
                downloaded += read as u64;
                progress(downloaded, total);
            }
            file.flush().map_err(|e| e.to_string())?;
            drop(file);

            fs::rename(&tmp_path, dest).map_err(|e| e.to_string())?;
            Ok(())
        })();

        match result {
            Ok(()) => return true,
            Err(message) => {
                if attempt < MAX_ATTEMPTS {
                    error_cb(&message, attempt, MAX_ATTEMPTS);
                    std::thread::sleep(Duration::from_millis(750 * attempt as u64));
                    continue;
                }
                error_cb(&message, attempt, MAX_ATTEMPTS);
                let _ = fs::remove_file(&tmp_path);
                return false;
            }
        }
    }
    false
}

fn auto_concurrency(kind: &str) -> usize {
    let cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let base = (cpus * 2).max(4);
    if cfg!(windows) {
        if kind == "resourcepacks" { base.min(4) } else { base.min(6) }
    } else if kind == "resourcepacks" {
        base.min(12)
    } else {
        base.min(16)
    }
}

/// Download every advertised asset that is not already on disk.
///
/// Existing files are recorded in the manifest and reported as
/// `already_exists`; missing files download in parallel.
pub fn download_assets(
    assets: &[ApiMod],
    kind: &str,
    game_dir: &Path,
    event_id: &str,
    force_file_names: &BTreeSet<String>,
    asset_cb: &(dyn Fn(&ApiMod, bool, &str) + Sync),
    file_cb: &(dyn Fn(&ApiMod, u64, Option<u64>) + Sync),
) -> (usize, usize) {
    download_assets_at(assets, kind, game_dir, event_id, force_file_names, asset_cb, file_cb, &paths::manifest_path())
}

pub(crate) fn download_assets_at(
    assets: &[ApiMod],
    kind: &str,
    game_dir: &Path,
    event_id: &str,
    force_file_names: &BTreeSet<String>,
    asset_cb: &(dyn Fn(&ApiMod, bool, &str) + Sync),
    file_cb: &(dyn Fn(&ApiMod, u64, Option<u64>) + Sync),
    manifest_path: &Path,
) -> (usize, usize) {
    let folder = paths::asset_folder(kind, game_dir);
    if !paths::is_within_directory(game_dir, &folder) {
        for asset in assets {
            asset_cb(asset, false, "unsafe target");
        }
        return (0, assets.len());
    }
    let _ = fs::create_dir_all(&folder);
    let existing: BTreeSet<String> = paths::local_file_names(&folder).into_iter().collect();

    let mut already = 0usize;
    let mut invalid = 0usize;
    let mut missing: Vec<ApiMod> = Vec::new();
    for asset in assets {
        if asset.file_name.is_empty() {
            continue;
        }
        if paths::safe_asset_target(game_dir, kind, &asset.file_name).is_none() {
            invalid += 1;
            asset_cb(asset, false, "unsafe target");
            continue;
        }
        if existing.contains(&asset.file_name) && !force_file_names.contains(&asset.file_name) {
            already += 1;
            let _ = manifest::record_installed_at(manifest_path, event_id, kind, &manifest::asset_id_for(asset, 0), &asset.file_name, Some(&asset.file_url));
            asset_cb(asset, true, "already_exists");
        } else {
            missing.push(asset.clone());
        }
    }

    if missing.is_empty() {
        return (already, invalid);
    }

    let missing_total = missing.len();
    let concurrency = auto_concurrency(kind).min(missing.len()).max(1);
    let queue = Arc::new(Mutex::new(missing.into_iter().enumerate().collect::<Vec<_>>()));
    let succeeded = Arc::new(Mutex::new(0usize));

    std::thread::scope(|scope| {
        for _ in 0..concurrency {
            let queue = Arc::clone(&queue);
            let succeeded = Arc::clone(&succeeded);
            scope.spawn(move || loop {
                let next = {
                    let mut queue = queue.lock().unwrap();
                    queue.pop()
                };
                let Some((_index, asset)) = next else { break };
                let Some(dest) = paths::safe_asset_target(game_dir, kind, &asset.file_name) else {
                    asset_cb(&asset, false, "unsafe target");
                    continue;
                };
                if paths::safe_asset_target(game_dir, kind, &format!("{}.part", asset.file_name)).is_none() {
                    asset_cb(&asset, false, "unsafe target");
                    continue;
                }
                let ok = download_file(
                    &asset.file_url,
                    &dest,
                    &mut |downloaded, total| file_cb(&asset, downloaded, total),
                    &mut |_message, _attempt, _max| {},
                );
                if ok {
                    let _ = manifest::record_installed_at(
                        manifest_path,
                        event_id,
                        kind,
                        &manifest::asset_id_for(&asset, 0),
                        &asset.file_name,
                        Some(&asset.file_url),
                    );
                    *succeeded.lock().unwrap() += 1;
                    asset_cb(&asset, true, "downloaded");
                } else {
                    asset_cb(&asset, false, "failed");
                }
            });
        }
    });

    let successful = *succeeded.lock().unwrap();
    (already + successful, invalid + missing_total - successful)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use paths::tests::TempDir;

    pub(crate) fn serve_once(body: &'static [u8]) -> (String, std::thread::JoinHandle<()>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/default", listener.local_addr().unwrap());
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
            let mut request = [0u8; 4096];
            stream.read(&mut request).unwrap();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
            stream.write_all(body).unwrap();
        });
        (url, thread)
    }

    #[test]
    fn unsafe_api_names_fail_without_downloading_or_recording() {
        let temp = TempDir::new();
        let game = temp.0.join("game");
        let manifest_path = temp.0.join("manifest.json");
        let assets: Vec<ApiMod> = ["../escape.jar", "..\\escape.jar", "file.jar:ads", "CON.jar", "LPT1.zip", "/escape.jar", "C:\\escape.jar", "mod.jar."]
            .iter().map(|name| ApiMod { file_name: (*name).into(), ..Default::default() }).collect();
        let failures = Mutex::new(Vec::new());
        let result = download_assets_at(&assets, "mods", &game, "event", &BTreeSet::new(),
            &|asset, ok, status| { assert!(!ok); failures.lock().unwrap().push((asset.file_name.clone(), status.to_string())); },
            &|_, _, _| panic!("unsafe asset attempted a download"), &manifest_path);
        assert_eq!(result, (0, assets.len()));
        assert_eq!(failures.into_inner().unwrap().len(), assets.len());
        assert_eq!(download_assets_at(&[ApiMod::default()], "mods", &game, "event", &BTreeSet::new(),
            &|_, _, _| panic!("unnamed assets should still be skipped"), &|_, _, _| {}, &manifest_path), (0, 0));
        assert!(!manifest_path.exists());
        assert!(!game.join("escape.jar").exists());
        assert!(paths::local_file_names(&game.join("mods")).is_empty());
    }

    #[test]
    fn escaped_asset_folder_is_not_written() {
        let temp = TempDir::new();
        let game = temp.0.join("game");
        let outside = temp.0.join("outside");
        fs::create_dir_all(&game).unwrap();
        fs::create_dir_all(&outside).unwrap();
        paths::tests::directory_link(&outside, &game.join("mods"));
        let asset = ApiMod { file_name: "mod.jar".into(), ..Default::default() };
        assert_eq!(download_assets_at(&[asset], "mods", &game, "event", &BTreeSet::new(), &|_, ok, _| assert!(!ok), &|_, _, _| {}, &temp.0.join("manifest.json")), (0, 1));
        assert!(fs::read_dir(&outside).unwrap().next().is_none());
    }

    #[test]
    fn valid_missing_asset_downloads_and_existing_asset_is_recorded() {
        let temp = TempDir::new();
        let game = temp.0.join("game");
        let manifest_path = temp.0.join("manifest.json");
        let (url, server) = serve_once(b"asset contents");
        let asset = ApiMod { id: "asset-id".into(), file_name: "mod.jar".into(), file_url: url.clone(), ..Default::default() };
        let call = |expected| {
            assert_eq!(download_assets_at(std::slice::from_ref(&asset), "mods", &game, "event", &BTreeSet::new(),
                &|_, ok, status| { assert!(ok); assert_eq!(status, expected); }, &|_, _, _| {}, &manifest_path), (1, 0));
        };
        call("downloaded");
        server.join().unwrap();
        call("already_exists");
        assert_eq!(fs::read(game.join("mods/mod.jar")).unwrap(), b"asset contents");
        assert_eq!(manifest::load_manifest_at(&manifest_path).events["event"]["mods"]["asset-id"].file_url.as_deref(), Some(url.as_str()));
    }
}
