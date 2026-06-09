use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use utoipa::ToSchema;

#[derive(Clone, Debug, Default)]
pub struct CredentialVault {
    records: BTreeMap<String, CredentialRecord>,
}

impl CredentialVault {
    pub fn register(&mut self, request: RegisterCredentialRequest) -> CredentialView {
        let id = format!("credential-{}", self.records.len() + 1);
        let record = CredentialRecord {
            id: id.clone(),
            label: request.label,
            exchange: request.exchange,
            payload: request.payload,
            created_at: Utc::now(),
        };
        let view = record.view();
        self.records.insert(id, record);
        view
    }

    pub fn list(&self) -> Vec<CredentialView> {
        self.records.values().map(CredentialRecord::view).collect()
    }

    pub fn get(&self, id: &str) -> Option<&CredentialRecord> {
        self.records.get(id)
    }

    pub fn records(&self) -> impl Iterator<Item = &CredentialRecord> {
        self.records.values()
    }
}

#[derive(Clone, Debug)]
pub struct CredentialRecord {
    pub id: String,
    pub label: String,
    pub exchange: ExchangeKind,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

impl CredentialRecord {
    pub fn view(&self) -> CredentialView {
        CredentialView {
            id: self.id.clone(),
            label: self.label.clone(),
            exchange: self.exchange.clone(),
            created_at: self.created_at,
        }
    }
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct RegisterCredentialRequest {
    pub label: String,
    pub exchange: ExchangeKind,
    pub payload: Value,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct CredentialView {
    pub id: String,
    pub label: String,
    pub exchange: ExchangeKind,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExchangeKind {
    Okx,
    Gate,
    Binance,
    Aster,
}
