use dioxus::prelude::*;

use crate::components::{
    auth::{Login, Register, ResetPassword, SetPassword, Unverified},
    home::HomeApp,
    layout::Layout,
    loader::PathPage,
};

#[derive(Routable, Clone, Debug, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Layout)]
    #[route("/")]
    HomeApp {},

    #[route("/user/login")]
    Login {},

    #[route("/user/register")]
    Register {},

    #[route("/user/reset-password")]
    ResetPassword {},

    #[route("/user/set-password")]
    SetPassword {},

    #[route("/user/unverified")]
    Unverified {},

    #[route("/:..segments")]
    PathPage { segments: Vec<String> },
}
