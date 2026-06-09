use crate::{
    capital::{CapitalSummary, summarize_credential, summarize_vault},
    credentials::{CredentialVault, CredentialView, RegisterCredentialRequest},
    domain::{CreateFundRequest, FundEvent, FundRecord},
    storage::{Database, StorageError},
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Serialize;
use std::{collections::BTreeMap, path::Path as FilePath, sync::Arc};
use tokio::sync::RwLock;
use utoipa::ToSchema;

pub type SharedStore = Arc<RwLock<Store>>;

pub struct Store {
    funds: BTreeMap<String, FundRecord>,
    credentials: CredentialVault,
    database: Database,
}

impl Store {
    pub fn open(path: impl AsRef<FilePath>) -> Result<Self, StorageError> {
        Self::from_database(Database::open(path)?)
    }

    fn from_database(database: Database) -> Result<Self, StorageError> {
        Ok(Self {
            funds: database
                .load_funds()?
                .into_iter()
                .map(|fund| (fund.account_id.clone(), fund))
                .collect(),
            credentials: database.load_credentials()?,
            database,
        })
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::from_database(Database::memory().expect("open in-memory sqlite database"))
            .expect("load in-memory sqlite database")
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    message: String,
}

#[derive(Debug)]
enum ApiError {
    Conflict(String),
    NotFound(String),
    Internal(String),
}

pub fn router(store: SharedStore) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/funds", post(create_fund).get(list_funds))
        .route("/funds/{account_id}", get(get_fund))
        .route("/funds/{account_id}/events", post(append_event))
        .route(
            "/credentials",
            post(register_credential).get(list_credentials),
        )
        .route(
            "/credentials/{credential_id}/capital-summary",
            get(get_credential_capital_summary),
        )
        .route("/capital-summary", get(get_capital_summary))
        .with_state(store)
}

#[utoipa::path(get, path = "/health", responses((status = 200, body = String)))]
pub async fn health() -> &'static str {
    "ok"
}

#[utoipa::path(
    post,
    path = "/funds",
    request_body = CreateFundRequest,
    responses(
        (status = 201, body = FundRecord),
        (status = 409, body = ErrorBody)
    )
)]
async fn create_fund(
    State(store): State<SharedStore>,
    Json(request): Json<CreateFundRequest>,
) -> Result<(StatusCode, Json<FundRecord>), ApiError> {
    let mut store = store.write().await;

    if store.funds.contains_key(&request.account_id) {
        return Err(ApiError::Conflict(format!(
            "fund '{}' already exists",
            request.account_id
        )));
    }

    let record = FundRecord::new(request.account_id.clone(), request.description);
    store.database.save_fund(&record)?;
    store.funds.insert(request.account_id, record.clone());

    Ok((StatusCode::CREATED, Json(record)))
}

#[utoipa::path(get, path = "/funds", responses((status = 200, body = Vec<FundRecord>)))]
async fn list_funds(State(store): State<SharedStore>) -> Json<Vec<FundRecord>> {
    let store = store.read().await;
    Json(store.funds.values().cloned().collect())
}

#[utoipa::path(
    get,
    path = "/funds/{account_id}",
    params(("account_id" = String, Path, description = "Fund account id")),
    responses((status = 200, body = FundRecord), (status = 404, body = ErrorBody))
)]
async fn get_fund(
    State(store): State<SharedStore>,
    Path(account_id): Path<String>,
) -> Result<Json<FundRecord>, ApiError> {
    let store = store.read().await;
    let record = store
        .funds
        .get(&account_id)
        .cloned()
        .ok_or_else(|| ApiError::NotFound(format!("fund '{account_id}' was not found")))?;

    Ok(Json(record))
}

#[utoipa::path(
    post,
    path = "/funds/{account_id}/events",
    params(("account_id" = String, Path, description = "Fund account id")),
    request_body = FundEvent,
    responses((status = 200, body = FundRecord), (status = 404, body = ErrorBody))
)]
async fn append_event(
    State(store): State<SharedStore>,
    Path(account_id): Path<String>,
    Json(event): Json<FundEvent>,
) -> Result<Json<FundRecord>, ApiError> {
    let mut store = store.write().await;
    let record = store
        .funds
        .get_mut(&account_id)
        .ok_or_else(|| ApiError::NotFound(format!("fund '{account_id}' was not found")))?;

    record.append(event);
    let saved = record.clone();
    store.database.save_fund(&saved)?;

    Ok(Json(saved))
}

