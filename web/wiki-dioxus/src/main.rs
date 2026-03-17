mod components;
#[allow(dead_code)]
mod graphql;
#[allow(dead_code)]
mod i18n;
#[allow(dead_code)]
mod nhost;
mod route;
#[allow(dead_code)]
mod session;

use dioxus::prelude::*;
use route::Route;

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    log::info!("RadikalWiki starting...");

    // Detect browser language for i18n
    if let Some(window) = web_sys::window() {
        if let Some(lang) = window.navigator().language() {
            if lang.starts_with("da") {
                *i18n::LANG.write() = i18n::Lang::Da;
            }
        }
    }

    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        Router::<Route> {}
    }
}
