use accounts::{models::Account, AccountsClient};
use async_trait::async_trait;
use gcal_rs::{GCalClient, OToken};

use crate::{models::Calendar, services::CalendarService, Result};

#[derive(Clone)]
pub struct GoogleCalendarService {
    account: Account,
    client: AccountsClient,
    google: GCalClient,
}

impl GoogleCalendarService {
    pub async fn new(account: &Account, client: &AccountsClient) -> Result<Self> {
        let mut client = client.clone();
        let access_token = client.get_access_token(&account.id).await?;
        let refresh_token = client.get_refresh_token(&account.id).await?;
        let mut token = OToken::default();
        token.access = access_token;
        token.refresh = (!refresh_token.is_empty()).then_some(refresh_token);
        Ok(GoogleCalendarService {
            account: account.clone(),
            client: client.clone(),
            google: GCalClient::new(token, None)?,
        })
    }

    pub async fn refresh_access_token(&mut self) -> Result<()> {
        let access_token = self.client.get_access_token(&self.account.id).await?;
        let mut token = OToken::default();
        token.access = access_token;
        self.google = GCalClient::new(token, None)?;
        Ok(())
    }
}

#[async_trait]
impl CalendarService for GoogleCalendarService {
    async fn fetch_calendars(&mut self) -> Result<Vec<Calendar>> {
        self.refresh_access_token().await?;

        let calendar_client = self.google.clone().calendar_client();
        let calendars = calendar_client
            .list(false, Default::default())
            .await?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(calendars)
    }
}
