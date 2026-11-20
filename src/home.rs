use rune::{Cx, El, Icon, ScrollSpec, SurfaceFit, col, el, icon, row, text};

use crate::{data, icons};
use crate::state::{AppState, ModalKind, Tab};
use crate::theme::{self, ACCENT_SOFT, BORDER, FLOOR, MUTED, MUTED_LIGHT, RAISED, SECONDARY_TEXT, SURFACE};
use crate::widgets::{self, label};

fn badge(caption: &str, glyph: &'static Icon) -> El {
    widgets::chip(row().gap(7.0).center_y()
        .child(icon(glyph, 13.0).color(theme::accent()))
        .child(label(caption, 11.0, 500.0, theme::white())),
        theme::c(FLOOR).alpha(0.88), theme::c(BORDER), 10.0, 7.0)
}

fn hero(s: &AppState, height: f32, fill: bool) -> El {
    let mut body = el().key("hero").w_full().radius(18.0).clip_rounded(18.0)
        .fill(theme::c(RAISED)).border(1.0, theme::c(BORDER))
        .on_hover_change(|s: &mut AppState, inside, _: &mut rune::Tx| {
            s.hero_hover = inside;
            if !inside { s.hero_cursor = None; }
        })
        .on_pointer_move_local(|s: &mut AppState, p, _: &mut rune::Tx| {
            s.hero_cursor = Some([p.x, p.y]);
        });
    body = if fill { body.grow(1.0).min_h(240.0) } else { body.h(height) };
    if let Some(image) = &s.hero {
        body = body.child(el().key("hero-image").w_full().h_full()
            .pixels_fit(image.pixels(), SurfaceFit::Fill).opacity(0.7))
            .child(el().key("hero-metal").absolute(0.0, 0.0).w_full().h_full()
                .effect(crate::fx::HERO_METAL))
            .child(el().key("hero-wave").absolute(0.0, 0.0).w_full().h_full()
                .effect(crate::fx::HERO_WAVE).effect_backdrop(true));
    }
    body.child(el().key("hero-shade").absolute(0.0, 0.0).w_full().h_full().fill(theme::c(FLOOR).alpha(0.4)))
        .child(col().key("hero-content").absolute(0.0, 0.0).w_full().h_full().padding(26.0).justify_end()
            .child(col().gap(12.0)
                .child(label(&s.event.title, if fill || height > 270.0 { 38.0 } else { 30.0 }, 700.0, theme::white()).wrap())
                .child(row().gap(8.0).flex_wrap()
                    .child(badge(&format!("Minecraft {}", s.event.loader_version()), &icons::TERMINAL))
                    .child(badge(if s.event.is_fabric { "Fabric" } else { "Forge" }, &icons::BOLT))
                    .child(badge(&format!("Pack v{}", s.event.version), &icons::EXTENSION)))))
}

fn stat(glyph: &'static Icon, value: &str, caption: &str) -> El {
    row().grow(1.0).min_w(0.0).padding(14.0).gap(10.0).center_y().radius(12.0)
        .fill(theme::c(SURFACE)).border(1.0, theme::c(BORDER))
        .child(icon(glyph, 18.0).color(theme::accent()))
        .child(col().min_w(0.0).gap(4.0)
            .child(label(value, 17.0, 600.0, theme::white()).truncate())
            .child(label(caption, 10.0, 400.0, theme::c(MUTED))))
}

fn left_column(s: &AppState, width: f32, height: f32, fill: bool) -> El {
    let text_width = (width - 42.0).max(100.0);
    col().key("left-column").w(width).min_w(0.0).gap(16.0)
        .child(hero(s, if height > 760.0 { 290.0 } else { 240.0 }, fill))
        .child(row().key("pack-stats").w_full().gap(10.0)
            .child(stat(&icons::EXTENSION, &s.event.mods.len().to_string(), "Mods"))
            .child(stat(&icons::FOLDER, &s.event.resource_packs.len().to_string(), "Resource packs"))
            .child(stat(&icons::MEMORY, data::RAM_OPTIONS[s.max_ram], "RAM máxima")))
        .child(widgets::card(col().key("description").w(text_width).gap(12.0)
            .child(label("Acerca de este evento", 15.0, 600.0, theme::white()))
            .child(text(if s.event.description.trim().is_empty() { "Este pack todavía no tiene una descripción." } else { &s.event.description })
                .font(theme::style(13.0, 400.0).lh(21.0)).color(theme::c(SECONDARY_TEXT)).wrap().w(text_width)), 20.0))
}

