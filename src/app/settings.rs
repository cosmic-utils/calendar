use cosmic::app::Settings;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::i18n;

pub fn settings() -> Settings {
    // Get the system's preferred languages.
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();

    // Enable localizations to be applied.
    i18n::init(&requested_languages);

    // Initialize tracing subscriber.
    if std::env::var("RUST_LOG").is_err() {
        unsafe {
            std::env::set_var("RUST_LOG", "cosmic_ext_calendar=info");
        }
    }
    tracing_subscriber::registry()
        .with(EnvFilter::from_env("RUST_LOG"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Settings for configuring the application window and iced runtime.
    let settings = Settings::default().size_limits(
        cosmic::iced::Limits::NONE
            .min_width(360.0)
            .min_height(180.0),
    );

    settings
}
