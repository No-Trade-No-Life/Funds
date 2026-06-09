use crate::{capital::CapitalComponent, credentials::CredentialRecord, credentials::ExchangeKind};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use chrono::{SecondsFormat, Utc};
use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256, Sha512};
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
        ExchangeKind::Okx => fetch_okx_summary(&credential.payload).await,
        ExchangeKind::Gate => fetch_gate_summary(&credential.payload).await,
        ExchangeKind::Binance => fetch_binance_summary(&credential.payload).await,
        ExchangeKind::Aster => fetch_aster_summary(&credential.payload).await,
        ExchangeKind::Hyperliquid => fetch_hyperliquid_summary(&credential.payload).await,
        ExchangeKind::Bitget => fetch_bitget_summary(&credential.payload).await,
        ExchangeKind::Htx => fetch_htx_summary(&credential.payload).await,
    }
}

async fn fetch_okx_summary(payload: &Value) -> Result<ExchangeSummary, ExchangeError> {
    let access_key = required(payload, "access_key")?;
    let secret_key = required(payload, "secret_key")?;
    let passphrase = required(payload, "passphrase")?;
    let path = "/api/v5/asset/asset-valuation";
    let query = "?ccy=USDT";
    let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let signature = okx_signature(&timestamp, "GET", path, query, "", secret_key);
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

async fn fetch_gate_summary(payload: &Value) -> Result<ExchangeSummary, ExchangeError> {
    let access_key = required(payload, "access_key")?;
    let secret_key = required(payload, "secret_key")?;
    let path = "/api/v4/unified/accounts";
    let timestamp = format!("{}", Utc::now().timestamp());
    let signature = gate_signature("GET", path, "", "", &timestamp, secret_key);
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

async fn fetch_binance_summary(payload: &Value) -> Result<ExchangeSummary, ExchangeError> {
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

async fn fetch_aster_summary(payload: &Value) -> Result<ExchangeSummary, ExchangeError> {
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

async fn fetch_hyperliquid_summary(payload: &Value) -> Result<ExchangeSummary, ExchangeError> {
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

async fn fetch_bitget_summary(payload: &Value) -> Result<ExchangeSummary, ExchangeError> {
    let access_key = required(payload, "access_key")?;
    let secret_key = required(payload, "secret_key")?;
    let passphrase = required(payload, "passphrase")?;
    let path = "/api/v3/account/assets";
    let timestamp = Utc::now().timestamp_millis().to_string();
    let signature = bitget_signature(&timestamp, "GET", path, "", "", secret_key);
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

async fn fetch_htx_summary(payload: &Value) -> Result<ExchangeSummary, ExchangeError> {
    let access_key = required(payload, "access_key")?;
    let secret_key = required(payload, "secret_key")?;
    let path = "/linear-swap-api/v3/unified_account_info";
    let request_params = htx_request_params(access_key);
    let signature = htx_signature("GET", "api.hbdm.com", path, &request_params, secret_key);
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

fn okx_signature(
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

fn gate_signature(
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

fn bitget_signature(
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

fn htx_request_params(access_key: &str) -> String {
    format!(
        "AccessKeyId={}&SignatureMethod=HmacSHA256&SignatureVersion=2&Timestamp={}",
        url_encode(access_key),
        url_encode(&Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string())
    )
}

fn htx_signature(
    method: &str,
    host: &str,
    path: &str,
    request_params: &str,
    secret: &str,
) -> String {
    let request_string = format!("{method}\n{host}\n{path}\n{request_params}");
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key size");
    mac.update(request_string.as_bytes());
    BASE64.encode(mac.finalize().into_bytes())
}

fn signed_query(secret: &str) -> String {
    let query = format!(
        "timestamp={}&recvWindow=5000",
        Utc::now().timestamp_millis()
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

#[derive(Debug, Deserialize)]
struct GateUnifiedAccountResponse {
    unified_account_total_equity: String,
}

#[derive(Debug, Deserialize)]
struct BinanceBalanceEntry {
    asset: String,
    #[serde(rename = "totalWalletBalance")]
    total_wallet_balance: String,
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
    fn okx_signature_is_stable() {
        assert_eq!(
            okx_signature(
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

    #[test]
    fn gate_signature_uses_body_hash() {
        assert_eq!(
            gate_signature(
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

    #[test]
    fn bitget_signature_is_stable() {
        assert_eq!(
            bitget_signature(
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

    #[test]
    fn htx_signature_is_stable() {
        assert_eq!(
            htx_signature(
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
