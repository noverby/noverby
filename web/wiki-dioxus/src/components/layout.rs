use dioxus::prelude::*;

use crate::i18n::t;
use crate::route::Route;
use crate::session::use_session;

#[component]
pub fn Layout() -> Element {
    let _session = use_session();
    let mut open_drawer = use_signal(|| false);
    let mut search_mode = use_signal(|| false);
    let mut search_input = use_signal(String::new);

    // Check if we're on a user/auth page (no chrome needed)
    let route = use_route::<Route>();
    let is_auth_page = matches!(
        route,
        Route::Login {}
            | Route::Register {}
            | Route::ResetPassword {}
            | Route::SetPassword {}
            | Route::Unverified {}
    );

    if is_auth_page {
        return rsx! {
            Outlet::<Route> {}
        };
    }

    rsx! {
        div { class: "app-shell",
            // Main content area
            div { class: "main-content",
                Outlet::<Route> {}
            }

            // Bottom bar with search
            div { class: "bottom-bar",
                div { class: "bar",
                    // Menu button (mobile only, triggers drawer)
                    button {
                        class: "btn-icon",
                        style: "display: none;", // shown via CSS media query on mobile
                        onclick: move |_| open_drawer.set(true),
                        span { class: "avatar small", "\u{2630}" }
                    }

                    // Search or breadcrumbs
                    if *search_mode.read() {
                        input {
                            class: "breadcrumbs",
                            style: "background: transparent; border: none; color: white; outline: none; font-size: 14px;",
                            placeholder: "{t(\"common.search\")}",
                            value: "{search_input}",
                            oninput: move |evt| search_input.set(evt.value()),
                            onkeydown: move |evt| {
                                if evt.key() == Key::Escape {
                                    search_mode.set(false);
                                    search_input.set(String::new());
                                }
                            },
                        }
                        button {
                            class: "btn-icon",
                            onclick: move |_| {
                                search_mode.set(false);
                                search_input.set(String::new());
                            },
                            span { class: "avatar small", "\u{2715}" }
                        }
                    } else {
                        div { class: "breadcrumbs",
                            Link { to: Route::HomeApp {}, "{t(\"common.home\")}" }
                        }
                        button {
                            class: "btn-icon",
                            onclick: move |_| search_mode.set(true),
                            span { class: "avatar small", "\u{1F50D}" }
                        }
                    }

                    // User menu
                    UserMenu {}
                }
            }

            // Spacer for bottom bar
            div { class: "bar-spacer" }
        }

        // Mobile drawer
        div {
            class: if *open_drawer.read() { "mobile-drawer" } else { "mobile-drawer hidden" },
            div { style: "padding: 8px;",
                div { class: "bar",
                    div { class: "breadcrumbs",
                        "{t(\"common.home\")}"
                    }
                    button {
                        class: "btn-icon",
                        onclick: move |_| open_drawer.set(false),
                        span { class: "avatar small", "\u{2715}" }
                    }
                }
            }
        }
    }
}

#[component]
fn UserMenu() -> Element {
    let session = use_session();
    let nav = use_navigator();
    let is_auth = session.read().is_authenticated();

    rsx! {
        if is_auth {
            button {
                class: "btn-icon",
                onclick: move |_| {
                    crate::nhost::sign_out();
                    *crate::session::SESSION.write() = Default::default();
                    crate::session::save_session(&Default::default());
                    nav.push(Route::HomeApp {});
                },
                span { class: "avatar small secondary",
                    {session.read().user.as_ref().map(|u| {
                        u.display_name.chars().next().unwrap_or('?').to_string()
                    }).unwrap_or_else(|| "?".to_string())}
                }
            }
        } else {
            Link {
                to: Route::Login {},
                class: "btn-icon",
                span { class: "avatar small", "\u{1F464}" }
            }
        }
    }
}
