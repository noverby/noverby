use dioxus::prelude::*;

use crate::graphql::{self, Jsonb, NodeWithChildren, NodesInsertInput, Uuid};
use crate::i18n::t;
use crate::session::use_session;

/// SpeakApp — speaker queue management
#[component]
pub fn SpeakApp(node: NodeWithChildren) -> Element {
    let name = node.name.as_str();
    let children = &node.children;
    let session = use_session();
    let is_auth = session.read().is_authenticated();
    let node_id = node.id.0.clone();
    let context_id = node.context_id.clone();

    let speakers: Vec<_> = children.iter().collect();

    rsx! {
        div { class: "grid grid-2",
            // Speaker list card
            div {
                div { class: "card",
                    div { class: "card-header",
                        div { class: "avatar secondary", "\u{1F3A4}" }
                        div {
                            h3 { class: "title-medium", "{name}" }
                            p { class: "body-medium",
                                style: "color: var(--md-on-surface-variant);",
                                "{t(\"speak.speakerList\")}"
                            }
                        }
                    }
                    if speakers.is_empty() {
                        div { class: "card-content",
                            p { class: "body-medium",
                                style: "color: var(--md-on-surface-variant);",
                                "{t(\"speak.emptyList\")}"
                            }
                        }
                    } else {
                        div { class: "list",
                            for (i , speaker) in speakers.iter().enumerate() {
                                div { class: "list-item", key: "{speaker.id.0}",
                                    div { class: "avatar small secondary", "{i + 1}" }
                                    div { class: "list-item-text",
                                        div { class: "list-item-primary", "{speaker.name}" }
                                    }
                                    // Delete button
                                    if is_auth {
                                        {
                                            let speaker_id = speaker.id.0.clone();
                                            let token = session.read().access_token.clone();
                                            rsx! {
                                                button {
                                                    class: "btn-icon",
                                                    onclick: move |_| {
                                                        let token = token.clone();
                                                        let id = speaker_id.clone();
                                                        spawn(async move {
                                                            let _ = graphql::delete_node(token.as_deref(), &id).await;
                                                        });
                                                    },
                                                    "\u{2715}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Actions panel
            div {
                if is_auth {
                    div { class: "card",
                        div { class: "card-header",
                            h3 { class: "title-medium", "{t(\"speak.joinSpeakerList\")}" }
                        }
                        div { class: "card-content",
                            div { class: "stack stack-v",
                                {
                                    let speak_types = [
                                        ("0", t("speak.talk")),
                                        ("1", t("speak.question")),
                                        ("2", t("speak.clarify")),
                                        ("3", t("speak.procedure")),
                                    ];
                                    rsx! {
                                        for (type_key, label) in speak_types {
                                            {
                                                let node_id = node_id.clone();
                                                let context_id = context_id.clone();
                                                let display_name = session.read().user.as_ref().map(|u| u.display_name.clone()).unwrap_or_default();
                                                let token = session.read().access_token.clone();
                                                rsx! {
                                                    button {
                                                        class: "btn btn-outlined",
                                                        onclick: move |_| {
                                                            let name = display_name.clone();
                                                            let key = format!("{}-{}", name.to_lowercase(), chrono_now());
                                                            let parent = node_id.clone();
                                                            let ctx = context_id.clone();
                                                            let token = token.clone();
                                                            let type_val = type_key.to_string();
                                                            spawn(async move {
                                                                let _ = graphql::insert_node(
                                                                    token.as_deref(),
                                                                    NodesInsertInput {
                                                                        name: Some(name),
                                                                        key: Some(key),
                                                                        mime_id: Some("speak/speak".to_string()),
                                                                        parent_id: Some(Uuid(parent)),
                                                                        context_id: ctx,
                                                                        data: Some(Jsonb(serde_json::Value::String(type_val))),
                                                                        mutable: None,
                                                                        index: None,
                                                                    },
                                                                ).await;
                                                            });
                                                        },
                                                        "{label}"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn chrono_now() -> String {
    // Simple timestamp for key generation
    js_sys_date()
}

fn js_sys_date() -> String {
    let window = web_sys::window().unwrap();
    let performance = window.performance().unwrap();
    format!("{:.0}", performance.now())
}
