//! Google OAuth 2.0 with PKCE, plus OS keyring persistence.
//!
//! Client ids are never committed. Set `FOLIATE_GOOGLE_CLIENT_ID` at build or
//! run time. The redirect URI defaults to `foliate://oauth2redirect` and can
//! be overridden with `FOLIATE_OAUTH_REDIRECT`.

use base64::Engine as _;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use url::Url;

pub const AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
pub const DRIVE_APPDATA_SCOPE: &str = "https://www.googleapis.com/auth/drive.appdata";
pub const KEYRING_SERVICE: &str = "io.github.phancaoquy.wolidate";
pub const KEYRING_USER: &str = "google-drive";
pub const DEFAULT_REDIRECT_URI: &str = "foliate://oauth2redirect";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at_ms: i64,
    pub scope: Option<String>,
    pub token_type: Option<String>,
}

impl TokenSet {
    pub fn is_fresh(&self, now_ms: i64) -> bool {
        self.expires_at_ms > now_ms + 60_000
    }
}

#[derive(Debug, Clone)]
pub struct PendingAuth {
    pub state: String,
    pub verifier: String,
    pub redirect_uri: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
    refresh_token: Option<String>,
    scope: Option<String>,
    token_type: Option<String>,
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

pub fn client_id() -> Option<String> {
    std::env::var("FOLIATE_GOOGLE_CLIENT_ID")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| {
            option_env!("FOLIATE_GOOGLE_CLIENT_ID")
                .map(str::to_owned)
                .filter(|value| !value.is_empty())
        })
}

pub fn redirect_uri() -> String {
    std::env::var("FOLIATE_OAUTH_REDIRECT")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_REDIRECT_URI.to_owned())
}

fn random_urlsafe(bytes: usize) -> String {
    let mut buffer = vec![0_u8; bytes];
    rand::thread_rng().fill_bytes(&mut buffer);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buffer)
}

/// RFC 7636 S256 challenge.
pub fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

/// Start an authorization-code + PKCE flow.
pub fn begin() -> Result<(PendingAuth, String), String> {
    let client_id = client_id().ok_or_else(|| {
        "FOLIATE_GOOGLE_CLIENT_ID is not set; Google Drive sync is disabled".to_string()
    })?;
    let state = random_urlsafe(24);
    let verifier = random_urlsafe(48);
    let challenge = pkce_challenge(&verifier);
    let redirect_uri = redirect_uri();

    let mut url = Url::parse(AUTH_ENDPOINT).map_err(|error| error.to_string())?;
    url.query_pairs_mut()
        .append_pair("client_id", &client_id)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", DRIVE_APPDATA_SCOPE)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair("code_challenge_method", "S256")
        .append_pair("code_challenge", &challenge)
        .append_pair("state", &state);

    Ok((
        PendingAuth {
            state,
            verifier,
            redirect_uri,
        },
        url.to_string(),
    ))
}

pub async fn exchange_code(
    http: &reqwest::Client,
    code: &str,
    pending: &PendingAuth,
) -> Result<TokenSet, String> {
    let client_id = client_id().ok_or_else(|| "FOLIATE_GOOGLE_CLIENT_ID is not set".to_string())?;
    let response = http
        .post(TOKEN_ENDPOINT)
        .form(&[
            ("client_id", client_id.as_str()),
            ("code", code),
            ("code_verifier", pending.verifier.as_str()),
            ("grant_type", "authorization_code"),
            ("redirect_uri", pending.redirect_uri.as_str()),
        ])
        .send()
        .await
        .map_err(|error| error.to_string())?;
    parse_token_response(response, None).await
}

pub async fn refresh(http: &reqwest::Client, refresh_token: &str) -> Result<TokenSet, String> {
    let client_id = client_id().ok_or_else(|| "FOLIATE_GOOGLE_CLIENT_ID is not set".to_string())?;
    let response = http
        .post(TOKEN_ENDPOINT)
        .form(&[
            ("client_id", client_id.as_str()),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|error| error.to_string())?;
    parse_token_response(response, Some(refresh_token)).await
}

async fn parse_token_response(
    response: reqwest::Response,
    previous_refresh_token: Option<&str>,
) -> Result<TokenSet, String> {
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let body = body.chars().take(300).collect::<String>();
        return Err(format!("Google token endpoint returned {status}: {body}"));
    }
    let token: TokenResponse = response
        .json()
        .await
        .map_err(|error| format!("invalid Google token response: {error}"))?;
    Ok(TokenSet {
        access_token: token.access_token,
        refresh_token: token
            .refresh_token
            .or_else(|| previous_refresh_token.map(str::to_owned)),
        expires_at_ms: now_ms() + token.expires_in.saturating_mul(1000),
        scope: token.scope,
        token_type: token.token_type,
    })
}

#[cfg(target_os = "windows")]
fn init_keyring() -> Result<(), String> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    INIT.get_or_init(|| {
        let store = windows_native_keyring_store::Store::new()
            .map_err(|error| format!("Windows Credential Manager unavailable: {error}"))?;
        keyring_core::set_default_store(store);
        Ok(())
    })
    .clone()
}

#[cfg(target_os = "android")]
fn init_keyring() -> Result<(), String> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    INIT.get_or_init(|| {
        let store = android_native_keyring_store::Store::new()
            .map_err(|error| format!("Android Keystore unavailable: {error}"))?;
        keyring_core::set_default_store(store);
        Ok(())
    })
    .clone()
}

fn entry() -> Result<keyring_core::Entry, String> {
    init_keyring()?;
    keyring_core::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|error| error.to_string())
}

pub fn load_tokens() -> Result<Option<TokenSet>, String> {
    let entry = entry()?;
    match entry.get_password() {
        Ok(json) => serde_json::from_str(&json)
            .map(Some)
            .map_err(|error| format!("stored token is invalid: {error}")),
        Err(keyring_core::Error::NoEntry) => Ok(None),
        Err(error) => Err(format!("keyring read failed: {error}")),
    }
}

pub fn save_tokens(tokens: &TokenSet) -> Result<(), String> {
    let json = serde_json::to_string(tokens).map_err(|error| error.to_string())?;
    entry()?
        .set_password(&json)
        .map_err(|error| format!("keyring write failed: {error}"))
}

pub fn clear_tokens() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!("keyring delete failed: {error}")),
    }
}

/// Return a usable access token, refreshing when needed.
pub async fn fresh_access_token(http: &reqwest::Client) -> Result<String, String> {
    let tokens = load_tokens()?.ok_or_else(|| "Google Drive is not connected".to_string())?;
    if tokens.is_fresh(now_ms()) {
        return Ok(tokens.access_token);
    }
    let refresh_token = tokens
        .refresh_token
        .as_deref()
        .ok_or_else(|| "Google Drive session expired; reconnect".to_string())?;
    let refreshed = refresh(http, refresh_token).await?;
    let access_token = refreshed.access_token.clone();
    save_tokens(&refreshed)?;
    Ok(access_token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc7636_vector() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            pkce_challenge(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn fresh_token_window() {
        let tokens = TokenSet {
            access_token: "a".into(),
            refresh_token: None,
            expires_at_ms: 10_000,
            scope: None,
            token_type: None,
        };
        assert!(tokens.is_fresh(5_000));
        assert!(!tokens.is_fresh(9_500));
    }
}
