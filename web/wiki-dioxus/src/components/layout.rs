use dioxus::prelude::*;

use crate::i18n::t;
use crate::route::Route;
use crate::session::use_session;
use crate::theme::{apply_theme, use_theme, THEME};

#[component]
pub fn Layout() -> Element {
    let _session = use_session();
    let mut open_drawer = use_signal(|| false);
    let mut search_mode = use_signal(|| false);
    let mut search_input = use_signal(String::new);

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

            // Desktop drawer (sidebar)
            div { class: "drawer",
                div { class: "drawer-inner",
                    DrawerContent {}
                }
            }

            // Bottom/top bar with search and breadcrumbs
            div { class: "bottom-bar",
                div { class: "bar",
                    // Menu button (mobile)
                    button {
                        class: "btn-icon mobile-only",
                        onclick: move |_| {
                            open_drawer.set(true);
                        },
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
                        Breadcrumbs {}
                        button {
                            class: "btn-icon",
                            onclick: move |_| search_mode.set(true),
                            span { class: "avatar small", "\u{1F50D}" }
                        }
                    }

                    // Theme toggle
                    ThemeToggle {}

                    // User menu
                    UserMenu {}
                }
            }

            // Spacer for bottom bar
            div { class: "bar-spacer" }
        }

        // Mobile drawer overlay
        div {
            class: if *open_drawer.read() { "mobile-drawer" } else { "mobile-drawer hidden" },
            div { style: "padding: 8px;",
                div { class: "bar",
                    div { class: "breadcrumbs", "{t(\"common.home\")}" }
                    button {
                        class: "btn-icon",
                        onclick: move |_| {
                            open_drawer.set(false);
                        },
                        span { class: "avatar small", "\u{2715}" }
                    }
                }
            }
            DrawerContent {}
        }
    }
}

/// Breadcrumb navigation based on current route
#[component]
fn Breadcrumbs() -> Element {
    let route = use_route::<Route>();

    let segments: Vec<String> = match &route {
        Route::PathPage { segments } => segments.clone(),
        _ => vec![],
    };

    rsx! {
        div { class: "breadcrumbs",
            Link { to: Route::HomeApp {}, "\u{1F3E0}" }
            for (i, segment) in segments.iter().enumerate() {
                span { class: "separator", " / " }
                if i == segments.len() - 1 {
                    span { "{segment}" }
                } else {
                    Link {
                        to: Route::PathPage {
                            segments: segments[..=i].to_vec(),
                        },
                        "{segment}"
                    }
                }
            }
        }
    }
}

/// Theme toggle button
#[component]
fn ThemeToggle() -> Element {
    let theme = use_theme();

    rsx! {
        button {
            class: "btn-icon",
            onclick: move |_| {
                let new_theme = theme.read().toggle();
                apply_theme(&new_theme);
                *THEME.write() = new_theme;
            },
            span { class: "avatar small",
                if *theme.read() == crate::theme::ThemeMode::Dark {
                    "\u{2600}" // sun
                } else {
                    "\u{1F319}" // moon
                }
            }
        }
    }
}

/// User menu (login/logout)
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
                span {
                    class: "avatar small secondary",
                    {session
                        .read()
                        .user
                        .as_ref()
                        .map(|u| u.display_name.chars().next().unwrap_or('?').to_string())
                        .unwrap_or_else(|| "?".to_string())}
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

/// Drawer content — shows navigation tree
#[component]
fn DrawerContent() -> Element {
    let session = use_session();
    let is_auth = session.read().is_authenticated();
    let display_name = session
        .read()
        .user
        .as_ref()
        .map(|u| u.display_name.clone())
        .unwrap_or_default();

    rsx! {
        div { style: "padding: 16px;",
            if is_auth {
                div { class: "card",
                    div { class: "card-header",
                        div { class: "avatar", "\u{1F464}" }
                        div {
                            h3 { class: "title-medium", "{display_name}" }
                            p { class: "body-medium",
                                style: "color: var(--md-on-surface-variant);",
                                "{session.read().user.as_ref().map(|u| u.email.as_str()).unwrap_or(\"\")}"
                            }
                        }
                    }
                }
            }

            div { class: "list", style: "margin-top: 8px;",
                Link {
                    to: Route::HomeApp {},
                    class: "list-item",
                    div { class: "avatar small", "\u{1F3E0}" }
                    div { class: "list-item-text",
                        div { class: "list-item-primary", "{t(\"common.home\")}" }
                    }
                }
            }
        }
    }
}
