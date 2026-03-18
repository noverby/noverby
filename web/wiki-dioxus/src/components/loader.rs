use dioxus::prelude::*;

use crate::graphql::{self, NodeWithChildren};
use crate::i18n::t;
use crate::route::Route;
use crate::session::use_session;

use super::content::ContentApp;
use super::editor::EditorApp;
use super::file::FileApp;
use super::folder::FolderApp;
use super::home::HomeApp;
use super::member::MemberApp;
use super::node::NodeApp;
use super::sort::SortApp;
use super::speak::SpeakApp;
use super::vote::{PolicyApp, PollApp, VoteApp};

/// The catch-all path page — resolves URL segments to a node
#[component]
pub fn PathPage(segments: Vec<String>) -> Element {
    let session = use_session();
    let access_token = session.read().access_token.clone();
    let segments_clone = segments.clone();

    let node_future = use_resource(move || {
        let token = access_token.clone();
        let segs = segments_clone.clone();
        async move { graphql::resolve_path(token.as_deref(), &segs).await }
    });

    let result = node_future.read().clone();
    match result {
        Some(Ok(Some(node))) => {
            // Check for ?app= query parameter
            let app_param = web_sys::window()
                .and_then(|w| w.location().search().ok())
                .and_then(|s| {
                    s.trim_start_matches('?').split('&').find_map(|pair| {
                        let mut parts = pair.splitn(2, '=');
                        if parts.next() == Some("app") {
                            parts.next().map(String::from)
                        } else {
                            None
                        }
                    })
                });

            match app_param.as_deref() {
                Some("vote") => rsx! { VoteApp { node } },
                Some("speak") => rsx! { SpeakApp { node } },
                Some("member") => rsx! { MemberApp { node } },
                Some("editor") => rsx! { EditorApp { node } },
                Some("sort") => rsx! { SortApp { node } },
                _ => rsx! { MimeLoader { node, path: segments.clone() } },
            }
        }
        Some(Ok(None)) => {
            rsx! { NodeNotFound {} }
        }
        Some(Err(e)) => {
            rsx! {
                div { class: "card",
                    div { class: "card-content",
                        p { class: "body-large", "{t(\"error.somethingWentWrong\")}" }
                        pre { class: "error-fallback", "{e}" }
                    }
                }
            }
        }
        None => {
            rsx! {
                div { class: "spinner-overlay",
                    div { class: "spinner" }
                }
            }
        }
    }
}

/// Routes a node to the appropriate app based on its MIME type
#[component]
fn MimeLoader(node: NodeWithChildren, path: Vec<String>) -> Element {
    let mime_id = node.mime_id.as_deref().unwrap_or("");

    match mime_id {
        "wiki/folder" => rsx! { FolderApp { node: node.clone(), parent_path: path } },
        "wiki/document" => rsx! { ContentApp { node: node.clone() } },
        "wiki/file" => rsx! { FileApp { node: node.clone() } },
        "wiki/home" => rsx! { HomeApp {} },
        "wiki/group" | "wiki/event" => {
            rsx! { FolderApp { node: node.clone(), parent_path: path } }
        }
        "vote/policy" | "vote/change" => {
            rsx! { PolicyApp { node: node.clone() } }
        }
        "vote/position" => {
            rsx! { NodeApp { node: node.clone(), title: t("mime.position") } }
        }
        "vote/candidate" => {
            rsx! { NodeApp { node: node.clone(), title: t("mime.candidate") } }
        }
        "vote/poll" => rsx! { PollApp { node: node.clone() } },
        _ => rsx! { NodeApp { node: node.clone(), title: t("mime.unknown") } },
    }
}

/// Mime type to icon character
pub fn mime_icon(mime_id: &str) -> &'static str {
    match mime_id {
        "wiki/folder" => "\u{1F4C1}",
        "wiki/document" => "\u{1F4C4}",
        "wiki/file" => "\u{1F4CE}",
        "wiki/group" => "\u{1F465}",
        "wiki/event" => "\u{1F4C5}",
        "wiki/user" => "\u{1F464}",
        "wiki/home" => "\u{1F3E0}",
        "vote/policy" | "vote/change" => "\u{1F4DC}",
        "vote/position" => "\u{1F3AF}",
        "vote/candidate" => "\u{1F3C6}",
        "vote/poll" => "\u{1F4CA}",
        "map/map" => "\u{1F5FA}",
        _ => "\u{2753}",
    }
}

#[component]
fn NodeNotFound() -> Element {
    let session = use_session();
    let is_auth = session.read().is_authenticated();

    rsx! {
        div { class: "card",
            div { class: "card-header",
                div { class: "avatar", "\u{26A0}" }
                h3 { class: "headline-small", "{t(\"node.documentUnavailable\")}" }
            }
            div { class: "card-content",
                p { class: "body-large mb-1", "{t(\"node.notFoundOrNoAccess\")}" }
                if !is_auth {
                    p { class: "body-large mb-2", "{t(\"node.maybeLoginForAccess\")}" }
                    Link {
                        to: Route::Login {},
                        class: "btn btn-primary",
                        "{t(\"common.logIn\")}"
                    }
                }
            }
        }
    }
}
