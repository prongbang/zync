//! Server-side credentials store + API — DESIGN.md ADR-001. Owns the
//! `credentials` table CRUD routes, at-rest encryption (via `crate::crypto`),
//! the host-pattern selection function, and `resolve_credential_spec[_for_url]`
//! — the seam the remote-op handlers (`crate::git`, `crate::repository`) call
//! to turn a remote URL into a `zync_git_core::CredentialSpec`.

use crate::{
    crypto::{self, CryptoError},
    db::{CredentialSecretRow, CredentialSummary, Database},
    AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use zeroize::{Zeroize, Zeroizing};

/// Single-user server today — every route acts as this seeded user.
/// TODO(P3): resolve the authenticated user from the request instead once
/// real auth lands (see `crate::auth`).
pub(crate) const DEFAULT_USER_ID: &str = "owner";

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/credentials", get(list_credentials).post(create_credential))
        .route("/credentials/:id", delete(delete_credential))
}

/// Deserialize-only; holds plaintext secret material until it's encrypted in
/// `create_credential`, so — per ADR-001 "no derived `Debug` that prints
/// secrets" — this gets a hand-written `Debug` that redacts every secret
/// field (mirrors `CredentialSecretRow`'s manual impl).
#[derive(Deserialize)]
struct CreateCredentialRequest {
    label: String,
    host_pattern: String,
    kind: String,
    username: Option<String>,
    // https_token bundle
    token: Option<String>,
    // ssh_key bundle
    private_key: Option<String>,
    passphrase: Option<String>,
    public_key: Option<String>,
}

