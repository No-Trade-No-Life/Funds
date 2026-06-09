#![forbid(unsafe_code)]

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    net::SocketAddr,
    sync::Arc,
};
use tokio::sync::RwLock;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

type SharedStore = Arc<RwLock<Store>>;

#[derive(Default)]
struct Store {
    funds: BTreeMap<String, FundRecord>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
struct FundRecord {
    account_id: String,
    description: String,
    state: FundState,
    events: Vec<FundEvent>,
}

#[derive(Debug, Deserialize, ToSchema)]
struct CreateFundRequest {
    account_id: String,
    description: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
struct FundEvent {
    updated_at: DateTime<Utc>,
    comment: Option<String>,
    fund_equity: Option<FundEquity>,
    order: Option<InvestorOrder>,
    investor: Option<InvestorUpdate>,
    taxation: Option<TaxationKind>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
struct FundEquity {
    equity: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
struct InvestorOrder {
    name: String,
    deposit: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
struct InvestorUpdate {
    name: String,
    tax_rate: Option<f64>,
    add_tax_threshold: Option<f64>,
    referrer: Option<String>,
    referrer_rebate_rate: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
enum TaxationKind {
    Legacy,
    PreserveFundAssets,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
struct FundState {
    account_id: String,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
    description: String,
    total_assets: f64,
    total_taxed: f64,
    summary: FundSummary,
    investors: BTreeMap<String, InvestorMeta>,
    investor_derived: BTreeMap<String, InvestorDerived>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
struct FundSummary {
    total_deposit: f64,
    total_share: f64,
    total_tax: f64,
    unit_price: f64,
    total_profit: f64,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
struct InvestorMeta {
    name: String,
    share: f64,
    tax_threshold: f64,
    deposit: f64,
    tax_rate: f64,
    avg_cost_price: f64,
    referrer: Option<String>,
    referrer_rebate_rate: f64,
    claimed_referrer_rebate: f64,
    taxed: f64,
    created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
struct InvestorDerived {
    pre_tax_assets: f64,
    taxable: f64,
    tax: f64,
    after_tax_assets: f64,
    after_tax_profit: f64,
    after_tax_share: f64,
    floating_profit_rate: f64,
    share_ratio: f64,
}

#[derive(Debug, Serialize, ToSchema)]
struct ErrorBody {
    message: String,
}

#[derive(Debug)]
enum ApiError {
    Conflict(String),
    NotFound(String),
}

#[derive(OpenApi)]
#[openapi(
    paths(health, create_fund, list_funds, get_fund, append_event),
    components(schemas(
        CreateFundRequest,
        ErrorBody,
        FundEquity,
        FundEvent,
        FundRecord,
        FundState,
        FundSummary,
        InvestorDerived,
        InvestorMeta,
        InvestorOrder,
        InvestorUpdate,
        TaxationKind
    )),
    tags((name = "funds", description = "Fund and investor relationship management"))
)]
struct ApiDoc;

#[tokio::main]
async fn main() {
    let app = app(Arc::new(RwLock::new(Store::default())));
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind HTTP listener");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("serve HTTP API");
}

fn app(store: SharedStore) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/funds", post(create_fund).get(list_funds))
        .route("/funds/{account_id}", get(get_fund))
        .route("/funds/{account_id}/events", post(append_event))
        .merge(SwaggerUi::new("/docs").url("/api-doc/openapi.json", ApiDoc::openapi()))
        .with_state(store)
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("install Ctrl-C signal handler");
}

#[utoipa::path(get, path = "/health", responses((status = 200, body = String)))]
async fn health() -> &'static str {
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

    let record = FundRecord {
        account_id: request.account_id.clone(),
        description: request.description.clone(),
        state: FundState::new(request.account_id.clone(), request.description),
        events: Vec::new(),
    };

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

    record.state.apply(&event);
    record.events.push(event);

    Ok(Json(record.clone()))
}

impl FundState {
    fn new(account_id: String, description: String) -> Self {
        Self {
            account_id,
            created_at: None,
            updated_at: None,
            description,
            total_assets: 0.0,
            total_taxed: 0.0,
            summary: FundSummary {
                total_deposit: 0.0,
                total_share: 0.0,
                total_tax: 0.0,
                unit_price: 1.0,
                total_profit: 0.0,
            },
            investors: BTreeMap::new(),
            investor_derived: BTreeMap::new(),
        }
    }