fn tab_button(key: &str, caption: &str, glyph: &'static Icon, active: bool, grow: f32, tab: Tab) -> El {
    row().key(key).grow(grow).h(36.0).gap(6.0).px(8.0).center().radius(8.0)
        .fill(if active { theme::c(RAISED) } else { theme::c(FLOOR) }).hover(theme::c(RAISED))
        .role("button").label(caption).focusable(true)
        .on_press(move |s: &mut AppState| s.tab = tab)
        .child(icon(glyph, 14.0).color(if active { theme::accent() } else { theme::c(MUTED) }))
        .child(label(caption, 12.0, 600.0, if active { theme::white() } else { theme::c(MUTED) }))
}

fn mod_row(s: &AppState, name: &str, file_name: &str, installed: bool, thumb: Option<&rune::Image>, index: usize) -> El {
    let thumbnail = match thumb {
        Some(image) => el().key(format!("thumb-{name}")).size(36.0, 36.0).radius(9.0)
            .clip_rounded(9.0).pixels_fit(image.pixels(), SurfaceFit::Fill),
        None => el().key(format!("thumb-{name}")).size(36.0, 36.0).radius(9.0).fill(theme::c(RAISED)).center()
            .child(icon(match s.tab { Tab::Mods => &icons::EXTENSION, Tab::Shaders => &icons::BOLT, Tab::Resources => &icons::FOLDER }, 18.0).color(theme::c(MUTED))),
    };
    let tab = s.tab;
    let disabled = s.busy || s.loading || s.launching || !s.api_available;
    row().key(format!("mod-{name}")).w_full().h(72.0).px(4.0).gap(12.0).center_y().radius(9.0)
        .hover(theme::c(RAISED))
        .child(thumbnail)
        .child(col().gap(5.0).grow(1.0).min_w(0.0)
            .child(label(name, 13.0, 600.0, theme::white()).truncate())
            .child(label(file_name, 10.0, 400.0, theme::c(MUTED)).truncate()))
        .child(el().size(7.0, 7.0).radius(4.0).fill(if installed { theme::accent() } else { theme::c(BORDER) })
            .label(if installed { "Instalado" } else { "Pendiente de instalar" }))
        .child(el().key(format!("reinstall-{index}")).size(30.0, 30.0).radius(8.0).center()
            .fill(theme::c(RAISED)).hover(theme::c("#2B4053")).disabled(disabled).opacity(if disabled { 0.4 } else { 1.0 })
            .role("button").label(&format!("{} {name}", if installed { "Reinstalar" } else { "Instalar" })).focusable(true)
            .on_press(move |s: &mut AppState| s.reinstall(tab, index))
            .child(icon(if installed { &icons::REFRESH } else { &icons::DOWNLOAD }, 14.0).color(theme::c(MUTED_LIGHT))))
}

fn filtered_mods(s: &AppState) -> Vec<(usize, &str, &str, Option<&rune::Image>)> {
    let query = s.search.text.trim().to_lowercase();
    let (assets, thumbnails): (&[data::ApiMod], &[Option<rune::Image>]) = match s.tab {
        Tab::Mods => (&s.event.mods, &s.thumbnails),
        Tab::Shaders => (s.event.shader_packs_list(), &[]),
        Tab::Resources => (&s.event.resource_packs, &s.resource_thumbnails),
    };
    assets.iter().enumerate()
        .filter(|(_, m)| query.is_empty() || m.id.to_lowercase().contains(&query) || m.file_name.to_lowercase().contains(&query))
        .map(|(i, m)| (i, m.id.as_str(), m.file_name.as_str(), thumbnails.get(i).and_then(|image| image.as_ref())))
        .collect()
}

