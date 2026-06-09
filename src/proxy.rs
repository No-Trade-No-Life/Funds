use axum::{
    body::{Body, to_bytes},
    extract::Request,
    http::{HeaderMap, StatusCode, Uri, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
};

const DEFAULT_FRONTEND_ORIGIN: &str = "http://127.0.0.1:5173";

pub async fn frontend_proxy(request: Request) -> Response {
    let origin = frontend_origin();
    let target = target_url(&origin, request.uri());
    let method = request.method().clone();
    let headers = request.headers().clone();
    let body = request.into_body();
    let bytes = match to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(error) => return bad_gateway(format!("read frontend proxy request body: {error}")),
    };

    let response = match reqwest::Client::new()
        .request(method, target)
        .headers(forward_headers(headers))
        .body(bytes)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => return bad_gateway(format!("frontend dev server is unavailable: {error}")),
    };

    let status = response.status();
    let content_type = response.headers().get(CONTENT_TYPE).cloned();
    let bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(error) => return bad_gateway(format!("read frontend proxy response body: {error}")),
    };

    let mut proxied = Response::new(Body::from(bytes));
    *proxied.status_mut() = status;

    if let Some(content_type) = content_type {
        proxied.headers_mut().insert(CONTENT_TYPE, content_type);
    }

    proxied
}

fn frontend_origin() -> String {
    std::env::var("FRONTEND_ORIGIN").unwrap_or_else(|_| DEFAULT_FRONTEND_ORIGIN.to_owned())
}

fn target_url(origin: &str, uri: &Uri) -> String {
    let path = uri.path_and_query().map_or("/", |path| path.as_str());
    format!("{origin}{path}")
}

fn forward_headers(headers: HeaderMap) -> HeaderMap {
    let mut forwarded = HeaderMap::new();

    if let Some(content_type) = headers.get(CONTENT_TYPE) {
        forwarded.insert(CONTENT_TYPE, content_type.clone());
    }

    forwarded
}

fn bad_gateway(message: String) -> Response {
    (StatusCode::BAD_GATEWAY, message).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_url_preserves_path_and_query() {
        let uri = "/assets/index.js?v=1".parse().unwrap();

        assert_eq!(
            target_url("http://127.0.0.1:5173", &uri),
            "http://127.0.0.1:5173/assets/index.js?v=1"
        );
    }
}
