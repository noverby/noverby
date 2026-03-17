use dioxus::prelude::*;

use crate::graphql::NodeWithChildren;
use crate::i18n::t;
use crate::session::use_session;

/// MemberApp — member list and invitation management
#[component]
pub fn MemberApp(node: NodeWithChildren) -> Element {
    let name = node.name.as_str();
    let children = &node.children;
    let session = use_session();
    let is_auth = session.read().is_authenticated();
    let mut invite_input = use_signal(String::new);

    rsx! {
        div { class: "card",
            div { class: "card-header",
                div { class: "avatar", "\u{1F465}" }
                div {
                    h3 { class: "title-medium", "{name}" }
                    p { class: "body-medium",
                        style: "color: var(--md-on-surface-variant);",
                        "{t(\"common.members\")}"
                    }
                }
            }

            // Member list
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
                            div { class: "avatar small", "\u{1F464}" }
                            div { class: "list-item-text",
                                div { class: "list-item-primary", "{child.name}" }
                                div { class: "list-item-secondary",
                                    "{child.mime_id.as_deref().unwrap_or(\"\")}"
                                }
                            }
                        }
                    }
                }
            }

            // Invite input
            if is_auth {
                div { class: "card-content",
                    div { class: "text-field",
                        label { "{t(\"invite.nameOrEmail\")}" }
                        input {
                            r#type: "text",
                            placeholder: "{t(\"invite.nameOrEmail\")}",
                            value: "{invite_input}",
                            oninput: move |evt| invite_input.set(evt.value()),
                        }
                    }
                    button {
                        class: "btn btn-primary mt-1",
                        disabled: invite_input.read().is_empty(),
                        onclick: move |_| {
                            // TODO: Execute GraphQL mutation to invite member
                            log::info!("Invite: {}", invite_input.read());
                            invite_input.set(String::new());
                        },
                        "{t(\"invite.invite\")}"
                    }
                }
            }
        }
    }
}
