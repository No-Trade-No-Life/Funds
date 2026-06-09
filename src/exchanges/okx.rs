use super::{ExchangeError, ExchangeSummary, HmacSha256, insert_header, parse_number, required};
use crate::capital::CapitalComponent;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use chrono::{SecondsFormat, Utc};
use hmac::Mac;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use serde_json::Value;

pub async fn fetch_summary(payload: &Value) -> Result<ExchangeSummary, ExchangeError> {
    let access_key = required(payload, "access_key")?;
    let secret_key = required(payload, "secret_key")?;
    let passphrase = required(payload, "passphrase")?;
    let path = "/api/v5/asset/asset-valuation";
    let query = "?ccy=USDT";
    let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let signature = signature(&timestamp, "GET", path, query, "", secret_key);
    let mut headers = HeaderMap::new();
    insert_header(&mut headers, "OK-ACCESS-KEY", access_key)?;
    insert_header(&mut headers, "OK-ACCESS-SIGN", &signature)?;
    insert_header(&mut headers, "OK-ACCESS-TIMESTAMP", &timestamp)?;
    insert_header(&mut headers, "OK-ACCESS-PASSPHRASE", passphrase)?;

    let response = reqwest::Client::new()
        .get(format!("https://www.okx.com{path}{query}"))
        .headers(headers)
        .send()
        .await
        .map_err(|error| ExchangeError::Http(error.to_string()))?
        .json::<OkxAssetValuationResponse>()
        .await
        .map_err(|error| ExchangeError::Http(error.to_string()))?;

    if response.code != "0" {
        return Err(ExchangeError::Api(response.msg));
    }

    let valuation = response
        .data
        .first()
        .ok_or_else(|| ExchangeError::Api("OKX asset valuation response is empty".to_owned()))?;

    Ok(ExchangeSummary {
        components: vec![CapitalComponent {
            name: "total".to_owned(),
            equity_usd: parse_number(&valuation.total_bal),
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
struct OkxAssetValuationResponse {
    code: String,
    msg: String,
    data: Vec<OkxAssetValuation>,
}

#[derive(Debug, Deserialize)]
struct OkxAssetValuation {
    #[serde(rename = "totalBal")]
    total_bal: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_is_stable() {
        assert_eq!(
            signature(
                "2026-06-09T00:00:00.000Z",
                "GET",
                "/api/v5/asset/asset-valuation",
                "?ccy=USDT",
                "",
                "secret"
            ),
            "Ut4CdXigjrCvi2L0kr5VHJY7AzgnJmNKS0+gPV2QKXY="
        );
    }
}
