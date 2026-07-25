//! Password hashing/verification — ADR-002 Decision 1.
//!
//! argon2id (`Argon2::default()` = v19, m=19456, t=2, p=1), the current OWASP
//! first choice. The full PHC string is stored, so params travel with the hash.
//! Both hashing and verification are CPU/memory-heavy by design, so callers run
//! them on `tokio::task::spawn_blocking` (see `crate::auth`), never on the async
//! runtime.

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::rngs::OsRng;
use std::sync::OnceLock;

/// A process-lifetime dummy argon2id hash used for the constant-time dummy
/// verify on an unknown user (ADR-002 Decision 1), so login timing can't
/// distinguish "no such user" from "wrong password". Computed lazily from a
/// random throwaway password (rather than a fragile hardcoded literal) so it is
/// always a valid PHC string with the current default params, and it can never
/// match a real login (the source password is never exposed).
pub fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        let mut secret = [0u8; 32];
        rand::RngCore::fill_bytes(&mut OsRng, &mut secret);
        let secret = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD_NO_PAD,
            secret,
        );
        hash_password(&secret).expect("hash dummy password")
    })
}

/// Hash a password into an argon2id PHC string. CPU/memory-heavy — call on a
/// blocking thread.
pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("failed to hash password: {e}"))?;
    Ok(hash.to_string())
}

/// Verify `password` against a stored PHC hash in constant time. A malformed
/// stored hash returns `false` (never an error that could leak via a 500).
/// CPU/memory-heavy — call on a blocking thread.
pub fn verify_password(password: &str, phc: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(phc) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_then_verify_round_trips() {
        let hash = hash_password("correct horse battery staple").expect("hash");
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("wrong password", &hash));
    }

    #[test]
    fn distinct_salts_produce_distinct_hashes() {
        let a = hash_password("same").expect("hash");
        let b = hash_password("same").expect("hash");
        assert_ne!(a, b, "a fresh salt must make each hash unique");
        assert!(verify_password("same", &a));
        assert!(verify_password("same", &b));
    }

    #[test]
    fn dummy_hash_is_a_valid_phc_string() {
        // The dummy-verify path must run a real argon2 verification (that's the
        // point — equal timing), so it must parse as a PHC hash.
        let dummy = dummy_hash();
        assert!(PasswordHash::new(dummy).is_ok());
        // And it must not match a plausible password.
        assert!(!verify_password("password", dummy));
        // Stable across calls (process-lifetime).
        assert_eq!(dummy, dummy_hash());
    }

    #[test]
    fn malformed_stored_hash_verifies_false() {
        assert!(!verify_password("anything", "not-a-phc-string"));
    }
}
