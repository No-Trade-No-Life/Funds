use super::{ExchangeError, ExchangeSummary, HmacSha256, required, url_encode};
use crate::capital::CapitalComponent;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use chrono::Utc;
use hmac::Mac;
use serde::Deserialize;
use serde_json::Value;

pub async fn fetch_summary(payload: &Value) -> Result<ExchangeSummary, ExchangeError> {
    let access_key = required(payload, "access_key")?;
    let secret_key = required(payload, "secret_key")?;
    let path = "/linear-swap-api/v3/unified_account_info";
    let request_params = request_params(access_key);
    let signature = signature("GET", "api.hbdm.com", path, &request_params, secret_key);
    let url = format!(
        "https://api.hbdm.com{path}?{request_params}&Signature={}",
        url_encode(&signature)
    );

    let response = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .map_err(|error| ExchangeError::Http(error.to_string()))?
        .json::<HtxUnifiedAccountResponse>()
        .await
        .map_err(|error| ExchangeError::Http(error.to_string()))?;

    if response.status != "ok" {
        return Err(ExchangeError::Api(response.msg.unwrap_or(response.status)));
    }

    Ok(ExchangeSummary {
        components: response
            .data
            .into_iter()
            .map(|account| CapitalComponent {
                name: account.margin_asset,
                equity_usd: account.margin_balance,
            })
            .collect(),
    })
}

fn request_params(access_key: &str) -> String {
    format!(
        "AccessKeyId={}&SignatureMethod=HmacSHA256&SignatureVersion=2&Timestamp={}",
        url_encode(access_key),
        url_encode(&Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string())
    )
}

fn signature(method: &str, host: &str, path: &str, request_params: &str, secret: &str) -> String {
    let request_string = format!("{method}\n{host}\n{path}\n{request_params}");
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key size");
    mac.update(request_string.as_bytes());
    BASE64.encode(mac.finalize().into_bytes())
}

#[derive(Debug, Deserialize)]
struct HtxUnifiedAccountResponse {
    status: String,
    msg: Option<String>,
    data: Vec<HtxUnifiedAccount>,
}

#[derive(Debug, Deserialize)]
struct HtxUnifiedAccount {
    margin_asset: String,
    margin_balance: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_is_stable() {
        assert_eq!(
            signature(
                "GET",
                "api.hbdm.com",
                "/linear-swap-api/v3/unified_account_info",
                "AccessKeyId=access&SignatureMethod=HmacSHA256&SignatureVersion=2&Timestamp=2026-06-09T00%3A00%3A00",
                "secret"
            ),
            "lO4ZTPoSwRBYYk7qXl40fIIMSCSspmM5DIruajsE7Fs="
        );
    }
}