pub fn panel(s: &AppState, narrow: bool) -> El {
    let items = filtered_mods(s);
    let total = match s.tab {
        Tab::Mods => s.event.mods.len(),
        Tab::Shaders => s.event.shader_packs_list().len(),
        Tab::Resources => s.event.resource_packs.len(),
    };
    let mut list = col().key("mod-list").w_full().grow(1.0).min_h(0.0).px(18.0).pb(18.0)
        .scroll(ScrollSpec { stick_bottom: false });
    if items.is_empty() {
        list = list.child(col().key("search-empty").h(210.0).w_full().center().gap(12.0)
            .child(el().size(48.0, 48.0).radius(15.0).fill(theme::c(RAISED)).center()
                .child(icon(&icons::SEARCH, 22.0).color(theme::c(MUTED))))
            .child(label("No hay coincidencias", 15.0, 600.0, theme::white()))
            .child(label("Prueba otro nombre o archivo.", 12.0, 400.0, theme::c(MUTED)))
            .child(el().key("search-reset").py(6.0).px(12.0).radius(8.0).hover(theme::c(RAISED))
                .role("button").label("Limpiar búsqueda").focusable(true)
                .on_press(|s: &mut AppState| s.search = rune::TextEdit::new(""))
                .child(label("Limpiar búsqueda", 12.0, 600.0, theme::accent()))));
    } else {
        let kind = match s.tab { Tab::Mods => "mods", Tab::Shaders => "shader_packs", Tab::Resources => "resource_packs" };
        let folder = crate::paths::asset_folder(kind, &crate::paths::instance_dir(&s.event.id));
        for (index, name, file_name, thumb) in &items {
            let installed = crate::paths::safe_file_name(file_name) && folder.join(file_name).is_file();
            list = list.child(mod_row(s, name, file_name, installed, *thumb, *index));
        }
    }
    let mut search = row().key("search-field").w_full().h(42.0).radius(9.0)
        .fill(theme::c(FLOOR)).border(1.0, theme::c(BORDER)).center_y()
        .child(el().w(36.0).center().child(icon(&icons::SEARCH, 15.0).color(theme::c(MUTED))))
        .child(rune::text_input(&s.search, theme::style(12.0, 400.0)).key("mods-search")
            .label("Buscar en el contenido del pack").single_line().grow(1.0).min_w(0.0).py(12.0).h_full().center_y()
            .color(theme::white()).caret(theme::accent(), theme::accent().alpha(0.2))
            .placeholder(match s.tab {
                Tab::Mods => "Buscar mods o archivos…",
                Tab::Shaders => "Buscar shaders o archivos…",
                Tab::Resources => "Buscar resource packs o archivos…",
            }, theme::c(MUTED))
            .on_edit(|s: &mut AppState| &mut s.search));
    if !s.search.text.is_empty() {
        search = search.child(el().key("search-clear").size(30.0, 30.0).center().radius(8.0).hover(theme::c(RAISED))
            .role("button").label("Limpiar búsqueda").focusable(true)
            .on_press(|s: &mut AppState| s.search = rune::TextEdit::new(""))
            .child(icon(&icons::CLOSE, 13.0).color(theme::c(MUTED_LIGHT))));
    }
    let mut tabs = row().key("tabs").w_full().padding(4.0).gap(4.0).radius(10.0).fill(theme::c(FLOOR))
        .child(tab_button("tab-MODS", "Mods", &icons::EXTENSION, s.tab == Tab::Mods, 1.0, Tab::Mods));
    if !s.event.shader_packs_list().is_empty() {
        tabs = tabs.child(tab_button("tab-SHADERS", "Shaders", &icons::BOLT, s.tab == Tab::Shaders, 1.0, Tab::Shaders));
    }
    tabs = tabs.child(tab_button("tab-RESOURCE PACKS", "Resource packs", &icons::FOLDER, s.tab == Tab::Resources, 1.35, Tab::Resources));
    let mut root = col().key("mods-panel").min_h(0.0).radius(16.0).clip_rounded(16.0)
        .fill(theme::c(SURFACE)).border(1.0, theme::c(BORDER));
    root = if narrow { root.h(480.0).w_full() } else { root.w(400.0).h_full() };
    root.child(col().padding(18.0).gap(14.0)
        .child(row().gap(10.0).center_y()
            .child(label("Contenido del pack", 16.0, 600.0, theme::white()).grow(1.0))
            .child(label(&format!("{} / {total}", items.len()), 11.0, 500.0, theme::c(MUTED))))
        .child(tabs)
        .child(search))
        .child(el().h(1.0).w_full().fill(theme::c(BORDER))).child(list)
}