#[utoipa::path(
    post,
    path = "/credentials",
    request_body = RegisterCredentialRequest,
    responses((status = 201, body = CredentialView))
)]
async fn register_credential(
    State(store): State<SharedStore>,
    Json(request): Json<RegisterCredentialRequest>,
) -> Result<(StatusCode, Json<CredentialView>), ApiError> {
    let mut store = store.write().await;
    let credential = store.credentials.register(request);
    store.database.save_credential(&credential)?;

    Ok((StatusCode::CREATED, Json(credential.view())))
}

#[utoipa::path(get, path = "/credentials", responses((status = 200, body = Vec<CredentialView>)))]
async fn list_credentials(State(store): State<SharedStore>) -> Json<Vec<CredentialView>> {
    let store = store.read().await;

    Json(store.credentials.list())
}

#[utoipa::path(get, path = "/capital-summary", responses((status = 200, body = CapitalSummary)))]
async fn get_capital_summary(State(store): State<SharedStore>) -> Json<CapitalSummary> {
    let credentials = {
        let store = store.read().await;
        store.credentials.clone()
    };

    Json(summarize_vault(&credentials).await)
}

#[utoipa::path(
    get,
    path = "/credentials/{credential_id}/capital-summary",
    params(("credential_id" = String, Path, description = "Credential id")),
    responses((status = 200, body = crate::capital::AccountCapitalSummary), (status = 404, body = ErrorBody))
)]
async fn get_credential_capital_summary(
    State(store): State<SharedStore>,
    Path(credential_id): Path<String>,
) -> Result<Json<crate::capital::AccountCapitalSummary>, ApiError> {
    let credential = {
        let store = store.read().await;
        store
            .credentials
            .get(&credential_id)
            .cloned()
            .ok_or_else(|| {
                ApiError::NotFound(format!("credential '{credential_id}' was not found"))
            })?
    };

    Ok(Json(summarize_credential(&credential).await))
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::Conflict(message) => (StatusCode::CONFLICT, message),
            ApiError::NotFound(message) => (StatusCode::NOT_FOUND, message),
            ApiError::Internal(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
        };

        (status, Json(ErrorBody { message })).into_response()
    }
}

impl From<StorageError> for ApiError {
    fn from(error: StorageError) -> Self {
        Self::Internal(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, header::CONTENT_TYPE},
    };
    use serde_json::{Value, json};
    use tower::ServiceExt;

    #[tokio::test]
    async fn create_fund_returns_created_record() {
        let app = test_app();
        let response = app
            .oneshot(json_request(
                "POST",
                "/funds",
                json!({
                    "account_id": "fund/main",
                    "description": "Main fund"
                }),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let body = response_body(response).await;
        assert_eq!(body["account_id"], "fund/main");
        assert_eq!(body["state"]["summary"]["unit_price"], 1.0);
    }

    #[tokio::test]
    async fn duplicate_fund_returns_conflict() {
        let app = test_app();
        let request_body = json!({
            "account_id": "fund/main",
            "description": "Main fund"
        });

        app.clone()
            .oneshot(json_request("POST", "/funds", request_body.clone()))
            .await
            .unwrap();
        let response = app
            .oneshot(json_request("POST", "/funds", request_body))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn append_event_updates_fund_state() {
        let app = test_app();

        app.clone()
            .oneshot(json_request(
                "POST",
                "/funds",
                json!({
                    "account_id": "fund/main",
                    "description": "Main fund"
                }),
            ))
            .await
            .unwrap();
        let response = app
            .oneshot(json_request(
                "POST",
                "/funds/fund%2Fmain/events",
                json!({
                    "updated_at": "2026-06-09T00:00:00Z",
                    "comment": null,
                    "fund_equity": null,
                    "order": {
                        "name": "Alice",
                        "deposit": 100.0
                    },
                    "investor": null,
                    "taxation": null
                }),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response_body(response).await;
        assert_eq!(body["events"].as_array().unwrap().len(), 1);
        assert_eq!(body["state"]["total_assets"], 100.0);
        assert_eq!(body["state"]["investors"]["Alice"]["share"], 100.0);
    }

    #[tokio::test]
    async fn get_missing_fund_returns_not_found() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/funds/missing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn register_credential_hides_payload() {
        let response = test_app()
            .oneshot(json_request(
                "POST",
                "/credentials",
                json!({
                    "label": "OKX main",
                    "exchange": "okx",
                    "payload": {
                        "access_key": "access",
                        "secret_key": "secret",
                        "passphrase": "passphrase"
                    }
                }),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let body = response_body(response).await;
        assert_eq!(body["id"], "credential-1");
        assert_eq!(body.get("payload"), None);
    }

    fn test_app() -> Router {
        router(Arc::new(RwLock::new(Store::default())))
    }

    fn json_request(method: &str, uri: &str, body: Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    async fn response_body(response: Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }
}
