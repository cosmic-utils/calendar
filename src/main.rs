// SPDX-License-Identifier: GPL-3.0-only

mod app;
mod components;
mod models;
mod notifications;
mod services;
mod shared;

pub use shared::error::{Error, Result};

fn main() -> cosmic::iced::Result {
    cosmic::app::run::<app::AppModel>(app::settings(), app::flags())
}
