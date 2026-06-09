#![forbid(unsafe_code)]

mod api;
mod capital;
mod credentials;
mod domain;
mod exchanges;
mod proxy;
mod storage;

use crate::{api::Store, capital::*, credentials::*, domain::*};
use axum::Router;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        api::health,
        api::create_fund,
        api::list_funds,
        api::get_fund,
        api::append_event,
        api::register_credential,
        api::list_credentials,
        api::get_credential_capital_summary,
        api::get_capital_summary
    ),
    components(schemas(
        AccountCapitalSummary,
        CapitalComponent,
        CapitalSummary,
        api::ErrorBody,
        CredentialView,
        CreateFundRequest,
        ExchangeKind,
        FundEquity,
        FundEvent,
        FundRecord,
        FundState,
        FundSummary,
        InvestorDerived,
        InvestorMeta,
        InvestorOrder,
        InvestorUpdate,
        RegisterCredentialRequest,
        SummaryStatus,
        TaxationKind
    )),
    tags((name = "funds", description = "Fund and investor relationship management"))
)]
struct ApiDoc;

#[tokio::main]
async fn main() {
    let app = app();
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind HTTP listener");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("serve HTTP API");
}

fn app() -> Router {
    let store = Arc::new(RwLock::new(
        Store::open(database_path()).expect("open sqlite database"),
    ));

    api::router(store)
        .merge(SwaggerUi::new("/docs").url("/api-doc/openapi.json", ApiDoc::openapi()))
        .fallback(proxy::frontend_proxy)
}

fn database_path() -> String {
    std::env::var("DATABASE_PATH").unwrap_or_else(|_| "funds.sqlite".to_owned())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("install Ctrl-C signal handler");
}
