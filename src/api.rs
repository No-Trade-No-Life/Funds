use crate::domain::{CreateFundRequest, FundEvent, FundRecord};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Serialize;
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::RwLock;
use utoipa::ToSchema;

pub type SharedStore = Arc<RwLock<Store>>;

#[derive(Default)]
pub struct Store {
    funds: BTreeMap<String, FundRecord>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    message: String,
}

#[derive(Debug)]
enum ApiError {
    Conflict(String),
    NotFound(String),
}

pub fn router(store: SharedStore) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/funds", post(create_fund).get(list_funds))
        .route("/funds/{account_id}", get(get_fund))
        .route("/funds/{account_id}/events", post(append_event))
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

    Ok(Json(record.clone()))
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::Conflict(message) => (StatusCode::CONFLICT, message),
            ApiError::NotFound(message) => (StatusCode::NOT_FOUND, message),
        };

        (status, Json(ErrorBody { message })).into_response()
    }
}
