use super::{ExchangeError, ExchangeSummary, HmacSha256, insert_header, parse_number, required};
use crate::capital::CapitalComponent;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use chrono::Utc;
use hmac::Mac;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use serde_json::Value;

pub async fn fetch_summary(payload: &Value) -> Result<ExchangeSummary, ExchangeError> {
    let access_key = required(payload, "access_key")?;
    let secret_key = required(payload, "secret_key")?;
    let passphrase = required(payload, "passphrase")?;
    let path = "/api/v3/account/assets";
    let timestamp = Utc::now().timestamp_millis().to_string();
    let signature = signature(&timestamp, "GET", path, "", "", secret_key);
    let mut headers = HeaderMap::new();
    insert_header(&mut headers, "ACCESS-KEY", access_key)?;
    insert_header(&mut headers, "ACCESS-SIGN", &signature)?;
    insert_header(&mut headers, "ACCESS-TIMESTAMP", &timestamp)?;
    insert_header(&mut headers, "ACCESS-PASSPHRASE", passphrase)?;

    let response = reqwest::Client::new()
        .get(format!("https://api.bitget.com{path}"))
        .headers(headers)
        .send()
        .await
        .map_err(|error| ExchangeError::Http(error.to_string()))?
        .json::<BitgetAccountAssetsResponse>()
        .await
        .map_err(|error| ExchangeError::Http(error.to_string()))?;

    if response.msg != "success" {
        return Err(ExchangeError::Api(response.msg));
    }

    Ok(ExchangeSummary {
        components: vec![CapitalComponent {
            name: "uta".to_owned(),
            equity_usd: parse_number(&response.data.usdt_equity),
        }],
    })
}

fn signature(
    timestamp: &str,
    method: &str,
    path: &str,
    query: &str,
    body: &str,
    secret: &str,
) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key size");
    mac.update(format!("{timestamp}{method}{path}{query}{body}").as_bytes());
    BASE64.encode(mac.finalize().into_bytes())
}

#[derive(Debug, Deserialize)]
struct BitgetAccountAssetsResponse {
    msg: String,
    data: BitgetAccountAssets,
}

#[derive(Debug, Deserialize)]
struct BitgetAccountAssets {
    #[serde(rename = "usdtEquity")]
    usdt_equity: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_is_stable() {
        assert_eq!(
            signature(
                "1780963200000",
                "GET",
                "/api/v3/account/assets",
                "",
                "",
                "secret"
            ),
            "ChcsAoPz0D1XUEmHny8yjk1Jp1viMyMN7iZxB+qY9VA="
        );
    }
}
