//! Session tokens + sliding-expiry logic — ADR-002 Decision 2.
//!
//! The cookie carries a 256-bit random opaque token; the DB stores only its
//! SHA-256 (a leaked DB yields hashes, not live bearer tokens). Expiry is a
//! sliding idle window (7d) refreshed at most once per refresh window (1d),
//! under a hard 30d absolute cap from creation.

use base64::Engine;
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};

/// Idle TTL — a session unused for this long is dead.
pub const IDLE_TTL_DAYS: i64 = 7;
/// Refresh window — the sliding window is only bumped once this has elapsed
/// since `last_used`, so an active session costs at most one DB write per day.
pub const REFRESH_WINDOW_DAYS: i64 = 1;
/// Absolute lifetime cap from `created_at`, regardless of activity.
pub const ABSOLUTE_TTL_DAYS: i64 = 30;

pub const COOKIE_NAME: &str = "zync_session";

/// Mint a fresh 256-bit session token, base64url (no padding) encoded. This is
/// the raw value that goes in the cookie; only its [`hash_token`] is stored.
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// SHA-256 (hex) of a raw token — the primary key stored in `sessions.id`. The
/// token is high-entropy, so a plain hash (no salt/KDF) is sufficient: there is
/// nothing to brute-force.
pub fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Fixed RFC3339 rendering used for every session timestamp (seconds precision,
/// `Z`), so the fixed width makes the sweep's lexical SQL comparison monotonic.
pub fn format_ts(ts: DateTime<Utc>) -> String {
    ts.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Timestamps for a brand-new session anchored at `now`.
pub struct NewSession {
    pub created_at: String,
    pub last_used: String,
    pub expires_at: String,
}

pub fn new_session_timestamps(now: DateTime<Utc>) -> NewSession {
    let expires = now + Duration::days(IDLE_TTL_DAYS);
    NewSession {
        created_at: format_ts(now),
        last_used: format_ts(now),
        expires_at: format_ts(expires),
    }
}

/// The outcome of validating a stored session against the current time.
#[derive(Debug, PartialEq, Eq)]
pub enum SessionCheck {
    /// Session is dead (idle-expired, past the absolute cap, or unparseable):
    /// the middleware deletes the row and returns 401.
    Invalid,
    /// Session is valid and still inside its refresh window — no write needed.
    Valid,
    /// Session is valid but the refresh window elapsed — bump `last_used` and
    /// `expires_at` to these values and re-set the cookie.
    Refresh { last_used: String, expires_at: String },
}

/// Pure sliding-expiry evaluation (ADR-002 Decision 2). Takes the stored
/// timestamps as RFC3339 strings; any parse failure is treated as `Invalid`
/// (fail closed).
pub fn evaluate(
    created_at: &str,
    last_used: &str,
    expires_at: &str,
    now: DateTime<Utc>,
) -> SessionCheck {
    let (Ok(created), Ok(last), Ok(expires)) = (
        parse(created_at),
        parse(last_used),
        parse(expires_at),
    ) else {
        return SessionCheck::Invalid;
    };

    // Idle TTL: past expiry → dead.
    if now >= expires {
        return SessionCheck::Invalid;
    }
    // Absolute cap: past 30d from creation → force re-login regardless of
    // activity.
    if now >= created + Duration::days(ABSOLUTE_TTL_DAYS) {
        return SessionCheck::Invalid;
    }
    // Sliding refresh, throttled to once per refresh window.
    if now - last > Duration::days(REFRESH_WINDOW_DAYS) {
        let new_expires = now + Duration::days(IDLE_TTL_DAYS);
        return SessionCheck::Refresh {
            last_used: format_ts(now),
            expires_at: format_ts(new_expires),
        };
    }
    SessionCheck::Valid
}

fn parse(ts: &str) -> Result<DateTime<Utc>, chrono::ParseError> {
    DateTime::parse_from_rfc3339(ts).map(|dt| dt.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_high_entropy_and_unique() {
        let a = generate_token();
        let b = generate_token();
        assert_ne!(a, b);
        // 32 bytes base64url-no-pad → 43 chars.
        assert_eq!(a.len(), 43);
    }

    #[test]
    fn hash_token_is_deterministic_and_hex() {
        let h = hash_token("some-token");
        assert_eq!(h, hash_token("some-token"));
        assert_ne!(h, hash_token("other-token"));
        assert_eq!(h.len(), 64); // 32 bytes hex
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn fresh_session_is_valid_and_not_yet_refreshed() {
        let now = Utc::now();
        let s = new_session_timestamps(now);
        assert_eq!(
            evaluate(&s.created_at, &s.last_used, &s.expires_at, now),
            SessionCheck::Valid
        );
    }

    #[test]
    fn idle_expired_session_is_invalid() {
        let created = Utc::now() - Duration::days(10);
        let last = Utc::now() - Duration::days(8);
        let expires = last + Duration::days(IDLE_TTL_DAYS); // 1d ago
        let check = evaluate(
            &format_ts(created),
            &format_ts(last),
            &format_ts(expires),
            Utc::now(),
        );
        assert_eq!(check, SessionCheck::Invalid);
    }

    #[test]
    fn stale_but_valid_session_refreshes() {
        let now = Utc::now();
        let created = now - Duration::days(3);
        let last = now - Duration::days(2); // > 1d refresh window
        let expires = now + Duration::days(5); // still valid
        match evaluate(
            &format_ts(created),
            &format_ts(last),
            &format_ts(expires),
            now,
        ) {
            SessionCheck::Refresh {
                last_used,
                expires_at,
            } => {
                assert_eq!(last_used, format_ts(now));
                assert_eq!(expires_at, format_ts(now + Duration::days(IDLE_TTL_DAYS)));
            }
            other => panic!("expected Refresh, got {other:?}"),
        }
    }

    #[test]
    fn absolute_cap_invalidates_even_when_active() {
        let now = Utc::now();
        let created = now - Duration::days(31); // past 30d cap
        let last = now - Duration::minutes(1); // very recently used
        let expires = now + Duration::days(5); // idle window still open
        let check = evaluate(
            &format_ts(created),
            &format_ts(last),
            &format_ts(expires),
            now,
        );
        assert_eq!(check, SessionCheck::Invalid);
    }

    #[test]
    fn unparseable_timestamps_fail_closed() {
        assert_eq!(
            evaluate("garbage", "garbage", "garbage", Utc::now()),
            SessionCheck::Invalid
        );
    }
}
