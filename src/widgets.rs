use rune::{Color, El, Icon, SurfaceFit, col, el, icon, row, text, text_input};

use crate::icons;
use crate::state::{AppState, ModalKind, Route};
use crate::theme::{self, ACCENT_SOFT, BORDER, FLOOR, INK, MUTED, MUTED_LIGHT, RAISED, SURFACE};

pub fn label(value: &str, size: f32, weight: f32, color: Color) -> El {
    text(value).font(theme::style(size, weight)).color(color)
}

pub fn sidebar_width(size: [f32; 2]) -> f32 {
    if size[0] < 1100.0 { 76.0 } else { 200.0 }
}

fn nav_item(i: &'static Icon, key: &str, caption: &str, active: bool, compact: bool, press: impl Fn(&mut AppState) + 'static) -> El {
    let fg = if active { theme::accent() } else { theme::c(MUTED_LIGHT) };
    let mut item = row().key(key).h(46.0).w_full().px(14.0).gap(12.0).center_y()
        .radius(10.0).fill(if active { theme::c(ACCENT_SOFT) } else { theme::c(SURFACE) })
        .hover(theme::c(RAISED)).role("button").label(caption).focusable(true)
        .on_press(press).child(icon(i, 20.0).color(fg));
    if compact { item = item.center(); }
    else { item = item.child(label(caption, 13.0, if active { 600.0 } else { 500.0 }, fg)); }
    item
}

fn sidebar(s: &AppState, size: [f32; 2]) -> El {
    let compact = size[0] < 1100.0;
    let mark = match &s.icon {
        Some(image) => el().size(38.0, 38.0).radius(10.0).clip_rounded(10.0)
            .pixels_fit(image.pixels(), SurfaceFit::Fill),
        None => el().size(38.0, 38.0).radius(12.0).fill(theme::accent()).center()
            .child(label("M", 22.0, 700.0, theme::c(INK))),
    };
    let mut branding = row().key("logo").h(84.0).gap(12.0).center_y().child(mark);
    if compact { branding = branding.center(); }
    else {
        branding = branding.child(col().gap(2.0).grow(1.0).min_w(0.0)
            .child(label("Private Mewing Launcher", 12.0, 700.0, theme::white()).wrap()));
    }
    let mut navigation = col().gap(6.0);
    navigation = navigation
        .child(nav_item(&icons::HOME, "nav-HOME", "Inicio", s.route == Route::Home, compact,
            |s| { s.route = Route::Home; s.modal = None; s.ram_menu = None; }))
        .child(nav_item(&icons::SETTINGS, "nav-SETTINGS", "Ajustes", s.route == Route::Settings, compact,
            |s| { s.route = Route::Settings; s.modal = None; s.ram_menu = None; }))
        .child(nav_item(&icons::FOLDER, "nav-FOLDERS", "Archivos", false, compact, |s| s.open_folder()));
    let mut tools = col().gap(6.0);
    tools = tools
        .child(nav_item(&icons::TERMINAL, "nav-LOGS", "Consola", s.modal == Some(ModalKind::Logs), compact,
            |s| s.modal = Some(ModalKind::Logs)))
        .child(nav_item(&icons::DOWNLOAD, "nav-DOWNLOADS", "Descargas", s.modal == Some(ModalKind::Downloads), compact,
            |s| s.modal = Some(ModalKind::Downloads)));
    let bottom = col().gap(8.0).py(16.0)
        .child(label(&format!("v{}", crate::data::APP_VERSION), if compact { 9.0 } else { 10.0 }, 400.0, theme::c(MUTED)));
    col().key("sidebar").w(sidebar_width(size)).h_full().px(if compact { 10.0 } else { 14.0 })
        .fill(theme::c(SURFACE)).border_sides([0.0, 1.0, 0.0, 0.0], theme::c(BORDER))
        .child(branding).child(navigation).child(el().h(20.0)).child(tools)
        .child(el().grow(1.0)).child(bottom)
}

fn variant_select(s: &AppState, compact: bool) -> El {
    row().key("variant-select").w(if compact { 200.0 } else { 252.0 }).h(40.0).px(12.0).gap(10.0)
        .radius(10.0).fill(theme::c(SURFACE)).border(1.0, theme::c(BORDER)).hover(theme::c(RAISED)).center_y()
        .role("button").label("Cambiar variante del pack").focusable(true)
        .on_press(|s: &mut AppState| s.modal = Some(ModalKind::Variant))
        .child(icon(&icons::EXTENSION, 15.0).color(theme::accent()))
        .child(label(&s.event.title, 12.0, 500.0, theme::white()).grow(1.0).min_w(0.0).truncate())
        .child(label("▾", 12.0, 400.0, theme::c(MUTED_LIGHT)))
}

fn update_button(s: &AppState) -> El {
    let version = s.update.as_ref().map(|u| u.version.as_str()).unwrap_or("");
    let caption = if s.updating { "Descargando…".to_string() } else { format!("Actualizar v{version}") };
    el().key("update-button").h(40.0).px(14.0).radius(10.0)
        .fill(theme::accent().alpha(0.14)).border(1.0, theme::accent().alpha(0.45))
        .hover(theme::accent().alpha(0.22))
        .disabled(s.updating).opacity(if s.updating { 0.65 } else { 1.0 })
        .role("button").label("Instalar actualización").focusable(true).center()
        .on_press(|s: &mut AppState| s.install_update())
        .child(row().gap(8.0).center_y()
            .child(icon(&icons::REFRESH, 14.0).color(theme::accent()))
            .child(label(&caption, 12.0, 600.0, theme::accent())))
}