    fn apply(&mut self, event: &FundEvent) {
        self.created_at.get_or_insert(event.updated_at);
        self.updated_at = Some(event.updated_at);

        if let Some(comment) = &event.comment {
            self.description = comment.clone();
        }

        if let Some(fund_equity) = &event.fund_equity {
            self.total_assets = fund_equity.equity;
        }

        if let Some(order) = &event.order {
            self.apply_order(order, event.updated_at);
        }

        if let Some(investor) = &event.investor {
            self.apply_investor_update(investor, event.updated_at);
        }

        if let Some(taxation) = &event.taxation {
            self.apply_taxation(taxation, event.updated_at);
        }

        self.recalculate();
    }

    fn apply_order(&mut self, order: &InvestorOrder, updated_at: DateTime<Utc>) {
        let unit_price = self.summary.unit_price;
        let investor = ensure_investor(&mut self.investors, &order.name, updated_at);
        let share = order.deposit / unit_price;

        if share > 0.0 {
            investor.avg_cost_price = (investor.avg_cost_price * investor.share + order.deposit)
                / (investor.share + share);
        }

        investor.deposit += order.deposit;
        investor.tax_threshold += order.deposit;
        investor.share += share;
        self.total_assets += order.deposit;
    }

    fn apply_investor_update(&mut self, update: &InvestorUpdate, updated_at: DateTime<Utc>) {
        let investor = ensure_investor(&mut self.investors, &update.name, updated_at);

        if let Some(tax_rate) = update.tax_rate {
            investor.tax_rate = tax_rate;
        }

        if let Some(add_tax_threshold) = update.add_tax_threshold {
            investor.tax_threshold += add_tax_threshold;
        }

        if let Some(referrer) = &update.referrer {
            investor.referrer = Some(referrer.clone());
        }

        if let Some(referrer_rebate_rate) = update.referrer_rebate_rate {
            investor.referrer_rebate_rate = referrer_rebate_rate;
        }
    }

    fn apply_taxation(&mut self, taxation: &TaxationKind, updated_at: DateTime<Utc>) {
        let snapshot = self.investor_derived.clone();

        match taxation {
            TaxationKind::Legacy => {
                for investor in self.investors.values_mut() {
                    let Some(derived) = snapshot.get(&investor.name) else {
                        continue;
                    };
                    investor.share = derived.after_tax_share;
                    investor.tax_threshold = derived.after_tax_assets;
                    self.total_assets -= derived.tax;
                    self.total_taxed += derived.tax;
                }
            }
            TaxationKind::PreserveFundAssets => {
                let investor_names: BTreeSet<String> = self.investors.keys().cloned().collect();
                let mut total_tax_share = 0.0;
                let mut rebates = Vec::new();

                for investor in self.investors.values_mut() {
                    let Some(derived) = snapshot.get(&investor.name) else {
                        continue;
                    };
                    let tax_share = investor.share - derived.after_tax_share;
                    let rebate_share = investor.referrer.as_ref().map_or(0.0, |referrer| {
                        rebate_share(
                            referrer,
                            &investor_names,
                            tax_share,
                            investor.referrer_rebate_rate,
                        )
                    });

                    if rebate_share != 0.0
                        && let Some(referrer) = &investor.referrer
                    {
                        rebates.push((referrer.clone(), rebate_share));
                    }

                    investor.share = derived.after_tax_share;
                    investor.tax_threshold = derived.after_tax_assets;
                    investor.taxed += derived.tax;
                    total_tax_share += tax_share - rebate_share;
                    self.total_taxed += derived.tax;
                }

                let unit_price = self.summary.unit_price;
                for (referrer, rebate_share) in rebates {
                    let rebate_value = rebate_share * unit_price;
                    let investor = ensure_investor(&mut self.investors, &referrer, updated_at);
                    investor.share += rebate_share;
                    investor.tax_threshold += rebate_value;
                    investor.claimed_referrer_rebate += rebate_value;
                }

                let tax_account = ensure_investor(&mut self.investors, "@tax", updated_at);
                tax_account.share += total_tax_share;
                tax_account.tax_threshold += total_tax_share * unit_price;
            }
        }
    }

