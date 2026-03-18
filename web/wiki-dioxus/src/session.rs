use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct User {
    pub id: String,
    pub email: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub user: Option<User>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub node_id: Option<String>,
}

impl Session {
    pub fn is_authenticated(&self) -> bool {
        self.user.is_some() && self.access_token.is_some()
    }
}

pub static SESSION: GlobalSignal<Session> = Signal::global(Session::default);

pub fn use_session() -> Signal<Session> {
    SESSION.signal()
}

/// Load session from localStorage on startup
pub fn load_session() {
    if let Ok(Some(json)) = web_sys_storage() {
        if let Ok(session) = serde_json::from_str::<Session>(&json) {
            *SESSION.write() = session;
        }
    }
}

/// Save session to localStorage
pub fn save_session(session: &Session) {
    if let Ok(json) = serde_json::to_string(session) {
        let _ = set_web_sys_storage(&json);
    }
}

fn web_sys_storage() -> Result<Option<String>, ()> {
    let window = web_sys::window().ok_or(())?;
    let storage = window.local_storage().map_err(|_| ())?.ok_or(())?;
    storage.get_item("wiki_session").map_err(|_| ())
}

fn set_web_sys_storage(value: &str) -> Result<(), ()> {
    let window = web_sys::window().ok_or(())?;
    let storage = window.local_storage().map_err(|_| ())?.ok_or(())?;
    storage.set_item("wiki_session", value).map_err(|_| ())
}
