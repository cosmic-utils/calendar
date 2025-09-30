use crate::{models::Calendar, Result};
pub mod google;
pub mod microsoft;

use accounts::{
    models::{Account, Provider},
    AccountsClient,
};
use async_trait::async_trait;
pub use google::GoogleCalendarService;
pub use microsoft::MicrosoftCalendarService;

#[async_trait]
pub trait CalendarService: Send + Sync {
    async fn fetch_calendars(&mut self) -> Result<Vec<Calendar>>;
}

pub struct CalendarServiceFactory;

impl CalendarServiceFactory {
    pub async fn get_service(account: &Account) -> Result<Box<dyn CalendarService>> {
        let client = AccountsClient::new().await?;
        match account.provider {
            Provider::Google => Ok(Box::new(
                GoogleCalendarService::new(account, &client).await?,
            )),
            Provider::Microsoft => Ok(Box::new(
                MicrosoftCalendarService::new(account, &client).await?,
            )),
        }
    }
}
