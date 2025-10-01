use accounts::{models::Account, AccountsClient};
use async_trait::async_trait;
use graph_rs_sdk::GraphClient;

use crate::{
    models::Calendar,
    services::{microsoft::models::CalendarsResponse, CalendarService},
    Error, Result,
};

pub mod models;

#[derive(Debug, Clone)]
pub struct MicrosoftCalendarService {
    account: Account,
    client: AccountsClient,
    graph: GraphClient,
}

impl MicrosoftCalendarService {
    pub async fn new(account: &Account, client: &AccountsClient) -> Result<Self> {
        let mut client = client.clone();
        let access_token = client.get_access_token(&account.id).await?;

        Ok(MicrosoftCalendarService {
            account: account.clone(),
            client: client.clone(),
            graph: GraphClient::new(access_token),
        })
    }

    pub async fn refresh_access_token(&mut self) -> Result<()> {
        let token = self.client.get_access_token(&self.account.id).await?;
        self.graph = GraphClient::new(token);
        Ok(())
    }
}

#[async_trait]
impl CalendarService for MicrosoftCalendarService {
    async fn fetch_calendars(&mut self) -> Result<Vec<Calendar>> {
        self.refresh_access_token().await?;

        let response = self.graph.me().calendars().list_calendars().send().await?;
        if response.status() != 200 {
            return Err(Error::Unknown(format!(
                "Failed to fetch calendars: {}",
                response.text().await?
            )));
        }
        let response: CalendarsResponse = response.json().await.unwrap();
        Ok(response.value.into_iter().map(Into::into).collect())
    }
}