fn modal_header(title: &str, subtitle: &str) -> El {
    let mut text = col().grow(1.0).min_w(0.0).gap(7.0)
        .child(label(title, 20.0, 600.0, theme::white()));
    if !subtitle.is_empty() {
        text = text.child(label(subtitle, 11.0, 400.0, theme::c(MUTED)).wrap());
    }
    row().h(if subtitle.is_empty() { 68.0 } else { 88.0 }).px(24.0).gap(16.0).center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], theme::c(BORDER))
        .child(text)
        .child(el().key("modal-close").size(32.0, 32.0).radius(9.0).center().hover(theme::c(RAISED))
            .role("button").label("Cerrar diálogo").focusable(true)
            .on_press(|s: &mut AppState| s.modal = None)
            .child(icon(&icons::CLOSE, 17.0).color(theme::c(MUTED_LIGHT))))
}

fn modal_panel(key: &str, size: [f32; 2], width: f32, height: f32) -> El {
    col().key(key).w(width.min((size[0] - 48.0).max(280.0)))
        .h(height.min((size[1] - 48.0).max(250.0))).min_h(0.0)
        .fill(theme::c(SURFACE)).border(1.0, theme::c(BORDER))
}

fn logs_modal(s: &AppState, size: [f32; 2]) -> El {
    let mut lines = col().key("log-lines").gap(7.0).padding(20.0).fill(theme::c(FLOOR));
    if s.logs.is_empty() {
        lines = lines.child(col().key("logs-empty").h(210.0).center().gap(14.0)
            .child(icon(&icons::TERMINAL, 30.0).color(theme::c(MUTED)))
            .child(label("La consola está en silencio", 16.0, 600.0, theme::white()))
            .child(label("La actividad y los mensajes de Minecraft aparecerán aquí.", 12.0, 400.0, theme::c(MUTED)).wrap()));
    } else {
        for (index, line) in s.logs.iter().enumerate() {
            lines = lines.child(row().gap(16.0).items_start()
                .child(label(&format!("{:03}", index + 1), 10.0, 400.0, theme::c(MUTED)).w(30.0))
                .child(label(line, 12.0, 400.0, theme::c(SECONDARY_TEXT)).wrap().grow(1.0).min_w(0.0)));
        }
    }
    let controls = row().h(60.0).px(24.0).gap(10.0).center_y()
        .child(el().key("logs-copy").px(14.0).py(9.0).radius(8.0).fill(theme::c(RAISED)).hover(theme::c(BORDER))
            .role("button").label("Copiar logs").focusable(true)
            .on_press_tx(|s: &mut AppState, tx| tx.copy(s.logs.join("\n")))
            .child(label("Copiar", 12.0, 500.0, theme::white())))
        .child(el().key("logs-clear").px(14.0).py(9.0).radius(8.0).hover(theme::c(RAISED))
            .role("button").label("Limpiar logs").focusable(true)
            .on_press(|s: &mut AppState| s.logs.clear()).child(label("Limpiar", 12.0, 500.0, theme::c(MUTED_LIGHT))))
        .child(el().grow(1.0))
        .child(el().key("logs-autoscroll").px(10.0).py(9.0).radius(8.0).hover(theme::c(RAISED))
            .role("button").label("Alternar seguimiento de logs").focusable(true)
            .on_press(|s: &mut AppState| s.log_autoscroll = !s.log_autoscroll)
            .child(label(if s.log_autoscroll { "Seguimiento: activo" } else { "Seguimiento: pausado" }, 11.0, 500.0, theme::accent())));
    widgets::modal_shell(modal_panel("logs-modal", size, 860.0, 600.0)
        .child(modal_header("Consola", ""))
        .child(lines.grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: s.log_autoscroll })).child(controls))
}

