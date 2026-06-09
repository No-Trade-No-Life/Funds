use super::{
    ExchangeError, ExchangeSummary, hmac_sha512_hex, insert_header, parse_number, required,
};
use crate::capital::CapitalComponent;
use chrono::Utc;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha512};

pub async fn fetch_summary(payload: &Value) -> Result<ExchangeSummary, ExchangeError> {
    let access_key = required(payload, "access_key")?;
    let secret_key = required(payload, "secret_key")?;
    let path = "/api/v4/unified/accounts";
    let timestamp = format!("{}", Utc::now().timestamp());
    let signature = signature("GET", path, "", "", &timestamp, secret_key);
    let mut headers = HeaderMap::new();
    insert_header(&mut headers, "KEY", access_key)?;
    insert_header(&mut headers, "SIGN", &signature)?;
    insert_header(&mut headers, "Timestamp", &timestamp)?;

    let response = reqwest::Client::new()
        .get("https://api.gateio.ws/api/v4/unified/accounts")
        .headers(headers)
        .send()
        .await
        .map_err(|error| ExchangeError::Http(error.to_string()))?
        .json::<GateUnifiedAccountResponse>()
        .await
        .map_err(|error| ExchangeError::Http(error.to_string()))?;

    Ok(ExchangeSummary {
        components: vec![CapitalComponent {
            name: "unified".to_owned(),
            equity_usd: parse_number(&response.unified_account_total_equity),
        }],
    })
}

fn signature(
    method: &str,
    path: &str,
    query: &str,
    body: &str,
    timestamp: &str,
    secret: &str,
) -> String {
    let body_hash = hex::encode(Sha512::digest(body.as_bytes()));
    let sign_target = format!("{method}\n{path}\n{query}\n{body_hash}\n{timestamp}");
    hmac_sha512_hex(secret, &sign_target)
}

#[derive(Debug, Deserialize)]
struct GateUnifiedAccountResponse {
    unified_account_total_equity: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_uses_body_hash() {
        assert_eq!(
            signature(
                "GET",
                "/api/v4/unified/accounts",
                "",
                "",
                "1780963200",
                "secret"
            ),
            "151a375f9efc52dc9d7bf7d01078566eb786697087df34a1b6e07ff734dd7a3002e20047dc525f56bfd647b8334dc5ff155f9f8a8aeda5a0691b01ea5f5ee884"
        );
    }
}