impl std::fmt::Debug for CreateCredentialRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateCredentialRequest")
            .field("label", &self.label)
            .field("host_pattern", &self.host_pattern)
            .field("kind", &self.kind)
            .field("username", &self.username)
            .field("token", &self.token.as_ref().map(|_| "<redacted>"))
            .field("private_key", &self.private_key.as_ref().map(|_| "<redacted>"))
            .field("passphrase", &self.passphrase.as_ref().map(|_| "<redacted>"))
            .field("public_key", &self.public_key.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

#[derive(Debug, Serialize)]
struct CredentialResponse {
    id: String,
    label: String,
    host_pattern: String,
    kind: String,
    username: Option<String>,
    created_at: String,
}

impl From<CredentialSummary> for CredentialResponse {
    fn from(record: CredentialSummary) -> Self {
        CredentialResponse {
            id: record.id,
            label: record.label,
            host_pattern: record.host_pattern,
            kind: record.kind,
            username: record.username,
            created_at: record.created_at,
        }
    }
}

async fn list_credentials(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<CredentialResponse>>, (StatusCode, String)> {
    let records = state
        .db
        .list_credentials_by_user(DEFAULT_USER_ID)
        .map_err(internal_error)?;
    Ok(Json(records.into_iter().map(Into::into).collect()))
}

async fn create_credential(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateCredentialRequest>,
) -> Result<(StatusCode, Json<CredentialResponse>), (StatusCode, String)> {
    if request.label.trim().is_empty() {
        return Err(bad_request("label must not be empty"));
    }
    if request.host_pattern.trim().is_empty() {
        return Err(bad_request("host_pattern must not be empty"));
    }
    validate_host_pattern(&request.host_pattern)?;

    let bundle = match request.kind.as_str() {
        "https_token" => {
            let token = request
                .token
                .as_deref()
                .filter(|t| !t.trim().is_empty())
                .ok_or_else(|| bad_request("token is required for kind = 'https_token'"))?;
            serde_json::json!({ "token": token })
        }
        "ssh_key" => {
            let private_key = request
                .private_key
                .as_deref()
                .filter(|k| !k.trim().is_empty())
                .ok_or_else(|| bad_request("private_key is required for kind = 'ssh_key'"))?;
            serde_json::json!({
                "private_key": private_key,
                "passphrase": request.passphrase,
                "public_key": request.public_key,
            })
        }
        other => {
            return Err(bad_request(&format!(
                "kind must be 'https_token' or 'ssh_key', got '{other}'"
            )))
        }
    };

    let key = state.secrets.key().map_err(crypto_error)?;
    // Zeroized so the serialized plaintext bundle (token / private key /
    // passphrase) is wiped as soon as it goes out of scope, not just left
    // for the allocator to reuse.
    let plaintext: Zeroizing<Vec<u8>> =
        Zeroizing::new(serde_json::to_vec(&bundle).map_err(|e| internal_error(e.into()))?);
    let (cipher, nonce) = crypto::encrypt(key, &plaintext).map_err(crypto_error)?;

    let record = state
        .db
        .insert_credential(
            DEFAULT_USER_ID,
            &request.label,
            &request.host_pattern,
            &request.kind,
            request.username.as_deref(),
            &cipher,
            &nonce,
        )
        .map_err(internal_error)?;

    Ok((StatusCode::CREATED, Json(record.into())))
}

async fn delete_credential(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .db
        .delete_credential(&id, DEFAULT_USER_ID)
        .map_err(internal_error)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Enforce the ADR-001 wildcard contract at write time: a `host_pattern` is
/// either an exact host, or a single leading `"*."` glob with a non-empty
/// suffix (`"*.github.com"`). Anything else — `"*github.com"` (missing dot,
/// a common typo), multiple `*`s, a bare `"*"` — is rejected so a malformed
/// pattern can never silently fail to match at selection time.
fn validate_host_pattern(pattern: &str) -> Result<(), (StatusCode, String)> {
    if pattern.contains('*') {
        if !pattern.starts_with("*.") || pattern.matches('*').count() > 1 {
            return Err(bad_request(
                "host_pattern wildcards must be a single leading \"*.\" (e.g. \"*.github.com\")",
            ));
        }
        if pattern.len() == 2 {
            return Err(bad_request(
                "host_pattern wildcard must have a suffix after \"*.\" (e.g. \"*.github.com\")",
            ));
        }
    }
    Ok(())
}

fn bad_request(message: &str) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, message.to_string())
}

fn internal_error(error: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

/// `CryptoError::NotConfigured` maps to a clear, non-secret 503 per
/// ADR-001; encrypt/decrypt failures are treated as internal errors (they
/// only happen on tampered/corrupt data or a wrong key).
fn crypto_error(error: CryptoError) -> (StatusCode, String) {
    match error {
        CryptoError::NotConfigured => (StatusCode::SERVICE_UNAVAILABLE, error.to_string()),
        CryptoError::Encrypt | CryptoError::Decrypt => {
            (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
        }
    }
}

// ---- Host-pattern parsing & credential selection (ADR-001 Decision 3) ----
//
// `select_credential`, `parse_remote_host`, and `decrypt_secret_bundle` are
// the building blocks `resolve_credential_spec[_for_url]` (below) composes
// into the seam the remote-op handlers call.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemeClass {
    Https,
    Ssh,
}

impl SchemeClass {
    fn compatible_kind(self) -> &'static str {
        match self {
            SchemeClass::Https => "https_token",
            SchemeClass::Ssh => "ssh_key",
        }
    }
}

/// Parse the host and scheme-class out of a git remote URL:
/// `https://host[:port]/...`, `ssh://[user@]host[:port]/...`, and the
/// scp-like `[user@]host:path` form. Returns `None` for anything else
/// (e.g. a local filesystem path, which never needs credentials).
pub fn parse_remote_host(remote_url: &str) -> Option<(String, SchemeClass)> {
    let url = remote_url.trim();
    if let Some(rest) = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")) {
        return host_from_authority(rest).map(|host| (host, SchemeClass::Https));
    }
    if let Some(rest) = url.strip_prefix("ssh://") {
        return host_from_authority(rest).map(|host| (host, SchemeClass::Ssh));
    }
    if !url.contains("://") {
        if let Some(colon) = url.find(':') {
            let before = &url[..colon];
            let after = &url[colon + 1..];
            // Guard against Windows-style absolute paths (`C:\...`) and bare
            // paths with a colon in them: an scp-like host has no `/` or `\`
            // before the colon and something after it.
            if !before.is_empty()
                && !before.contains('/')
                && !before.contains('\\')
                && !after.is_empty()
            {
                let host = before.rsplit('@').next().unwrap_or(before);
                if !host.is_empty() {
                    return Some((host.to_lowercase(), SchemeClass::Ssh));
                }
            }
        }
    }
    None
}

