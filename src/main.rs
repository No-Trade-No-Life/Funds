#![forbid(unsafe_code)]

mod api;
mod domain;
mod proxy;

use crate::{api::Store, domain::*};
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
        api::append_event
    ),
    components(schemas(
        api::ErrorBody,
        CreateFundRequest,
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
    let store = Arc::new(RwLock::new(Store::default()));

    api::router(store)
        .merge(SwaggerUi::new("/docs").url("/api-doc/openapi.json", ApiDoc::openapi()))
        .fallback(proxy::frontend_proxy)
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("install Ctrl-C signal handler");
}
