mod aster;
mod binance;
mod bitget;
mod gate;
mod htx;
mod hyperliquid;
mod okx;

use crate::{capital::CapitalComponent, credentials::CredentialRecord, credentials::ExchangeKind};
use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::Value;
use sha2::{Sha256, Sha512};
use std::{error::Error, fmt};

type HmacSha256 = Hmac<Sha256>;
type HmacSha512 = Hmac<Sha512>;

#[derive(Clone, Debug)]
pub struct ExchangeSummary {
    pub components: Vec<CapitalComponent>,
}

#[derive(Clone, Debug)]
pub enum ExchangeError {
    MissingField(&'static str),
    InvalidHeader(&'static str),
    Http(String),
    Api(String),
}

impl fmt::Display for ExchangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExchangeError::MissingField(field) => {
                write!(formatter, "missing credential field '{field}'")
            }
            ExchangeError::InvalidHeader(header) => {
                write!(formatter, "invalid HTTP header '{header}'")
            }
            ExchangeError::Http(message) => write!(formatter, "exchange HTTP error: {message}"),
            ExchangeError::Api(message) => write!(formatter, "exchange API error: {message}"),
        }
    }
}

impl Error for ExchangeError {}

pub async fn fetch_exchange_summary(
    credential: &CredentialRecord,
) -> Result<ExchangeSummary, ExchangeError> {
    match credential.exchange {
        ExchangeKind::Okx => okx::fetch_summary(&credential.payload).await,
        ExchangeKind::Gate => gate::fetch_summary(&credential.payload).await,
        ExchangeKind::Binance => binance::fetch_summary(&credential.payload).await,
        ExchangeKind::Aster => aster::fetch_summary(&credential.payload).await,
        ExchangeKind::Hyperliquid => hyperliquid::fetch_summary(&credential.payload).await,
        ExchangeKind::Bitget => bitget::fetch_summary(&credential.payload).await,
        ExchangeKind::Htx => htx::fetch_summary(&credential.payload).await,
    }
}

fn required<'a>(payload: &'a Value, field: &'static str) -> Result<&'a str, ExchangeError> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(ExchangeError::MissingField(field))
}

fn insert_header(
    headers: &mut HeaderMap,
    name: &'static str,
    value: &str,
) -> Result<(), ExchangeError> {
    let value = HeaderValue::from_str(value).map_err(|_| ExchangeError::InvalidHeader(name))?;
    headers.insert(name, value);
    Ok(())
}

fn signed_query(secret: &str) -> String {
    let query = format!(
        "timestamp={}&recvWindow=5000",
        chrono::Utc::now().timestamp_millis()
    );
    let signature = hmac_sha256_hex(secret, &query);
    format!("{query}&signature={signature}")
}

fn hmac_sha256_hex(secret: &str, message: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key size");
    mac.update(message.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn hmac_sha512_hex(secret: &str, message: &str) -> String {
    let mut mac = HmacSha512::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key size");
    mac.update(message.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn parse_number(value: &str) -> f64 {
    value.parse().unwrap_or(0.0)
}

fn url_encode(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace(':', "%3A")
        .replace('+', "%2B")
        .replace('/', "%2F")
        .replace('=', "%3D")
}
