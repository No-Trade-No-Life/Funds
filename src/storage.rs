use crate::{
    credentials::{CredentialRecord, CredentialVault, ExchangeKind},
    domain::{FundEvent, FundRecord},
};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use serde_json::Value;
use std::{error::Error, fmt, path::Path, sync::Arc, sync::Mutex};

#[derive(Clone)]
pub struct Database {
    connection: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let database = Self {
            connection: Arc::new(Mutex::new(Connection::open(path)?)),
        };
        database.migrate()?;
        Ok(database)
    }

    pub fn memory() -> Result<Self, StorageError> {
        let database = Self {
            connection: Arc::new(Mutex::new(Connection::open_in_memory()?)),
        };
        database.migrate()?;
        Ok(database)
    }

    pub fn load_funds(&self) -> Result<Vec<FundRecord>, StorageError> {
        let connection = self.lock()?;
        let mut statement =
            connection.prepare("SELECT account_id, description, events_json FROM funds")?;
        let rows = statement.query_map([], |row| {
            let account_id: String = row.get(0)?;
            let description: String = row.get(1)?;
            let events_json: String = row.get(2)?;
            Ok((account_id, description, events_json))
        })?;

        let mut funds = Vec::new();
        for row in rows {
            let (account_id, description, events_json) = row?;
            let events: Vec<FundEvent> = serde_json::from_str(&events_json)?;
            let mut fund = FundRecord::new(account_id, description);
            for event in events {
                fund.append(event);
            }
            funds.push(fund);
        }

        Ok(funds)
    }

    pub fn save_fund(&self, fund: &FundRecord) -> Result<(), StorageError> {
        let events_json = serde_json::to_string(&fund.events)?;
        let connection = self.lock()?;
        connection.execute(
            "INSERT INTO funds (account_id, description, events_json) VALUES (?1, ?2, ?3)
             ON CONFLICT(account_id) DO UPDATE SET description = excluded.description, events_json = excluded.events_json",
            params![fund.account_id, fund.description, events_json],
        )?;

        Ok(())
    }

    pub fn load_credentials(&self) -> Result<CredentialVault, StorageError> {
        let connection = self.lock()?;
        let mut statement = connection.prepare(
            "SELECT id, label, exchange, payload_json, created_at FROM credentials ORDER BY id",
        )?;
        let rows = statement.query_map([], |row| {
            let id: String = row.get(0)?;
            let label: String = row.get(1)?;
            let exchange_json: String = row.get(2)?;
            let payload_json: String = row.get(3)?;
            let created_at: String = row.get(4)?;
            Ok((id, label, exchange_json, payload_json, created_at))
        })?;

        let mut credentials = Vec::new();
        for row in rows {
            let (id, label, exchange_json, payload_json, created_at) = row?;
            credentials.push(CredentialRecord {
                id,
                label,
                exchange: serde_json::from_str::<ExchangeKind>(&exchange_json)?,
                payload: serde_json::from_str::<Value>(&payload_json)?,
                created_at: DateTime::parse_from_rfc3339(&created_at)?.with_timezone(&Utc),
            });
        }

        Ok(CredentialVault::from_records(credentials))
    }

    pub fn save_credential(&self, credential: &CredentialRecord) -> Result<(), StorageError> {
        let exchange_json = serde_json::to_string(&credential.exchange)?;
        let payload_json = serde_json::to_string(&credential.payload)?;
        let created_at = credential.created_at.to_rfc3339();
        let connection = self.lock()?;
        connection.execute(
            "INSERT INTO credentials (id, label, exchange, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET label = excluded.label, exchange = excluded.exchange, payload_json = excluded.payload_json, created_at = excluded.created_at",
            params![credential.id, credential.label, exchange_json, payload_json, created_at],
        )?;

        Ok(())
    }

    pub fn delete_credential(&self, id: &str) -> Result<(), StorageError> {
        let connection = self.lock()?;
        connection.execute("DELETE FROM credentials WHERE id = ?1", params![id])?;

        Ok(())
    }

    fn migrate(&self) -> Result<(), StorageError> {
        let connection = self.lock()?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS funds (
                account_id TEXT NOT NULL PRIMARY KEY,
                description TEXT NOT NULL,
                events_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS credentials (
                id TEXT NOT NULL PRIMARY KEY,
                label TEXT NOT NULL,
                exchange TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );",
        )?;

        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, StorageError> {
        self.connection
            .lock()
            .map_err(|_| StorageError::PoisonedConnection)
    }
}

#[derive(Debug)]
pub enum StorageError {
    Sqlite(rusqlite::Error),
    Json(serde_json::Error),
    Time(chrono::ParseError),
    PoisonedConnection,
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::Sqlite(error) => write!(formatter, "sqlite error: {error}"),
            StorageError::Json(error) => write!(formatter, "json error: {error}"),
            StorageError::Time(error) => write!(formatter, "time parse error: {error}"),
            StorageError::PoisonedConnection => {
                write!(formatter, "sqlite connection lock was poisoned")
            }
        }
    }
}

impl Error for StorageError {}

impl From<rusqlite::Error> for StorageError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<chrono::ParseError> for StorageError {
    fn from(error: chrono::ParseError) -> Self {
        Self::Time(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        credentials::RegisterCredentialRequest,
        domain::{FundEvent, InvestorOrder},
    };
    use serde_json::json;

    #[test]
    fn persists_fund_events_and_rebuilds_state() {
        let database = Database::memory().unwrap();
        let updated_at = "2026-06-09T00:00:00Z".parse().unwrap();
        let mut fund = FundRecord::new("fund/main".to_owned(), "Main fund".to_owned());
        fund.append(FundEvent {
            updated_at,
            comment: None,
            fund_equity: None,
            order: Some(InvestorOrder {
                name: "Alice".to_owned(),
                deposit: 100.0,
            }),
            investor: None,
            taxation: None,
        });

        database.save_fund(&fund).unwrap();
        let funds = database.load_funds().unwrap();

        assert_eq!(funds[0].state.total_assets, 100.0);
        assert_eq!(funds[0].events.len(), 1);
    }

    #[test]
    fn persists_credentials_with_payload() {
        let database = Database::memory().unwrap();
        let mut vault = CredentialVault::default();
        let credential = vault.register(RegisterCredentialRequest {
            label: "OKX main".to_owned(),
            exchange: ExchangeKind::Okx,
            payload: json!({ "secret_key": "secret" }),
        });

        database.save_credential(&credential).unwrap();
        let loaded = database.load_credentials().unwrap();

        assert_eq!(loaded.list()[0].label, "OKX main");
        assert_eq!(
            loaded.get("credential-1").unwrap().payload["secret_key"],
            "secret"
        );
    }

    #[test]
    fn deletes_credentials() {
        let database = Database::memory().unwrap();
        let mut vault = CredentialVault::default();
        let credential = vault.register(RegisterCredentialRequest {
            label: "OKX main".to_owned(),
            exchange: ExchangeKind::Okx,
            payload: json!({ "secret_key": "secret" }),
        });

        database.save_credential(&credential).unwrap();
        database.delete_credential("credential-1").unwrap();

        assert!(database.load_credentials().unwrap().list().is_empty());
    }
}