fn header(s: &AppState, size: [f32; 2]) -> El {
    let compact = size[0] < 900.0;
    let mut bar = row().key("header").w_full().h(72.0).px(if compact { 18.0 } else { 28.0 }).gap(14.0).center_y()
        .child(el().grow(1.0));
    if s.update.is_some() { bar = bar.child(update_button(s)); }
    bar.child(variant_select(s, compact))
}

fn username_field(s: &AppState, compact: bool) -> El {
    col().key("username-field").w(if compact { 170.0 } else { 220.0 }).gap(6.0)
        .child(row().h(42.0).w_full().radius(9.0).fill(theme::c(RAISED)).border(1.0, theme::c(BORDER)).center_y()
            .child(el().w(36.0).center().child(icon(&icons::HOME, 16.0).color(theme::accent())))
            .child(text_input(&s.username, theme::style(13.0, 500.0)).key("username-input")
                .label("Nombre de jugador").single_line().grow(1.0).min_w(0.0).pr(12.0).py(12.0).h_full().center_y()
                .color(theme::white()).caret(theme::accent(), theme::accent().alpha(0.2))
                .placeholder("Tu nombre", theme::c(MUTED))
                .on_edit(|s: &mut AppState| &mut s.username)
                .on_input_change(|s: &mut AppState, _| s.save_username())))
}

fn launch_button(s: &AppState, compact: bool) -> El {
    let caption = if s.loading { "Cargando pack…" } else if s.busy { "Procesando…" }
        else if s.launching { "Detener juego" } else if !s.api_available { "Reintentar conexión" }
        else if s.action_caption() == "Launch" { "Jugar ahora" }
        else if s.loader_installed { "Sincronizar pack" } else { "Instalar y preparar" };
    let glyph = if s.launching { &icons::STOP } else if !s.api_available { &icons::REFRESH }
        else if s.action_caption() == "Launch" { &icons::PLAY } else { &icons::DOWNLOAD };
    el().key("launch-button").w(if compact { 200.0 } else { 240.0 }).h(52.0).radius(12.0)
        .fill(if s.launching { theme::c(theme::DANGER) } else { theme::accent() })
        .hover(if s.launching { theme::c("#FFADBA") } else { theme::c("#89EDD0") })
        .disabled(s.busy || s.loading).opacity(if s.busy || s.loading { 0.55 } else { 1.0 })
        .role("button").label(caption).focusable(true).center()
        .on_press(|s: &mut AppState| {
            if !s.api_available && !s.launching { s.load_event(s.settings.selectedEventId.clone()); }
            else { s.primary_action(); }
        })
        .child(row().gap(10.0).center_y()
            .child(icon(glyph, 18.0).color(theme::c(INK)))
            .child(label(caption, 14.0, 700.0, theme::c(INK))))
}

fn footer(s: &AppState, size: [f32; 2]) -> El {
    let compact = size[0] < 900.0;
    row().key("footer").w_full().h(96.0).px(if compact { 18.0 } else { 28.0 }).gap(24.0).center_y()
        .fill(theme::c(SURFACE)).border_sides([1.0, 0.0, 0.0, 0.0], theme::c(BORDER))
        .child(username_field(s, compact))
        .child(el().grow(1.0))
        .child(launch_button(s, compact))
}

pub fn shell(s: &AppState, size: [f32; 2], content: El, show_footer: bool) -> El {
    let mut workspace = col().key("workspace").grow(1.0).min_w(0.0).min_h(0.0)
        .child(header(s, size))
        .child(el().key("slot").grow(1.0).min_h(0.0).min_w(0.0)
            .px(if size[0] < 1100.0 { 18.0 } else { 28.0 }).pt(8.0).pb(24.0).child(content));
    if show_footer { workspace = workspace.child(footer(s, size)); }
    let mut root = col().key("root").w_full().h_full().fill(theme::c(FLOOR))
        .child(row().key("body").w_full().grow(1.0).min_h(0.0).child(sidebar(s, size)).child(workspace));
    if let Some(modal) = crate::home::modal(s, size) { root = root.child(modal); }
    root
}

pub fn card(content: El, padding: f32) -> El {
    el().w_full().padding(padding).radius(16.0).fill(theme::c(SURFACE))
        .border(1.0, theme::c(BORDER)).child(content)
}

pub fn chip(content: El, bg: Color, border: Color, padding_x: f32, padding_y: f32) -> El {
    el().px(padding_x).py(padding_y).radius(8.0).fill(bg).border(1.0, border).child(content)
}

pub fn modal_shell(panel: El) -> El {
    el().key("modal-scrim").absolute(0.0, 0.0).w_full().h_full().fill(theme::c(FLOOR).alpha(0.86))
        .on_press(|s: &mut AppState| s.modal = None)
        .child(el().w_full().h_full().center().on_press(|_: &mut AppState| {})
            .child(panel.radius(18.0).clip_rounded(18.0).shadow(14.0, 50.0, Color::BLACK.alpha(0.4))))
}

pub fn switch(on: bool, press: fn(&mut AppState)) -> El {
    let (w, h, k) = (46.0, 26.0, 18.0);
    let inset = (h - k) / 2.0;
    el().key("hide-switch").role("switch").label("Ocultar al iniciar Minecraft").checked(on).focusable(true)
        .size(w, h).radius(h / 2.0).fill(if on { theme::accent() } else { theme::c(BORDER) })
        .hover(if on { theme::c("#89EDD0") } else { theme::c("#354B61") }).on_press(press)
        .child(el().key("hide-knob").size(k, k).radius(k / 2.0)
            .absolute(if on { w - k - inset } else { inset }, inset)
            .fill(if on { theme::c(INK) } else { theme::c(MUTED_LIGHT) }))
}
