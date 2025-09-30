use accounts::zbus;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid time component range: {0}")]
    Time(#[from] time::error::ComponentRange),
    #[error("Date calculation error: {0}")]
    DateCalculation(String),
    #[error("Zbus error: {0}")]
    Zbus(#[from] zbus::Error),
    #[error("Zbus error: {0}")]
    ZbusFdo(#[from] zbus::fdo::Error),
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Serde JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("Graph error: {0}")]
    GraphError(#[from] graph_rs_sdk::GraphFailure),
    #[error("Google Calendar error: {0}")]
    GCal(#[from] gcal_rs::ClientError),
    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, Error>;
