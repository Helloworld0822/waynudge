use crate::{
    error::{Error, Result, invalid},
    routines::text,
};
use axum::{
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use subtle::ConstantTimeEq;

#[derive(Clone, Debug)]
pub struct User(pub String);
#[derive(Clone)]
pub struct Auth {
    tokens: Arc<Vec<(String, [u8; 32])>>,
    pub public_host: Option<String>,
    pub public_mcp: bool,
}
impl Auth {
    pub fn new(users: HashMap<String, String>, public_host: Option<String>) -> Result<Self> {
        if users.is_empty() {
            return Err(invalid("WAYNUDGE_USERS must contain at least one user"));
        }
        let mut seen = HashSet::new();
        let mut tokens = vec![];
        for (user, token) in users {
            text(&user)?;
            if user == crate::error::PUBLIC_USER {
                return Err(invalid("reserved user ID"));
            }
            if token.len() < 32
                || token.len() > 256
                || !token.bytes().all(|b| b.is_ascii_graphic())
                || !seen.insert(token.clone())
            {
                return Err(invalid("tokens must be unique 32..256 ASCII characters"));
            }
            tokens.push((user, Sha256::digest(token.as_bytes()).into()));
        }
        if public_host.as_ref().is_some_and(|h| {
            h.is_empty()
                || !h
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        }) {
            return Err(invalid("PUBLIC_HOST must be a hostname"));
        }
        Ok(Self {
            tokens: Arc::new(tokens),
            public_host,
            public_mcp: false,
        })
    }
    pub fn with_public_mcp(mut self, enabled: bool) -> Self {
        self.public_mcp = enabled;
        self
    }
}
pub async fn authenticate(State(auth): State<Auth>, mut request: Request, next: Next) -> Response {
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    let mut user = None;
    match token {
        Some(token) => {
            let digest: [u8; 32] = Sha256::digest(token.as_bytes()).into();
            for (id, expected) in auth.tokens.iter() {
                if bool::from(digest.ct_eq(expected)) {
                    user = Some(User(id.clone()));
                }
            }
        }
        None if auth.public_mcp
            && !request.headers().contains_key(header::AUTHORIZATION)
            && matches!(request.uri().path(), "/mcp" | "/mcp/") =>
        {
            user = Some(User(crate::error::PUBLIC_USER.into()))
        }
        None => {}
    }
    let Some(user) = user else {
        return unauthorized();
    };
    if let Some(origin) = request.headers().get(header::ORIGIN) {
        let valid = origin.to_str().is_ok_and(|origin| {
            origin == "http://localhost:8080"
                || origin == "http://127.0.0.1:8080"
                || auth
                    .public_host
                    .as_ref()
                    .is_some_and(|h| origin == format!("https://{h}"))
        });
        if !valid {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    request.extensions_mut().insert(user);
    next.run(request).await
}
fn unauthorized() -> Response {
    let mut response = Error::Unauthorized.into_response();
    response.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        header::HeaderValue::from_static("Bearer"),
    );
    response
}