fn host_from_authority(rest: &str) -> Option<String> {
    let rest = rest.rsplit('@').next()?; // strip userinfo, if any
    let authority = rest.split('/').next()?; // strip path
    let host = authority.split(':').next()?; // strip port
    if host.is_empty() {
        None
    } else {
        Some(host.to_lowercase())
    }
}

/// Score a `host_pattern` against a resolved `host`. `None` means no match;
/// `Some(specificity)` ranks matches so the caller can prefer more specific
/// patterns (exact match scores highest via `usize::MAX`).
///
/// Only a pattern starting with the literal `"*."` is treated as a
/// wildcard — anything else (including a bare `"*"` or a typo'd
/// `"*github.com"`) is compared for exact equality instead, so it simply
/// never matches unless the host is literally that string. `validate_host_pattern`
/// rejects those forms at write time; this is the read-time half of the
/// same contract, kept strict independently of what's already in the DB.
fn host_match_score(pattern: &str, host: &str) -> Option<usize> {
    let pattern = pattern.trim().to_lowercase();
    let host = host.trim().to_lowercase();
    if let Some(suffix) = pattern.strip_prefix("*.") {
        // "*.github.com" (suffix = "github.com") matches "api.github.com"
        // but not the apex "github.com": matching against ".github.com"
        // (with the dot re-added) guarantees the label boundary, so a host
        // like "evilgithub.com" can never falsely match.
        if !suffix.is_empty() && host.ends_with(&format!(".{suffix}")) {
            Some(suffix.len())
        } else {
            None
        }
    } else if pattern == host {
        Some(usize::MAX)
    } else {
        None
    }
}

/// Pick the best-matching credential for `remote_url` from `user_id`'s
/// stored credentials, per ADR-001 Decision 3's selection order:
/// exact host > most-specific wildcard > newest `created_at`.
///
/// Step 1 of the ADR order ("explicit per-remote assignment") has no
/// storage yet — remotes don't carry a `credential_id` — so it's a no-op
/// here; wire it in once the remotes UI (PLAN.md P0.4 dep) adds that column.
pub fn select_credential(
    db: &Database,
    user_id: &str,
    remote_url: &str,
) -> anyhow::Result<Option<CredentialSummary>> {
    let candidates = db.list_credentials_by_user(user_id)?;
    Ok(select_from_candidates(&candidates, remote_url))
}

fn select_from_candidates(
    candidates: &[CredentialSummary],
    remote_url: &str,
) -> Option<CredentialSummary> {
    let (host, scheme) = parse_remote_host(remote_url)?;
    let compatible_kind = scheme.compatible_kind();

    let mut scored: Vec<(usize, &CredentialSummary)> = candidates
        .iter()
        .filter(|candidate| candidate.kind == compatible_kind)
        .filter_map(|candidate| {
            host_match_score(&candidate.host_pattern, &host).map(|score| (score, candidate))
        })
        .collect();

    scored.sort_by(|(score_a, cred_a), (score_b, cred_b)| {
        score_b
            .cmp(score_a)
            .then_with(|| cred_b.created_at.cmp(&cred_a.created_at))
    });

    scored.into_iter().next().map(|(_, credential)| credential.clone())
}

/// A decrypted credential secret bundle, interpreted per `kind` by
/// `build_credential_spec`. Deliberately *not* a `serde_json::Value` — per
/// ADR-001 "Secrets never enter errors" (and the P0.11 security review, W1),
/// a `Value` parsed from the decrypted plaintext is itself an un-wiped heap
/// allocation holding the token/private key/passphrase. Every field here is
/// zeroized on drop instead.
#[derive(Deserialize, Default)]
pub struct SecretBundle {
    token: Option<String>,
    private_key: Option<String>,
    passphrase: Option<String>,
    public_key: Option<String>,
}

