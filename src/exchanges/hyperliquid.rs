use super::{ExchangeError, ExchangeSummary, parse_number, required};
use crate::capital::CapitalComponent;
use serde::Deserialize;
use serde_json::Value;

pub async fn fetch_summary(payload: &Value) -> Result<ExchangeSummary, ExchangeError> {
    let address = required(payload, "address")?;
    let response = reqwest::Client::new()
        .post("https://api.hyperliquid.xyz/info")
        .json(&serde_json::json!({
            "type": "clearinghouseState",
            "user": address,
        }))
        .send()
        .await
        .map_err(|error| ExchangeError::Http(error.to_string()))?
        .json::<HyperliquidClearinghouseState>()
        .await
        .map_err(|error| ExchangeError::Http(error.to_string()))?;

    Ok(ExchangeSummary {
        components: vec![CapitalComponent {
            name: "perpetual".to_owned(),
            equity_usd: parse_number(&response.margin_summary.account_value),
        }],
    })
}

#[derive(Debug, Deserialize)]
struct HyperliquidClearinghouseState {
    #[serde(rename = "marginSummary")]
    margin_summary: HyperliquidMarginSummary,
}

#[derive(Debug, Deserialize)]
struct HyperliquidMarginSummary {
    #[serde(rename = "accountValue")]
    account_value: String,
}
