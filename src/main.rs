mod data;
mod discord;
mod download;
mod home;
mod icons;
mod instance;
mod manifest;
mod minecraft;
mod paths;
mod settings;
mod sha256;
mod state;
mod theme;
mod time;
mod widgets;

use rune::{AppSpec, Cx, El, ImageCache, Key, KeyEvent, Tx};

use state::{AppState, ModalKind, Route, Tab};

fn view(s: &AppState, cx: &mut Cx<AppState>) -> El {
    match s.route {
        Route::Home => home::view(s, cx),
        Route::Settings => settings::view(s, cx),
    }
}

fn on_key(s: &mut AppState, e: &KeyEvent, _: &mut Tx) -> bool {
    if e.key == Key::Escape {
        if s.modal.is_some() { s.modal = None; return true; }
        if s.ram_menu.is_some() { s.ram_menu = None; return true; }
    }
    false
}

fn setup(s: &mut AppState, waker: rune::Waker) {
    let cache = ImageCache::new(waker);
    let image = cache.load(include_bytes!("../assets/hero-final.png").to_vec());
    let icon = cache.load(include_bytes!("../assets/icon-256.png").to_vec());
    let _ = image.wait();
    let _ = icon.wait();
    s.hero = Some(image);
    s.icon = Some(icon);
    s.cache = Some(cache);
    let headless = rune::tooling::headless_run().is_some();
    if !headless {
        s.presence = Some(discord::Presence::start());
        s.load_event(s.settings.selectedEventId.clone());
        s.check_update();
    }
}

