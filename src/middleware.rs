use axum::http::HeaderValue;
use tower_http::set_header::SetResponseHeaderLayer;

/// Security header: X-Content-Type-Options: nosniff
pub fn x_content_type_options() -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::overriding(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    )
}

/// Security header: X-Frame-Options: DENY
pub fn x_frame_options() -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::overriding(
        axum::http::header::X_FRAME_OPTIONS,
        HeaderValue::from_static("DENY"),
    )
}

/// Security header: X-XSS-Protection: 1; mode=block
pub fn x_xss_protection() -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::overriding(
        axum::http::HeaderName::from_static("x-xss-protection"),
        HeaderValue::from_static("1; mode=block"),
    )
}

/// Security header: Strict-Transport-Security
pub fn strict_transport_security() -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::overriding(
        axum::http::header::STRICT_TRANSPORT_SECURITY,
        HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    )
}

/// Security header: Content-Security-Policy: default-src 'none'
pub fn content_security_policy() -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::overriding(
        axum::http::HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static("default-src 'none'"),
    )
}