    fn recalculate(&mut self) {
        self.summary.total_share = self.investors.values().map(|investor| investor.share).sum();
        self.summary.unit_price = unit_price(self.total_assets, self.summary.total_share);

        self.investor_derived = self
            .investors
            .values()
            .map(|investor| {
                let derived =
                    derive_investor(investor, self.summary.unit_price, self.summary.total_share);
                (investor.name.clone(), derived)
            })
            .collect();

        self.summary.total_deposit = self
            .investors
            .values()
            .map(|investor| investor.deposit)
            .sum();
        self.summary.total_tax = self
            .investor_derived
            .values()
            .map(|derived| derived.tax)
            .sum();
        self.summary.total_profit =
            self.total_assets - self.summary.total_deposit + self.total_taxed;
    }
}

fn ensure_investor<'a>(
    investors: &'a mut BTreeMap<String, InvestorMeta>,
    name: &str,
    created_at: DateTime<Utc>,
) -> &'a mut InvestorMeta {
    investors
        .entry(name.to_owned())
        .or_insert_with(|| InvestorMeta {
            name: name.to_owned(),
            share: 0.0,
            tax_threshold: 0.0,
            deposit: 0.0,
            tax_rate: 0.0,
            avg_cost_price: 1.0,
            referrer: None,
            referrer_rebate_rate: 0.0,
            claimed_referrer_rebate: 0.0,
            taxed: 0.0,
            created_at,
        })
}

fn unit_price(total_assets: f64, total_share: f64) -> f64 {
    if total_share == 0.0 {
        return 1.0;
    }

    total_assets / total_share
}

fn derive_investor(investor: &InvestorMeta, unit_price: f64, total_share: f64) -> InvestorDerived {
    let pre_tax_assets = investor.share * unit_price;
    let taxable = pre_tax_assets - investor.tax_threshold;
    let tax = taxable.max(0.0) * investor.tax_rate;
    let after_tax_assets = pre_tax_assets - tax;
    let after_tax_profit = after_tax_assets - investor.deposit;
    let after_tax_share = after_tax_assets / unit_price;

    InvestorDerived {
        pre_tax_assets,
        taxable,
        tax,
        after_tax_assets,
        after_tax_profit,
        after_tax_share,
        floating_profit_rate: unit_price / investor.avg_cost_price - 1.0,
        share_ratio: share_ratio(investor.share, total_share),
    }
}

fn share_ratio(share: f64, total_share: f64) -> f64 {
    if total_share == 0.0 {
        return 0.0;
    }

    share / total_share
}

fn rebate_share(
    referrer: &str,
    investor_names: &BTreeSet<String>,
    tax_share: f64,
    rate: f64,
) -> f64 {
    if !investor_names.contains(referrer) {
        return 0.0;
    }

    tax_share * rate
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_creates_investor_share() {
        let updated_at = "2026-06-09T00:00:00Z".parse().unwrap();
        let mut state = FundState::new("fund/main".to_owned(), "Main fund".to_owned());

        state.apply(&FundEvent {
            updated_at,
            comment: None,
            fund_equity: None,
            order: Some(InvestorOrder {
                name: "Alice".to_owned(),
                deposit: 100.0,
            }),
            investor: None,
            taxation: None,
        });

        assert_eq!(state.total_assets, 100.0);
        assert_eq!(state.summary.total_share, 100.0);
        assert_eq!(state.investors["Alice"].share, 100.0);
    }

    #[test]
    fn tax_event_moves_tax_to_tax_account() {
        let updated_at = "2026-06-09T00:00:00Z".parse().unwrap();
        let mut state = FundState::new("fund/main".to_owned(), "Main fund".to_owned());

        state.apply(&FundEvent {
            updated_at,
            comment: None,
            fund_equity: None,
            order: Some(InvestorOrder {
                name: "Alice".to_owned(),
                deposit: 100.0,
            }),
            investor: Some(InvestorUpdate {
                name: "Alice".to_owned(),
                tax_rate: Some(0.2),
                add_tax_threshold: None,
                referrer: None,
                referrer_rebate_rate: None,
            }),
            taxation: None,
        });
        state.apply(&FundEvent {
            updated_at,
            comment: None,
            fund_equity: Some(FundEquity { equity: 200.0 }),
            order: None,
            investor: None,
            taxation: None,
        });
        state.apply(&FundEvent {
            updated_at,
            comment: None,
            fund_equity: None,
            order: None,
            investor: None,
            taxation: Some(TaxationKind::PreserveFundAssets),
        });

        assert_eq!(state.total_assets, 200.0);
        assert_eq!(state.investors["@tax"].share, 10.0);
        assert_eq!(state.investors["Alice"].taxed, 20.0);
    }
}
