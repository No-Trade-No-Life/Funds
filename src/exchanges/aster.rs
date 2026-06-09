use super::{ExchangeError, ExchangeSummary, insert_header, parse_number, required, signed_query};
use crate::capital::CapitalComponent;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use serde_json::Value;

pub async fn fetch_summary(payload: &Value) -> Result<ExchangeSummary, ExchangeError> {
    let access_key = required(payload, "api_key")?;
    let secret_key = required(payload, "secret_key")?;
    let query = signed_query(secret_key);
    let mut headers = HeaderMap::new();
    insert_header(&mut headers, "X-MBX-APIKEY", access_key)?;

    let response = reqwest::Client::new()
        .get(format!("https://fapi.asterdex.com/fapi/v4/account?{query}"))
        .headers(headers)
        .send()
        .await
        .map_err(|error| ExchangeError::Http(error.to_string()))?
        .json::<AsterAccountResponse>()
        .await
        .map_err(|error| ExchangeError::Http(error.to_string()))?;

    Ok(ExchangeSummary {
        components: response
            .assets
            .into_iter()
            .map(|asset| CapitalComponent {
                name: asset.asset,
                equity_usd: parse_number(&asset.wallet_balance),
            })
            .collect(),
    })
}

#[derive(Debug, Deserialize)]
struct AsterAccountResponse {
    assets: Vec<AsterAsset>,
}

#[derive(Debug, Deserialize)]
struct AsterAsset {
    asset: String,
    #[serde(rename = "walletBalance")]
    wallet_balance: String,
}