fn main() {
    let mut spec = AppSpec::new(
        data::APP_NAME,
        (1536.0, 864.0),
        AppState::default,
        view,
    );
    spec.fonts = &[
        include_bytes!("../assets/fonts/Inter-400.ttf"),
        include_bytes!("../assets/fonts/Inter-500.ttf"),
        include_bytes!("../assets/fonts/Inter-600.ttf"),
        include_bytes!("../assets/fonts/Inter-700.ttf"),
    ];
    spec.families = ("Inter", "Inter");
    spec.scenes = &[
        ("home", |_| {}),
        ("settings", |s: &mut AppState| s.route = Route::Settings),
        ("logs", |s: &mut AppState| s.modal = Some(ModalKind::Logs)),
        ("downloads", |s: &mut AppState| s.modal = Some(ModalKind::Downloads)),
        ("resources", |s: &mut AppState| s.tab = Tab::Resources),
        ("search-empty", |s: &mut AppState| s.search = rune::TextEdit::new("no-match-fixture")),
        ("ram-menu", |s: &mut AppState| { s.route = Route::Settings; s.ram_menu = Some(true); }),
        ("sync", |s: &mut AppState| s.modal = Some(ModalKind::ConfirmSync)),
        ("downloads-active", |s: &mut AppState| {
            s.modal = Some(ModalKind::Downloads);
            for (name, received, done, status) in [
                ("fabric-loader.jar", 100, Some(true), "Completado"),
                ("cobblemon.jar", 42, None, "Descargando"),
                ("resource-pack.zip", 18, Some(false), "La conexión se interrumpió"),
            ] {
                s.downloads.insert(name.into(), state::DownloadItem {
                    received, total: Some(100), done, status: status.into(),
                });
            }
        }),
    ];
    spec.setup = Some(setup);
    spec.on_key = Some(on_key);
    let _ = rune::set_app_icon(include_bytes!("../assets/icon-256.png").to_vec());
    let app = rune::App::new(spec).on_quit(|s, _| {
        if let Some(stop) = s.stop.take() { let _ = stop.send(()); }
        s.presence = None;
    });
    let handle = app.handle();
    app.init(move || {
        let mut state = AppState::default();
        state.handle = Some(handle);
        state
    }).run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Env, Runtime};

    fn runtime() -> Runtime<AppState> {
        let mut rt = Runtime::new(AppState::default(), view, (1536.0, 864.0), Env::new(true, 1.0));
        rt.frame(0.0);
        rt
    }

    fn tap(rt: &mut Runtime<AppState>, key: &str) {
        let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing key {key}"));
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
    }

    #[test]
    fn home_has_dashboard_and_session_controls() {
        let rt = runtime();
        for key in ["sidebar", "header", "hero", "description", "mods-panel", "footer",
                    "launch-button", "mod-Cobbreeding", "mod-Cloth Config API", "variant-select"] {
            assert!(rt.find(key).is_some(), "missing {key}");
        }
    }

    #[test]
    fn navigation_switches_to_settings_and_back() {
        let mut rt = runtime();
        tap(&mut rt, "nav-SETTINGS");
        assert_eq!(rt.state.route, Route::Settings);
        assert!(rt.find("ram-max").is_some());
        tap(&mut rt, "nav-HOME");
        assert_eq!(rt.state.route, Route::Home);
    }

    #[test]
    fn tabs_and_launch_button_are_interactive() {
        let mut rt = runtime();
        tap(&mut rt, "tab-RESOURCE PACKS");
        assert_eq!(rt.state.tab, Tab::Resources);
        assert!(rt.find("mod-ATM x MSD [v5.0].zip").is_some());
        tap(&mut rt, "launch-button");
        assert!(!rt.state.launching);
        assert_eq!(rt.state.modal, None);
    }

    #[test]
    fn search_matches_file_names_and_can_be_cleared() {
        let mut rt = runtime();
        rt.update(|s, _| s.search = rune::TextEdit::new("fabric-2.4.1.jar"));
        rt.frame(1.0 / 60.0);
        assert!(rt.find("mod-Cobbreeding").is_some());
        assert!(rt.find("mod-Cloth Config API").is_none());
        tap(&mut rt, "search-clear");
        assert!(rt.state.search.text.is_empty());
        assert!(rt.find("mod-Cloth Config API").is_some());
        rt.update(|s, _| s.search = rune::TextEdit::new("no-such-file"));
        rt.frame(1.0 / 60.0);
        assert!(rt.find("search-empty").is_some());
        tap(&mut rt, "search-reset");
        assert!(rt.state.search.text.is_empty());
    }

    #[test]
    fn tools_open_and_close_modals() {
        let mut rt = runtime();
        tap(&mut rt, "nav-LOGS");
        assert_eq!(rt.state.modal, Some(ModalKind::Logs));
        assert!(rt.find("logs-empty").is_some());
        tap(&mut rt, "logs-autoscroll");
        assert!(!rt.state.log_autoscroll);
        tap(&mut rt, "modal-close");
        assert_eq!(rt.state.modal, None);
        tap(&mut rt, "nav-DOWNLOADS");
        assert!(rt.find("downloads-empty").is_some());
        tap(&mut rt, "modal-close");
        assert_eq!(rt.state.modal, None);
    }

    #[test]
    fn ram_menu_opens_and_navigation_dismisses_it() {
        let mut rt = runtime();
        tap(&mut rt, "nav-SETTINGS");
        tap(&mut rt, "ram-max-trigger");
        assert_eq!(rt.state.ram_menu, Some(true));
        assert!(rt.find("ram-options").is_some());
        tap(&mut rt, "ram-max-trigger");
        assert_eq!(rt.state.ram_menu, None);
        tap(&mut rt, "ram-min-trigger");
        assert_eq!(rt.state.ram_menu, Some(false));
        tap(&mut rt, "nav-HOME");
        assert_eq!(rt.state.ram_menu, None);
    }

    #[test]
    fn compact_layout_keeps_navigation_and_primary_action() {
        let mut state = AppState::default();
        state.viewport = [900.0, 700.0];
        let mut rt = Runtime::new(state, view, (900.0, 700.0), Env::new(true, 1.0));
        rt.frame(0.0);
        for key in ["nav-HOME", "nav-SETTINGS", "launch-button", "variant-select"] {
            let (x, y) = rt.find(key).unwrap();
            assert!((0.0..900.0).contains(&x) && (0.0..700.0).contains(&y), "{key} is outside viewport: {x},{y}");
        }
        tap(&mut rt, "nav-DOWNLOADS");
        let (x, y) = rt.find("modal-close").unwrap();
        assert!((0.0..900.0).contains(&x) && (0.0..700.0).contains(&y));
    }

    #[test]
    fn busy_primary_action_does_not_start_another_operation() {
        let mut rt = runtime();
        rt.update(|s, _| { s.busy = true; s.api_available = true; });
        rt.frame(1.0 / 60.0);
        tap(&mut rt, "launch-button");
        assert!(rt.state.busy);
        assert!(!rt.state.launching);
        assert_eq!(rt.state.modal, None);
    }

    #[test]
    fn variant_selector_opens_the_modal() {
        let mut rt = runtime();
        tap(&mut rt, "variant-select");
        assert_eq!(rt.state.modal, Some(ModalKind::Variant));
    }
}
