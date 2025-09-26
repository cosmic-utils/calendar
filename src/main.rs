// SPDX-License-Identifier: GPL-3.0-only

mod app;
mod config;
mod error;
mod i18n;

pub use error::Error;

fn main() -> cosmic::iced::Result {
    cosmic::app::run::<app::AppModel>(app::settings(), app::flags())
}
