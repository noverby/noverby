use dioxus::prelude::*;

use crate::graphql::NodeWithChildren;
use crate::i18n::t;
use crate::session::use_session;

use super::content::ContentApp;
use super::loader::mime_icon;

/// VoteApp — voting interface for active polls
#[component]
pub fn VoteApp(node: NodeWithChildren) -> Element {
    let session = use_session();
    let is_auth = session.read().is_authenticated();

    rsx! {
        div { class: "card",
            div { class: "card-header",
                div { class: "avatar", "{mime_icon(\"vote/poll\")}" }
                h3 { class: "title-medium", "{t(\"mime.vote\")}" }
            }
            div { class: "card-content",
                if !is_auth {
                    p { class: "body-large", "{t(\"vote.noVotingRight\")}" }
                } else {
                    p { class: "body-large", "{t(\"vote.noVoteNow\")}" }
                }
            }
        }
    }
}

/// PolicyApp — document with comments, changes, and polls
#[component]
pub fn PolicyApp(node: NodeWithChildren) -> Element {
    let children = &node.children;

    // Separate children by type
    let polls: Vec<_> = children
        .iter()
        .filter(|c| c.mime_id.as_deref() == Some("vote/poll"))
        .collect();

    rsx! {
        // Main content
        ContentApp { node: node.clone() }

        // Poll list
        if !polls.is_empty() {
            div { class: "card mt-1",
                div { class: "card-header",
                    div { class: "avatar", "{mime_icon(\"vote/poll\")}" }
                    h3 { class: "title-medium", "{t(\"mime.vote\")}" }
                }
                div { class: "list",
                    for poll in polls.iter() {
                        div { class: "list-item", key: "{poll.id.0}",
                            div { class: "avatar small", "{mime_icon(\"vote/poll\")}" }
                            div { class: "list-item-text",
                                div { class: "list-item-primary", "{poll.name}" }
                            }
                        }
                    }
                }
            }
        }

        // Other children (amendments, comments, etc.)
        {
            let others: Vec<_> = children
                .iter()
                .filter(|c| c.mime_id.as_deref() != Some("vote/poll"))
                .collect();
            if !others.is_empty() {
                rsx! {
                    div { class: "card mt-1",
                        div { class: "list",
                            for child in others.iter() {
                                div { class: "list-item", key: "{child.id.0}",
                                    div { class: "avatar small",
                                        "{mime_icon(child.mime_id.as_deref().unwrap_or(\"\"))}"
                                    }
                                    div { class: "list-item-text",
                                        div { class: "list-item-primary", "{child.name}" }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                rsx! {}
            }
        }
    }
}

/// PollApp — poll administration
#[component]
pub fn PollApp(node: NodeWithChildren) -> Element {
    let name = node.name.as_str();
    let children = &node.children;

    rsx! {
        div { class: "card",
            div { class: "card-header",
                div { class: "avatar", "{mime_icon(\"vote/poll\")}" }
                div {
                    h3 { class: "title-medium", "{name}" }
                    p { class: "body-medium",
                        style: "color: var(--md-on-surface-variant);",
                        "{t(\"poll.managePoll\")}"
                    }
                }
            }
            if children.is_empty() {
                div { class: "card-content",
                    p { class: "body-medium",
                        style: "color: var(--md-on-surface-variant);",
                        "{t(\"common.noContent\")}"
                    }
                }
            } else {
                div { class: "list",
                    for child in children.iter() {
                        div { class: "list-item", key: "{child.id.0}",
                            div { class: "avatar small",
                                "{mime_icon(child.mime_id.as_deref().unwrap_or(\"\"))}"
                            }
                            div { class: "list-item-text",
                                div { class: "list-item-primary", "{child.name}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
