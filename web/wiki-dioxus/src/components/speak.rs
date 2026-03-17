use dioxus::prelude::*;

use crate::graphql::NodeWithChildren;
use crate::i18n::t;

/// SpeakApp — speaker queue management
#[component]
pub fn SpeakApp(node: NodeWithChildren) -> Element {
    let name = node.name.as_str();
    let children = &node.children;

    // Speakers are children sorted by their data (priority) and creation date
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
                            for (i, speaker) in speakers.iter().enumerate() {
                                div { class: "list-item", key: "{speaker.id.0}",
                                    div { class: "avatar small secondary",
                                        "{i + 1}"
                                    }
                                    div { class: "list-item-text",
                                        div { class: "list-item-primary",
                                            "{speaker.name}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Admin panel
            div {
                div { class: "card",
                    div { class: "card-header",
                        h3 { class: "title-medium", "{t(\"speak.manageSpeakerList\")}" }
                    }
                    div { class: "card-content",
                        div { class: "stack stack-h",
                            button { class: "btn btn-outlined",
                                "{t(\"speak.open\")}"
                            }
                            button { class: "btn btn-outlined",
                                "{t(\"speak.close\")}"
                            }
                        }
                    }
                }
            }
        }
    }
}