/// Hand-written so a stray `{:?}` (log line, panic message) can never print a
/// secret — mirrors `CreateCredentialRequest`'s manual `Debug` above.
impl std::fmt::Debug for SecretBundle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretBundle")
            .field("token", &self.token.as_ref().map(|_| "<redacted>"))
            .field("private_key", &self.private_key.as_ref().map(|_| "<redacted>"))
            .field("passphrase", &self.passphrase.as_ref().map(|_| "<redacted>"))
            .field("public_key", &self.public_key.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl SecretBundle {
    /// Wipe every field's contents in place. Called from `Drop`; also exercised directly by
    /// `secret_bundle_zeroizes_fields_on_drop` below (calling `Drop::drop` explicitly isn't
    /// something the compiler allows).
    fn zeroize_fields(&mut self) {
        self.token.zeroize();
        self.private_key.zeroize();
        self.passphrase.zeroize();
        self.public_key.zeroize();
    }
}

impl Drop for SecretBundle {
    fn drop(&mut self) {
        self.zeroize_fields();
    }
}

/// Decrypt a stored credential's secret bundle into a [`SecretBundle`]. Used
/// by `resolve_credential_spec[_for_url]` below to build a `CredentialSpec`
/// (git-core). Decrypts just-in-time; both the raw decrypted bytes
/// (`crypto::decrypt`'s `Zeroizing<Vec<u8>>`) and the parsed bundle are
/// zeroized on drop (see ADR-001 "Just-in-time decrypt").
pub fn decrypt_secret_bundle(
    key: &crypto::SecretKey,
    row: &CredentialSecretRow,
) -> Result<SecretBundle, CryptoError> {
    let plaintext = crypto::decrypt(key, &row.secret_cipher, &row.secret_nonce)?;
    serde_json::from_slice(&plaintext).map_err(|_| CryptoError::Decrypt)
}

// ---- Remote-op seam (ADR-001 Decision 4) ----
//
// `resolve_credential_spec`/`resolve_credential_spec_for_url` are what
// `crate::git` and `crate::repository`'s remote-op handlers call: given a
// remote (by name, looked up in the repo) or a bare URL (clone-on-register,
// which has no repo yet to look a remote up in), pick the best-matching
// stored credential for `user_id`, decrypt it just-in-time, and build the
// `zync_git_core::CredentialSpec` the network call needs. No match (or no
// such remote) falls back to `CredentialSpec::Default`, preserving today's
// ambient-credential behavior exactly. The decrypted plaintext bundle never
// leaves this function — it's consumed into the spec's `Zeroizing` fields
// and dropped.

/// Resolve the credential spec for `remote_name` in the repo at `repo_path`.
pub fn resolve_credential_spec(
    state: &AppState,
    user_id: &str,
    repo_path: impl AsRef<std::path::Path>,
    remote_name: &str,
) -> Result<zync_git_core::CredentialSpec, (StatusCode, String)> {
    let remote_url = zync_git_core::remotes(repo_path)
        .map_err(internal_error)?
        .into_iter()
        .find(|remote| remote.name == remote_name)
        .and_then(|remote| remote.url);

    match remote_url {
        Some(remote_url) => resolve_credential_spec_for_url(state, user_id, &remote_url),
        None => Ok(zync_git_core::CredentialSpec::Default),
    }
}

/// Resolve the credential spec for a bare remote URL — used by the
/// clone-on-register path, which has a URL but no repo (yet) to inspect.
pub fn resolve_credential_spec_for_url(
    state: &AppState,
    user_id: &str,
    remote_url: &str,
) -> Result<zync_git_core::CredentialSpec, (StatusCode, String)> {
    let Some(summary) =
        select_credential(&state.db, user_id, remote_url).map_err(internal_error)?
    else {
        return Ok(zync_git_core::CredentialSpec::Default);
    };

    let key = state.secrets.key().map_err(crypto_error)?;
    let row = state
        .db
        .get_decryptable(&summary.id, user_id)
        .map_err(internal_error)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "credential not found".to_string()))?;
    let bundle = decrypt_secret_bundle(key, &row).map_err(crypto_error)?;

    build_credential_spec(&row, bundle)
}

