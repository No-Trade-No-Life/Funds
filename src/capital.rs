use crate::{
    credentials::{CredentialRecord, CredentialVault, ExchangeKind},
    exchanges::{ExchangeError, fetch_exchange_summary},
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct CapitalSummary {
    pub generated_at: DateTime<Utc>,
    pub total_equity_usd: f64,
    pub accounts: Vec<AccountCapitalSummary>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct AccountCapitalSummary {
    pub credential_id: String,
    pub label: String,
    pub exchange: ExchangeKind,
    pub status: SummaryStatus,
    pub equity_usd: f64,
    pub components: Vec<CapitalComponent>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SummaryStatus {
    Ok,
    Failed,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct CapitalComponent {
    pub name: String,
    pub equity_usd: f64,
}

pub async fn summarize_vault(vault: &CredentialVault) -> CapitalSummary {
    let mut accounts = Vec::new();

    for credential in vault.records() {
        accounts.push(summarize_credential(credential).await);
    }

    CapitalSummary {
        generated_at: Utc::now(),
        total_equity_usd: accounts.iter().map(|account| account.equity_usd).sum(),
        accounts,
    }
}

pub async fn summarize_credential(credential: &CredentialRecord) -> AccountCapitalSummary {
    match fetch_exchange_summary(credential).await {
        Ok(summary) => AccountCapitalSummary {
            credential_id: credential.id.clone(),
            label: credential.label.clone(),
            exchange: credential.exchange.clone(),
            status: SummaryStatus::Ok,
            equity_usd: summary
                .components
                .iter()
                .map(|component| component.equity_usd)
                .sum(),
            components: summary.components,
            error: None,
        },
        Err(error) => failed_summary(credential, error),
    }
}

fn failed_summary(credential: &CredentialRecord, error: ExchangeError) -> AccountCapitalSummary {
    AccountCapitalSummary {
        credential_id: credential.id.clone(),
        label: credential.label.clone(),
        exchange: credential.exchange.clone(),
        status: SummaryStatus::Failed,
        equity_usd: 0.0,
        components: Vec::new(),
        error: Some(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::RegisterCredentialRequest;
    use serde_json::json;

    #[tokio::test]
    async fn failed_credential_does_not_contribute_to_total() {
        let mut vault = CredentialVault::default();
        vault.register(RegisterCredentialRequest {
            label: "Missing keys".to_owned(),
            exchange: ExchangeKind::Okx,
            payload: json!({}),
        });

        let summary = summarize_vault(&vault).await;

        assert_eq!(summary.total_equity_usd, 0.0);
        assert!(matches!(summary.accounts[0].status, SummaryStatus::Failed));
    }
}
