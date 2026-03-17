use dioxus::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub enum ThemeMode {
    Light,
    Dark,
}

impl ThemeMode {
    pub fn toggle(&self) -> Self {
        match self {
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::Light,
        }
    }

    pub fn data_attr(&self) -> &'static str {
        match self {
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
        }
    }
}

pub static THEME: GlobalSignal<ThemeMode> = Signal::global(|| ThemeMode::Light);

pub fn use_theme() -> Signal<ThemeMode> {
    THEME.signal()
}

/// Apply theme to the document element
pub fn apply_theme(mode: &ThemeMode) {
    if let Some(window) = web_sys::window() {
        if let Some(doc) = window.document() {
            if let Some(el) = doc.document_element() {
                let _ = el.set_attribute("data-theme", mode.data_attr());
            }
        }
    }
}