fn downloads_modal(s: &AppState, size: [f32; 2]) -> El {
    let active = s.downloads.values().filter(|d| d.done.is_none()).count();
    let failed = s.downloads.values().filter(|d| d.done == Some(false)).count();
    let done = s.downloads.values().filter(|d| d.done == Some(true)).count();
    let mut content = col().grow(1.0).min_h(0.0).padding(22.0).gap(14.0).scroll(ScrollSpec { stick_bottom: false });
    if s.downloads.is_empty() {
        content = content.child(col().key("downloads-empty").h(230.0).w_full().center().gap(14.0)
            .child(el().size(56.0, 56.0).radius(17.0).fill(theme::c(ACCENT_SOFT)).center()
                .child(icon(&icons::DOWNLOAD, 25.0).color(theme::accent())))
            .child(label("Sin descargas en curso", 17.0, 600.0, theme::white())));
    } else {
        content = content.child(row().gap(10.0)
            .child(stat(&icons::DOWNLOAD, &active.to_string(), "En curso"))
            .child(stat(&icons::CHECK, &done.to_string(), "Completadas"))
            .child(stat(&icons::CLOSE, &failed.to_string(), "Fallidas")));
        for (name, item) in s.downloads.iter().rev().take(80) {
            let fraction = if item.done == Some(true) { Some(1.0) } else {
                item.total.filter(|total| *total > 0).map(|total| (item.received as f32 / total as f32).clamp(0.0, 1.0))
            };
            let progress = fraction.map(|value| format!("{:.0}%", value * 100.0))
                .unwrap_or_else(|| format!("{:.1} MB", item.received as f64 / 1_048_576.0));
            let color = if item.done == Some(false) { theme::c(theme::DANGER) } else { theme::accent() };
            let mut track = row().h(4.0).w_full().radius(2.0).clip_rounded(2.0).fill(theme::c(BORDER));
            if let Some(fraction) = fraction.filter(|value| *value > 0.0) {
                track = track.child(el().h_full().grow(fraction).fill(color))
                    .child(el().h_full().grow(1.0 - fraction));
            }
            content = content.child(col().padding(14.0).gap(9.0).radius(10.0).fill(theme::c(FLOOR))
                .child(row().gap(10.0).center_y()
                    .child(label(name, 12.0, 600.0, theme::white()).grow(1.0).min_w(0.0).truncate())
                    .child(label(&progress, 11.0, 600.0, color)))
                .child(track)
                .child(label(&item.status, 11.0, 400.0, if item.done == Some(false) { color } else { theme::c(MUTED) }).wrap()));
        }
    }
    widgets::modal_shell(modal_panel("downloads-modal", size, 700.0, 520.0)
        .child(modal_header("Descargas", "")).child(content))
}

fn confirm_sync_modal(s: &AppState, size: [f32; 2]) -> El {
    let mut files = col().grow(1.0).min_h(0.0).gap(8.0).padding(14.0).radius(10.0).fill(theme::c(FLOOR))
        .scroll(ScrollSpec { stick_bottom: false });
    for removal in &s.sync_plan.removals {
        files = files.child(label(&format!("{} / {}", removal.kind, removal.file_name), 12.0, 400.0, theme::c(SECONDARY_TEXT)).wrap());
    }
    for file in &s.legacy_files { files = files.child(label(file, 12.0, 400.0, theme::c(SECONDARY_TEXT)).wrap()); }
    let warning = if s.legacy_files.is_empty() {
        "Se eliminarán estos archivos porque no pertenecen al pack seleccionado, incluidos los mods añadidos manualmente. Revisa la lista antes de continuar."
    } else {
        "Se moverán los datos anteriores a la instancia de este evento. Los archivos de destino existentes no se sobrescribirán; el runtime y los ajustes seguirán compartidos."
    };
    widgets::modal_shell(modal_panel("confirm-sync-modal", size, 700.0, 500.0)
        .child(modal_header("Revisar cambios", "Tu instancia necesita sincronizarse con el pack seleccionado."))
        .child(col().grow(1.0).min_h(0.0).padding(22.0).gap(16.0)
            .child(label(warning, 13.0, 500.0, theme::c(theme::WARNING)).wrap()).child(files)
            .child(row().gap(12.0).justify_end()
                .child(el().key("sync-cancel").px(18.0).h(42.0).radius(10.0).center().fill(theme::c(RAISED)).hover(theme::c(BORDER))
                    .role("button").label("Cancelar sincronización").focusable(true)
                    .on_press(|s: &mut AppState| s.modal = None).child(label("Cancelar", 12.0, 600.0, theme::white())))
                .child(el().key("sync-confirm").px(18.0).h(42.0).radius(10.0).center().fill(theme::c(theme::WARNING)).hover(theme::c("#FFE0AC"))
                    .role("button").label("Confirmar y sincronizar").focusable(true)
                    .on_press(|s: &mut AppState| s.confirm_sync()).child(label("Confirmar cambios", 12.0, 600.0, theme::c(theme::INK)))))))
}

