use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid time component range: {0}")]
    Time(#[from] time::error::ComponentRange),
}
