// SPDX-License-Identifier: GPL-3.0-only

mod app;
mod components;
mod config;
mod error;
mod i18n;
mod models;
mod services;

pub use error::{Error, Result};

fn main() -> cosmic::iced::Result {
    cosmic::app::run::<app::AppModel>(app::settings(), app::flags())
}
