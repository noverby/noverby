use dioxus::prelude::*;

use crate::graphql::{ChildNodeFields, NodeWithChildren};
use crate::i18n::t;

use super::loader::mime_icon;

#[component]
pub fn FolderApp(node: NodeWithChildren) -> Element {
    let name = node.name.as_str();
    let children = &node.children;

    rsx! {
        div { class: "card",
            div { class: "card-header",
                div { class: "avatar", "{mime_icon(\"wiki/folder\")}" }
                h3 { class: "title-medium", "{name}" }
            }
            if children.is_empty() {
                div { class: "card-content",
                    p { class: "body-medium", style: "color: var(--md-on-surface-variant);",
                        "{t(\"common.noContent\")}"
                    }
                }
            } else {
                div { class: "list",
                    for child in children.iter() {
                        FolderItem {
                            key: "{child.id.0}",
                            node: child.clone(),
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn FolderItem(node: ChildNodeFields) -> Element {
    let name = node.name.as_str();
    let mime_id = node.mime_id.as_deref().unwrap_or("");
    let icon = mime_icon(mime_id);
    let is_mutable = node.mutable;

    rsx! {
        div { class: "folder-item",
            div { class: "avatar small", "{icon}" }
            div { class: "list-item-text",
                div { class: "list-item-primary", "{name}" }
                if is_mutable {
                    div { class: "list-item-secondary",
                        "\u{1F513} {t(\"layout.notSubmitted\")}"
                    }
                }
            }
        }
    }
}
