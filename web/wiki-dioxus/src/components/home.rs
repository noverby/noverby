use dioxus::prelude::*;

use crate::i18n::{t, t_with};
use crate::route::Route;
use crate::session::use_session;

#[component]
pub fn HomeApp() -> Element {
    let session = use_session();
    let is_auth = session.read().is_authenticated();
    let display_name = session
        .read()
        .user
        .as_ref()
        .map(|u| u.display_name.clone())
        .unwrap_or_default();

    rsx! {
        div { class: "grid grid-3",
            // Main content column
            div {
                div { class: "card",
                    div { class: "card-header",
                        div { class: "avatar", "\u{1F44B}" }
                        h3 { class: "headline-small", "{t(\"layout.welcomeTitle\")}" }
                    }
                    div { class: "card-content",
                        if !is_auth {
                            p { class: "body-large mb-1", "{t(\"layout.loginOrRegister\")}" }
                            p { class: "body-medium mb-2", "{t(\"layout.rememberEmail\")}" }
                            div { class: "stack stack-h",
                                Link {
                                    to: Route::Login {},
                                    class: "btn btn-outlined",
                                    "\u{1F511} {t(\"common.logIn\")}"
                                }
                                Link {
                                    to: Route::Register {},
                                    class: "btn btn-outlined",
                                    "\u{1F464} {t(\"auth.register\")}"
                                }
                            }
                        } else {
                            p { class: "body-large mb-1",
                                "{t_with(\"layout.greeting\", &[(\"name\", &display_name)])}"
                            }
                            p { class: "body-large mb-1", "{t(\"layout.acceptInvitations\")}" }
                            p { class: "body-medium", "{t(\"layout.noInvitationsHint\")}" }
                        }
                    }
                }
            }

            // Sidebar column (invitations, etc.)
            if is_auth {
                div {
                    // Placeholder for invitations list
                }
            }
        }
    }
}
