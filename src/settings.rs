use rune::{Cx, El, Icon, ScrollSpec, col, el, icon, row};

use crate::data;
use crate::icons;
use crate::state::AppState;
use crate::theme::{self, ACCENT_SOFT, BORDER, FLOOR, FOREGROUND, MUTED, RAISED, SECONDARY_TEXT};
use crate::widgets::{self, label};

fn section_title(i: &'static Icon, title: &str) -> El {
    row().gap(12.0).center_y().w_full()
        .child(el().size(40.0, 40.0).radius(10.0).center()
            .fill(theme::c(ACCENT_SOFT))
            .child(icon(i, 20.0).color(theme::accent())))
        .child(label(title, 20.0, 700.0, theme::white()).min_w(0.0).wrap())
}

fn ram_dropdown(s: &AppState, is_max: bool) -> El {
    let selected = if is_max { s.max_ram } else { s.min_ram };
    let open = s.ram_menu == Some(is_max);
    let caption = data::RAM_OPTIONS[selected];
    let accessible_label = if is_max { "Memoria máxima de Minecraft" } else { "Memoria mínima de Minecraft" };
    let mut field = col().key(if is_max { "ram-max" } else { "ram-min" })
        .w_full().min_w(0.0).gap(8.0);
    field = field.child(
        row().key(if is_max { "ram-max-trigger" } else { "ram-min-trigger" })
            .role("combobox").label(accessible_label).focusable(true)
            .h(48.0).w_full().px(16.0).radius(10.0)
            .fill(theme::c(RAISED))
            .border(1.0, if open { theme::accent() } else { theme::c(BORDER) })
            .center_y().justify_between()
            .on_press(move |s: &mut AppState| {
                s.ram_menu = if s.ram_menu == Some(is_max) { None } else { Some(is_max) }
            })
            .child(label(caption, 14.0, 700.0, theme::c(FOREGROUND)))
            .child(label(if open { "▴" } else { "▾" }, 13.0, 400.0, theme::c(MUTED)))
    );
    if open {
        let mut opts = col().key("ram-options").role("listbox").label(accessible_label)
            .w_full().min_w(0.0).h(220.0).radius(10.0)
            .fill(theme::c(RAISED)).border(1.0, theme::c(BORDER))
            .scroll(ScrollSpec { stick_bottom: false });
        for (i, value) in data::RAM_OPTIONS.iter().enumerate() {
            let active = i == selected;
            opts = opts.child(
                row().key(format!("ram-{value}")).role("option").label(*value)
                    .checked(active).focusable(true).w_full().h(40.0).min_h(40.0)
                    .px(16.0).center_y().justify_between()
                    .fill(if active { theme::c(ACCENT_SOFT) } else { theme::c(RAISED) })
                    .hover(theme::c(BORDER))
                    .on_press(move |s: &mut AppState| {
                        if is_max { s.max_ram = i } else { s.min_ram = i }
                        s.ram_menu = None;
                        s.save_ram(is_max);
                    })
                    .child(label(value, 14.0, if active { 700.0 } else { 400.0 },
                        if active { theme::accent() } else { theme::c(FOREGROUND) }))
                    .child(label(if active { "✓" } else { "" }, 14.0, 700.0, theme::accent()))
            );
        }
        field = field.child(opts);
    }
    field
}

fn field_block(title: &str, helper: &str, control: El, narrow: bool) -> El {
    let field = el().padding(20.0).min_w(0.0).radius(16.0)
        .fill(theme::c(FLOOR)).border(1.0, theme::c(BORDER))
        .child(
            col().w_full().min_w(0.0).gap(10.0)
                .child(label(title, 14.0, 700.0, theme::c(FOREGROUND)))
                .child(label(helper, 13.0, 400.0, theme::c(MUTED)).w_full().wrap())
                .child(el().w_full().pt(6.0).child(control))
        );
    if narrow { field.w_full() } else { field.grow(1.0) }
}

fn ram_card(s: &AppState, narrow: bool) -> El {
    let blocks = if narrow { col().w_full().gap(16.0) } else { row().w_full().gap(20.0).items_start() }
        .child(field_block("Memoria máxima", "Límite de RAM que Minecraft puede utilizar.",
            ram_dropdown(s, true), narrow))
        .child(field_block("Memoria mínima", "RAM reservada al iniciar el juego.",
            ram_dropdown(s, false), narrow));
    widgets::card(
        col().w_full().min_w(0.0).gap(22.0)
            .child(section_title(&icons::MEMORY, "Memoria de Minecraft"))
            .child(blocks),
        if narrow { 20.0 } else { 28.0 },
    ).w_full().min_w(0.0).radius(16.0)
}

fn behavior_card(s: &AppState, narrow: bool) -> El {
    let field = el().w_full().padding(20.0).radius(16.0)
        .fill(theme::c(FLOOR)).border(1.0, theme::c(BORDER))
        .child(
            row().w_full().gap(20.0).center_y().justify_between()
                .child(col().gap(8.0).grow(1.0).min_w(0.0)
                    .child(label("Ocultar el launcher al iniciar", 14.0, 700.0, theme::c(FOREGROUND)).w_full().wrap())
                    .child(label("La ventana se oculta cuando Minecraft arranca y vuelve a mostrarse al cerrar el juego.",
                        13.0, 400.0, theme::c(SECONDARY_TEXT)).w_full().wrap()))
                .child(widgets::switch(s.hide_on_start, |s: &mut AppState| {
                    s.hide_on_start = !s.hide_on_start;
                    s.save_hide();
                }).role("switch").label("Ocultar el launcher al iniciar Minecraft")
                    .checked(s.hide_on_start).focusable(true))
        );
    widgets::card(
        col().w_full().gap(22.0)
            .child(section_title(&icons::TUNE, "Comportamiento"))
            .child(field),
        if narrow { 20.0 } else { 28.0 },
    ).w_full().min_w(0.0).radius(16.0)
}

pub fn view(s: &AppState, cx: &mut Cx<AppState>) -> El {
    let size = cx.layout("root").map(|r| [r[2], r[3]]).unwrap_or(s.viewport);
    let narrow = size[0] < 1100.0;
    let content = col().key("settings-content").w_full().grow(1.0).min_w(0.0).min_h(0.0).gap(24.0)
        .child(label("Ajustes", if narrow { 28.0 } else { 34.0 }, 700.0, theme::white()))
        .child(ram_card(s, narrow))
        .child(behavior_card(s, narrow))
        .scroll(ScrollSpec { stick_bottom: false });
    cx.caption_color(theme::white());
    widgets::shell(s, size, content, true, cx.chrome_insets())
}