fn variant_modal(s: &AppState, size: [f32; 2]) -> El {
    let family = data::variant_family(&s.events, &s.event);
    let mut options = col().key("variant-options").grow(1.0).min_h(0.0).gap(10.0).padding(22.0)
        .scroll(ScrollSpec { stick_bottom: false });
    for (index, event) in family.iter().enumerate() {
        let active = event.id == s.event.id;
        let id = event.id.clone();
        options = options.child(row().key(format!("variant-{index}")).w_full().h(76.0).px(16.0).gap(14.0).center_y().radius(12.0)
            .fill(if active { theme::c(ACCENT_SOFT) } else { theme::c(FLOOR) })
            .border(1.0, if active { theme::accent() } else { theme::c(BORDER) }).hover(theme::c(RAISED))
            .disabled(s.busy || s.loading || s.launching).role("button").label(&event.title).focusable(true)
            .on_press(move |s: &mut AppState| s.switch_variant(&id))
            .child(icon(&icons::EXTENSION, 22.0).color(theme::accent()))
            .child(col().grow(1.0).min_w(0.0).gap(6.0)
                .child(label(&event.title, 14.0, 600.0, theme::white()).truncate())
                .child(label(&format!("v{} · {} mods · {} resource packs", event.version, event.mods.len(), event.resource_packs.len()), 11.0, 400.0, theme::c(MUTED_LIGHT))))
            .child(icon(if active { &icons::CHECK } else { &icons::PLAY }, 17.0).color(if active { theme::accent() } else { theme::c(MUTED) })));
    }
    widgets::modal_shell(modal_panel("variant-modal", size, 620.0, 400.0)
        .child(modal_header("Elige el evento", "")).child(options))
}

pub fn modal(s: &AppState, size: [f32; 2]) -> Option<El> {
    match s.modal {
        Some(ModalKind::Logs) => Some(logs_modal(s, size)),
        Some(ModalKind::Downloads) => Some(downloads_modal(s, size)),
        Some(ModalKind::Variant) => Some(variant_modal(s, size)),
        Some(ModalKind::ConfirmSync) => Some(confirm_sync_modal(s, size)),
        None => None,
    }
}

pub fn view(s: &AppState, cx: &mut Cx<AppState>) -> El {
    let size = cx.layout("root").map(|r| [r[2], r[3]]).unwrap_or(s.viewport);
    let narrow = size[0] < 1250.0;
    let padding = if size[0] < 1100.0 { 36.0 } else { 56.0 };
    let width = (size[0] - widgets::sidebar_width(size) - padding).max(260.0);
    let mut content = col().key("home-content").w_full().grow(1.0).min_h(0.0).gap(22.0);
    if narrow {
        content = content.child(left_column(s, width, size[1], false)).child(panel(s, true))
            .scroll(ScrollSpec { stick_bottom: false });
    } else {
        let fill = size[1] >= 640.0;
        let mut left = left_column(s, width - 422.0, size[1], fill).h_full().min_h(0.0);
        if !fill { left = left.scroll(ScrollSpec { stick_bottom: false }); }
        content = content.child(row().w_full().grow(1.0).min_h(0.0).gap(22.0)
            .child(left)
            .child(panel(s, false)));
    }
    cx.caption_color(theme::white());
    widgets::shell(s, size, content, true, cx.chrome_insets())
}