/// Interpret a decrypted secret bundle (per `kind`) into a `CredentialSpec`.
/// `username` defaults to `"git"` when the row didn't record one, matching
/// `CredentialSpec::Default`'s own fallback for an unauthenticated ssh URL.
fn build_credential_spec(
    row: &CredentialSecretRow,
    mut bundle: SecretBundle,
) -> Result<zync_git_core::CredentialSpec, (StatusCode, String)> {
    let username = || row.username.clone().unwrap_or_else(|| "git".to_string());
    match row.kind.as_str() {
        "https_token" => {
            let token = bundle.token.take().ok_or_else(|| {
                internal_error(anyhow::anyhow!(
                    "credential '{}' bundle is missing 'token'",
                    row.id
                ))
            })?;
            Ok(zync_git_core::CredentialSpec::UserpassPlaintext {
                username: username(),
                secret: Zeroizing::new(token),
            })
        }
        "ssh_key" => {
            let private_key = bundle.private_key.take().ok_or_else(|| {
                internal_error(anyhow::anyhow!(
                    "credential '{}' bundle is missing 'private_key'",
                    row.id
                ))
            })?;
            let passphrase = bundle.passphrase.take().map(Zeroizing::new);
            Ok(zync_git_core::CredentialSpec::SshKey {
                username: username(),
                private_key: Zeroizing::new(private_key),
                passphrase,
            })
        }
        other => Err(internal_error(anyhow::anyhow!(
            "credential '{}' has unsupported kind '{other}'",
            row.id
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn credential(
        id: &str,
        host_pattern: &str,
        kind: &str,
        created_at: &str,
    ) -> CredentialSummary {
        CredentialSummary {
            id: id.to_string(),
            user_id: "owner".to_string(),
            label: id.to_string(),
            host_pattern: host_pattern.to_string(),
            kind: kind.to_string(),
            username: None,
            created_at: created_at.to_string(),
        }
    }

    #[test]
    fn parses_https_host_and_port() {
        let (host, scheme) = parse_remote_host("https://github.com/org/repo.git").unwrap();
        assert_eq!(host, "github.com");
        assert_eq!(scheme, SchemeClass::Https);

        let (host, _) = parse_remote_host("https://example.com:8443/org/repo.git").unwrap();
        assert_eq!(host, "example.com");
    }

    #[test]
    fn parses_https_with_userinfo() {
        let (host, scheme) =
            parse_remote_host("https://x-access-token@github.com/org/repo.git").unwrap();
        assert_eq!(host, "github.com");
        assert_eq!(scheme, SchemeClass::Https);
    }

    #[test]
    fn parses_ssh_scheme_url() {
        let (host, scheme) = parse_remote_host("ssh://git@github.com:22/org/repo.git").unwrap();
        assert_eq!(host, "github.com");
        assert_eq!(scheme, SchemeClass::Ssh);
    }

    #[test]
    fn parses_scp_like_ssh_url() {
        let (host, scheme) = parse_remote_host("git@github.com:org/repo.git").unwrap();
        assert_eq!(host, "github.com");
        assert_eq!(scheme, SchemeClass::Ssh);
    }

    #[test]
    fn rejects_local_paths() {
        assert!(parse_remote_host("/home/user/repo").is_none());
        assert!(parse_remote_host("../relative/repo").is_none());
    }

    #[test]
    fn wildcard_matches_but_excludes_apex() {
        assert!(host_match_score("*.github.com", "api.github.com").is_some());
        assert!(host_match_score("*.github.com", "github.com").is_none());
    }

    #[test]
    fn wildcard_typo_rejected_at_create() {
        // Missing the dot is the classic typo the ADR calls out — reject it
        // rather than silently storing a pattern that can never match.
        assert!(validate_host_pattern("*github.com").is_err());
        assert!(validate_host_pattern("*").is_err());
        assert!(validate_host_pattern("*.").is_err());
        assert!(validate_host_pattern("*.*.com").is_err());
        // Well-formed patterns are accepted.
        assert!(validate_host_pattern("github.com").is_ok());
        assert!(validate_host_pattern("*.github.com").is_ok());
    }

    #[test]
    fn wildcard_typo_not_treated_as_wildcard_in_matching() {
        // A pattern that doesn't start with "*." is never expanded as a
        // wildcard — it's only ever compared for exact string equality, so
        // "*github.com" (the typo) can never match a real host.
        assert!(host_match_score("*github.com", "github.com").is_none());
        assert!(host_match_score("*github.com", "api.github.com").is_none());
    }

    #[test]
    fn exact_beats_wildcard() {
        let creds = vec![
            credential("wildcard", "*.github.com", "https_token", "2026-01-01T00:00:00Z"),
            credential("exact", "api.github.com", "https_token", "2026-01-01T00:00:00Z"),
        ];
        let picked = select_from_candidates(&creds, "https://api.github.com/org/repo.git").unwrap();
        assert_eq!(picked.id, "exact");
    }

    #[test]
    fn more_specific_wildcard_wins() {
        let creds = vec![
            credential("broad", "*.com", "https_token", "2026-01-01T00:00:00Z"),
            credential("narrow", "*.github.com", "https_token", "2026-01-01T00:00:00Z"),
        ];
        let picked = select_from_candidates(&creds, "https://api.github.com/org/repo.git").unwrap();
        assert_eq!(picked.id, "narrow");
    }

    #[test]
    fn ties_break_by_newest_created_at() {
        let creds = vec![
            credential("older", "github.com", "https_token", "2026-01-01T00:00:00Z"),
            credential("newer", "github.com", "https_token", "2026-06-01T00:00:00Z"),
        ];
        let picked = select_from_candidates(&creds, "https://github.com/org/repo.git").unwrap();
        assert_eq!(picked.id, "newer");
    }

    #[test]
    fn kind_scheme_mismatch_is_never_selected() {
        let creds = vec![credential(
            "ssh-cred",
            "github.com",
            "ssh_key",
            &Utc::now().to_rfc3339(),
        )];
        // An https:// remote must never pick an ssh_key credential.
        assert!(select_from_candidates(&creds, "https://github.com/org/repo.git").is_none());
    }

    #[test]
    fn https_vs_ssh_credential_for_same_host() {
        let creds = vec![
            credential("token", "github.com", "https_token", "2026-01-01T00:00:00Z"),
            credential("key", "github.com", "ssh_key", "2026-01-01T00:00:00Z"),
        ];
        let https_pick = select_from_candidates(&creds, "https://github.com/org/repo.git").unwrap();
        assert_eq!(https_pick.id, "token");
        let ssh_pick = select_from_candidates(&creds, "git@github.com:org/repo.git").unwrap();
        assert_eq!(ssh_pick.id, "key");
    }

    #[test]
    fn no_match_returns_none() {
        let creds = vec![credential(
            "gitlab",
            "gitlab.com",
            "https_token",
            "2026-01-01T00:00:00Z",
        )];
        assert!(select_from_candidates(&creds, "https://github.com/org/repo.git").is_none());
    }

    #[test]
    fn decrypt_secret_bundle_round_trips_https_token() {
        let key = crypto::test_key(3);
        let plaintext = serde_json::json!({ "token": "ghp_example" });
        let (cipher, nonce) =
            crypto::encrypt(&key, &serde_json::to_vec(&plaintext).unwrap()).unwrap();
        let row = CredentialSecretRow {
            id: "id".into(),
            user_id: "owner".into(),
            label: "GitHub PAT".into(),
            host_pattern: "github.com".into(),
            kind: "https_token".into(),
            username: Some("x-access-token".into()),
            secret_cipher: cipher,
            secret_nonce: nonce,
            created_at: Utc::now().to_rfc3339(),
        };
        let decrypted = decrypt_secret_bundle(&key, &row).unwrap();
        assert_eq!(decrypted.token.as_deref(), Some("ghp_example"));
    }

    fn secret_row(kind: &str, username: Option<&str>) -> CredentialSecretRow {
        CredentialSecretRow {
            id: "row-id".into(),
            user_id: "owner".into(),
            label: "label".into(),
            host_pattern: "github.com".into(),
            kind: kind.into(),
            username: username.map(ToOwned::to_owned),
            secret_cipher: Vec::new(),
            secret_nonce: Vec::new(),
            created_at: Utc::now().to_rfc3339(),
        }
    }

    // `SecretBundle` implements `Drop`, so `SecretBundle { field, ..Default::default() }` struct
    // update syntax doesn't compile (moving the remaining fields out of the `Default::default()`
    // temporary is a partial move out of a `Drop` type) — these helpers build fully-specified
    // literals instead.
    fn token_bundle(token: &str) -> SecretBundle {
        SecretBundle {
            token: Some(token.to_string()),
            private_key: None,
            passphrase: None,
            public_key: None,
        }
    }

    fn ssh_bundle(private_key: &str, passphrase: Option<&str>) -> SecretBundle {
        SecretBundle {
            token: None,
            private_key: Some(private_key.to_string()),
            passphrase: passphrase.map(ToOwned::to_owned),
            public_key: None,
        }
    }

    #[test]
    fn build_credential_spec_https_token_uses_stored_username() {
        let row = secret_row("https_token", Some("x-access-token"));
        let bundle = token_bundle("ghp_example");
        let spec = build_credential_spec(&row, bundle).unwrap();
        match spec {
            zync_git_core::CredentialSpec::UserpassPlaintext { username, secret } => {
                assert_eq!(username, "x-access-token");
                assert_eq!(secret.as_str(), "ghp_example");
            }
            other => panic!("expected UserpassPlaintext, got {other:?}"),
        }
    }

    #[test]
    fn build_credential_spec_https_token_defaults_username_to_git() {
        let row = secret_row("https_token", None);
        let bundle = token_bundle("ghp_example");
        let spec = build_credential_spec(&row, bundle).unwrap();
        match spec {
            zync_git_core::CredentialSpec::UserpassPlaintext { username, .. } => {
                assert_eq!(username, "git");
            }
            other => panic!("expected UserpassPlaintext, got {other:?}"),
        }
    }

    #[test]
    fn build_credential_spec_https_token_missing_token_errors() {
        let row = secret_row("https_token", None);
        let bundle = SecretBundle::default();
        assert!(build_credential_spec(&row, bundle).is_err());
    }

    #[test]
    fn build_credential_spec_ssh_key_carries_passphrase() {
        let row = secret_row("ssh_key", Some("git"));
        let bundle = ssh_bundle("-----BEGIN KEY-----", Some("hunter2"));
        let spec = build_credential_spec(&row, bundle).unwrap();
        match spec {
            zync_git_core::CredentialSpec::SshKey {
                username,
                private_key,
                passphrase,
            } => {
                assert_eq!(username, "git");
                assert_eq!(private_key.as_str(), "-----BEGIN KEY-----");
                assert_eq!(passphrase.unwrap().as_str(), "hunter2");
            }
            other => panic!("expected SshKey, got {other:?}"),
        }
    }

    #[test]
    fn build_credential_spec_ssh_key_without_passphrase() {
        let row = secret_row("ssh_key", None);
        let bundle = ssh_bundle("-----BEGIN KEY-----", None);
        let spec = build_credential_spec(&row, bundle).unwrap();
        match spec {
            zync_git_core::CredentialSpec::SshKey { passphrase, .. } => {
                assert!(passphrase.is_none());
            }
            other => panic!("expected SshKey, got {other:?}"),
        }
    }

    #[test]
    fn build_credential_spec_unknown_kind_errors() {
        let row = secret_row("carrier-pigeon", None);
        assert!(build_credential_spec(&row, SecretBundle::default()).is_err());
    }

    #[test]
    fn secret_bundle_zeroizes_fields_on_drop() {
        // Regression test for W1 (P0.11 security review): the decrypted bundle used to be a
        // `serde_json::Value`, an un-wiped heap allocation holding the plaintext secret after
        // use. `SecretBundle` must actually wipe its fields' contents when dropped, not just
        // move the secret elsewhere. `Drop::drop` can't be called explicitly, so this exercises
        // the same `zeroize_fields` helper `Drop` delegates to. `Option<Z>::zeroize()` wipes the
        // inner value's bytes and then takes it, leaving `None` — that's the strongest possible
        // observable signal here (there's no safe way to peek at bytes that used to back a
        // freed `String`'s allocation).
        let mut bundle = SecretBundle {
            token: Some("ghp_example".to_string()),
            private_key: Some("-----BEGIN KEY-----".to_string()),
            passphrase: Some("hunter2".to_string()),
            public_key: Some("ssh-ed25519 AAAA".to_string()),
        };
        bundle.zeroize_fields();
        assert_eq!(bundle.token, None);
        assert_eq!(bundle.private_key, None);
        assert_eq!(bundle.passphrase, None);
        assert_eq!(bundle.public_key, None);
    }
}
