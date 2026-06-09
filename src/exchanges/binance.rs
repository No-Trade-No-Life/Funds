use super::{ExchangeError, ExchangeSummary, insert_header, parse_number, required, signed_query};
use crate::capital::CapitalComponent;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use serde_json::Value;

pub async fn fetch_summary(payload: &Value) -> Result<ExchangeSummary, ExchangeError> {
    let access_key = required(payload, "access_key")?;
    let secret_key = required(payload, "secret_key")?;
    let query = signed_query(secret_key);
    let mut headers = HeaderMap::new();
    insert_header(&mut headers, "X-MBX-APIKEY", access_key)?;

    let response = reqwest::Client::new()
        .get(format!("https://papi.binance.com/papi/v1/balance?{query}"))
        .headers(headers)
        .send()
        .await
        .map_err(|error| ExchangeError::Http(error.to_string()))?
        .json::<Vec<BinanceBalanceEntry>>()
        .await
        .map_err(|error| ExchangeError::Http(error.to_string()))?;

    Ok(ExchangeSummary {
        components: response
            .into_iter()
            .map(|entry| CapitalComponent {
                name: entry.asset,
                equity_usd: parse_number(&entry.total_wallet_balance),
            })
            .collect(),
    })
}

#[derive(Debug, Deserialize)]
struct BinanceBalanceEntry {
    asset: String,
    #[serde(rename = "totalWalletBalance")]
    total_wallet_balance: String,
}
