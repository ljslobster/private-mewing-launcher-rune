use rune::Icon;

pub static HOME: Icon = Icon {
    name: "home",
    view: 24.0,
    body: r#"<path d="M3 10.2 12 3l9 7.2V20a1.8 1.8 0 0 1-1.8 1.8H4.8A1.8 1.8 0 0 1 3 20V10.2z" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"/><path d="M9.3 21.5v-7.2h5.4v7.2" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"/>"#,
};

pub static SETTINGS: Icon = Icon {
    name: "settings",
    view: 24.0,
    body: r#"<circle cx="12" cy="12" r="3.1" fill="none" stroke="currentColor" stroke-width="2"/><path d="M19.4 14.9a1.7 1.7 0 0 0 .34 1.88l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.88-.34 1.7 1.7 0 0 0-1.03 1.56V21a2 2 0 0 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.55 1.7 1.7 0 0 0-1.88.34l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06a1.7 1.7 0 0 0 .34-1.88 1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 0 1 0-4h.1a1.7 1.7 0 0 0 1.55-1.1 1.7 1.7 0 0 0-.34-1.88l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06a1.7 1.7 0 0 0 1.88.34H9a1.7 1.7 0 0 0 1.03-1.56V3a2 2 0 0 1 4 0v.1a1.7 1.7 0 0 0 1.03 1.56 1.7 1.7 0 0 0 1.88-.34l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.88V9a1.7 1.7 0 0 0 1.56 1.03H21a2 2 0 0 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1.03z" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"/>"#,
};

pub static FOLDER: Icon = Icon {
    name: "folder",
    view: 24.0,
    body: r#"<path d="M21.5 19.2a1.8 1.8 0 0 1-1.8 1.8H4.3a1.8 1.8 0 0 1-1.8-1.8V4.8C2.5 3.8 3.3 3 4.3 3h5l2 3h8.4a1.8 1.8 0 0 1 1.8 1.8V19.2z" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"/>"#,
};

pub static IMAGE: Icon = Icon {
    name: "image",
    view: 24.0,
    body: r#"<rect x="3" y="3" width="18" height="18" rx="2" fill="none" stroke="currentColor" stroke-width="2"/><circle cx="8.5" cy="8.5" r="1.6" fill="none" stroke="currentColor" stroke-width="1.8"/><path d="M21 15.5 16 11 5 21" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"/>"#,
};

pub static DOWNLOAD: Icon = Icon {
    name: "download",
    view: 24.0,
    body: r#"<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><path d="m7 10 5 5 5-5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/><path d="M12 15V3" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>"#,
};

pub static TERMINAL: Icon = Icon {
    name: "terminal",
    view: 24.0,
    body: r#"<path d="m4 17 6-5-6-5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/><path d="M12 19h8" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>"#,
};

pub static EXTENSION: Icon = Icon {
    name: "extension",
    view: 24.0,
    body: r#"<path d="M20.5 11H19V7a2 2 0 0 0-2-2h-3V3.5A2.5 2.5 0 0 0 9.5 1 2.5 2.5 0 0 0 7 3.5V5H4a2 2 0 0 0-2 2v3.8h.5A2.7 2.7 0 1 1 2.5 14.3H2V19a2 2 0 0 0 2 2h3v-1.5a2.5 2.5 0 0 1 5 0V21h3a2 2 0 0 0 2-2v-4h.5a2.7 2.7 0 1 0 0-5.3z"/>"#,
};

#[allow(dead_code)]
pub static BOLT: Icon = Icon {
    name: "bolt",
    view: 24.0,
    body: r#"<path d="M13 2 4 13.5h6.2L9 22l10.5-12h-6.4L13 2z"/>"#,
};

pub static SEARCH: Icon = Icon {
    name: "search",
    view: 24.0,
    body: r#"<circle cx="10.5" cy="10.5" r="6.5" fill="none" stroke="currentColor" stroke-width="2"/><path d="m20 20-4.4-4.4" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>"#,
};

pub static REFRESH: Icon = Icon {
    name: "refresh",
    view: 24.0,
    body: r#"<path d="M20.5 11a8.5 8.5 0 1 0-2.4 6.2" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><path d="M20.5 5v6h-6" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

#[allow(dead_code)]
pub static UPDATE: Icon = Icon {
    name: "update",
    view: 24.0,
    body: r#"<path d="M21 12a9 9 0 1 1-2.6-6.4" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><path d="M21 4v5h-5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/><path d="M12 7v5l3.5 2" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

#[allow(dead_code)]
pub static PLAY: Icon = Icon {
    name: "play",
    view: 24.0,
    body: r#"<path d="M7 4.5v15l13-7.5L7 4.5z"/>"#,
};

pub static STOP: Icon = Icon {
    name: "stop",
    view: 24.0,
    body: r#"<rect x="5" y="5" width="14" height="14" rx="1"/>"#,
};

#[allow(dead_code)]
pub static ROCKET: Icon = Icon {
    name: "rocket",
    view: 24.0,
    body: r#"<path d="M13.6 3.2c3-.8 6.2.4 7.2 3.4-1 3-3.6 5-6.6 5.4l-4.2-4.2c.4-2.6 1.9-4.2 3.6-4.6z" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"/><path d="m9.8 7.8-4.3 4.3 6.4 6.4 4.3-4.3" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"/><path d="m5.5 12.1-3 1 1 3 3-1zM11.8 18.4l-1 3 3-1 1-3z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/><circle cx="15.5" cy="8.5" r="1.5" fill="none" stroke="currentColor" stroke-width="1.6"/>"#,
};

pub static TUNE: Icon = Icon {
    name: "tune",
    view: 24.0,
    body: r#"<path d="M4 21v-7M4 10V3M12 21v-9M12 8V3M20 21v-5M20 12V3M1 14h6M9 8h6M17 16h6" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>"#,
};

pub static MEMORY: Icon = Icon {
    name: "memory",
    view: 24.0,
    body: r#"<rect x="6" y="6" width="12" height="12" rx="2" fill="none" stroke="currentColor" stroke-width="2"/><path d="M9 2v4M15 2v4M9 18v4M15 18v4M2 9h4M2 15h4M18 9h4M18 15h4" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>"#,
};

pub static CLOSE: Icon = Icon {
    name: "close",
    view: 24.0,
    body: r#"<path d="M18 6 6 18M6 6l12 12" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>"#,
};

#[allow(dead_code)]
pub static CHECK: Icon = Icon {
    name: "check",
    view: 24.0,
    body: r#"<path d="m5 12.5 4.5 4.5L19 7" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
