use axum::{
    body::Body,
    http::{
        HeaderValue, Request,
        header::{CACHE_CONTROL, PRAGMA},
    },
    middleware::Next,
    response::Response,
};

const NO_STORE: &str = "no-store";
const NO_CACHE: &str = "no-cache";
const NOSNIFF: &str = "nosniff";
const NO_REFERRER: &str = "no-referrer";
const X_CONTENT_TYPE_OPTIONS: &str = "x-content-type-options";
const REFERRER_POLICY: &str = "referrer-policy";

pub async fn auth_security_headers(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(CACHE_CONTROL, HeaderValue::from_static(NO_STORE));
    headers.insert(PRAGMA, HeaderValue::from_static(NO_CACHE));
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static(NOSNIFF));
    headers.insert(REFERRER_POLICY, HeaderValue::from_static(NO_REFERRER));

    response
}
