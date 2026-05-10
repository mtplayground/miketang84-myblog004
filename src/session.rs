use tower_sessions_cookie_store::{
    CookieSessionConfig, CookieSessionManagerLayer, Key, SignedCookie,
};

pub fn session_layer(session_secret: &str) -> CookieSessionManagerLayer<SignedCookie> {
    let key = Key::derive_from(session_secret.as_bytes());
    let config = CookieSessionConfig::default()
        .with_secure(true)
        .with_http_only(true);

    CookieSessionManagerLayer::signed(key).with_config(config)
}
