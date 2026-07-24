//! At-rest encryption for the `credentials` table, per DESIGN.md ADR-001
//! ("Decision 1 — credentials table + at-rest encryption").
//!
//! AEAD: `XChaCha20Poly1305` (RustCrypto, pure Rust — no AES-NI dependence,
//! 192-bit nonce safe to generate randomly per write). Key material comes
//! from `ZYNC_SECRET_KEY` (base64, must decode to exactly 32 bytes), decoded
//! once at startup into a `zeroize`-wrapped buffer held on `AppState`.

use base64::Engine;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    Key, XChaCha20Poly1305, XNonce,
};
use rand::{rngs::OsRng, RngCore};
use zeroize::Zeroizing;

pub const KEY_LEN: usize = 32;
pub const NONCE_LEN: usize = 24;

/// A decoded 32-byte encryption key. Zeroed on drop.
#[derive(Clone)]
pub struct SecretKey(Zeroizing<[u8; KEY_LEN]>);

/// The server-wide encryption key state, resolved once at startup.
///
/// Kept as an enum (rather than `Option<SecretKey>`) so call sites and log
/// messages can distinguish "never configured" from "explicitly using the
/// insecure dev fallback" without re-deriving that from env vars.
#[derive(Clone)]
pub enum KeyState {
    /// A valid `ZYNC_SECRET_KEY` was decoded.
    Configured(SecretKey),
    /// No valid key was configured, but `ZYNC_DEV=1` (or `--dev`) requested
    /// the fixed all-zero fallback. Logged loudly at startup.
    DevFallback(SecretKey),
    /// No usable key. Credential create/decrypt operations must fail fast
    /// with `CryptoError::NotConfigured` rather than panicking or silently
    /// storing plaintext.
    Unconfigured,
}

impl KeyState {
    /// Resolve the key from `ZYNC_SECRET_KEY` / `ZYNC_DEV` at process
    /// startup. Never panics — an unusable key degrades credential features
    /// only, per ADR-001 ("Refusing the op, not crashing the server").
    pub fn load() -> Self {
        let dev_mode = is_dev_mode();

        if let Ok(raw) = std::env::var("ZYNC_SECRET_KEY") {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                match decode_key(trimmed) {
                    Some(bytes) => return KeyState::Configured(SecretKey(Zeroizing::new(bytes))),
                    None => {
                        tracing::warn!(
                            "ZYNC_SECRET_KEY is set but is not valid base64-encoded {KEY_LEN} bytes; \
                             credential storage is disabled until it is fixed"
                        );
                    }
                }
            }
        }

        if dev_mode {
            tracing::warn!(
                "ZYNC_DEV=1 and no valid ZYNC_SECRET_KEY: falling back to a fixed all-zero dev \
                 encryption key. Stored credentials are NOT meaningfully encrypted — do not let \
                 this database leave the dev machine."
            );
            return KeyState::DevFallback(SecretKey(Zeroizing::new([0u8; KEY_LEN])));
        }

        KeyState::Unconfigured
    }

    /// Borrow the active key, or a clear, non-secret error if none is usable.
    pub fn key(&self) -> Result<&SecretKey, CryptoError> {
        match self {
            KeyState::Configured(key) | KeyState::DevFallback(key) => Ok(key),
            KeyState::Unconfigured => Err(CryptoError::NotConfigured),
        }
    }
}

fn is_dev_mode() -> bool {
    std::env::var("ZYNC_DEV").map(|v| v == "1").unwrap_or(false)
        || std::env::args().any(|arg| arg == "--dev")
}

fn decode_key(raw: &str) -> Option<[u8; KEY_LEN]> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(raw)
        .ok()?;
    decoded.try_into().ok()
}

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error(
        "credentials disabled: set ZYNC_SECRET_KEY (base64, 32 bytes) to enable encrypted \
         credential storage"
    )]
    NotConfigured,
    #[error("failed to encrypt credential secret")]
    Encrypt,
    #[error("failed to decrypt credential secret")]
    Decrypt,
}

/// Encrypt `plaintext` under `key`, returning `(ciphertext, nonce)`. A fresh
/// random 24-byte nonce is generated per call (safe for XChaCha20's 192-bit
/// nonce space — see ADR-001).
pub fn encrypt(key: &SecretKey, plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>), CryptoError> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&*key.0));
    // `OsRng` per ADR-001 — a direct OS-CSPRNG read, rather than `rand::thread_rng()`'s
    // thread-local (also OS-seeded, but reseeded periodically rather than read fresh
    // per call).
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| CryptoError::Encrypt)?;
    Ok((ciphertext, nonce_bytes.to_vec()))
}

/// Decrypt a `(ciphertext, nonce)` pair produced by [`encrypt`]. Consumed by
/// `crate::credentials::decrypt_secret_bundle`, which the remote-op handlers
/// (`crate::git`, `crate::repository`) call just-in-time to build a
/// `CredentialSpec`.
///
/// Returns `Zeroizing<Vec<u8>>` rather than a bare `Vec<u8>` — the plaintext
/// is the decrypted secret bundle (token / private key / passphrase), and
/// per ADR-001 it must not sit in an un-wiped heap allocation after the
/// caller is done with it. Callers still owe the *parsed* form (e.g. a
/// deserialized struct) its own zeroize-on-drop handling; this only covers
/// the raw bytes.
pub fn decrypt(
    key: &SecretKey,
    ciphertext: &[u8],
    nonce: &[u8],
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    if nonce.len() != NONCE_LEN {
        return Err(CryptoError::Decrypt);
    }
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&*key.0));
    let nonce = XNonce::from_slice(nonce);
    cipher
        .decrypt(nonce, ciphertext)
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::Decrypt)
}

/// Construct a fixed-byte key for tests, including tests in sibling modules
/// (e.g. `crate::credentials`) that need a real `SecretKey` without going
/// through env vars.
#[cfg(test)]
pub fn test_key(byte: u8) -> SecretKey {
    SecretKey(Zeroizing::new([byte; KEY_LEN]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> SecretKey {
        super::test_key(7)
    }

    #[test]
    fn round_trips_plaintext() {
        let key = test_key();
        let plaintext = br#"{"token":"ghp_example"}"#;
        let (ciphertext, nonce) = encrypt(&key, plaintext).expect("encrypt");
        assert_ne!(ciphertext, plaintext, "ciphertext must not equal plaintext");
        assert_eq!(nonce.len(), NONCE_LEN);
        let decrypted = decrypt(&key, &ciphertext, &nonce).expect("decrypt");
        assert_eq!(decrypted.as_slice(), plaintext);
    }

    #[test]
    fn nonce_is_random_per_call() {
        let key = test_key();
        let (_, nonce_a) = encrypt(&key, b"same plaintext").unwrap();
        let (_, nonce_b) = encrypt(&key, b"same plaintext").unwrap();
        assert_ne!(nonce_a, nonce_b);
    }

    #[test]
    fn decrypt_fails_with_wrong_key() {
        let key_a = test_key();
        let key_b = SecretKey(Zeroizing::new([9u8; KEY_LEN]));
        let (ciphertext, nonce) = encrypt(&key_a, b"secret").unwrap();
        assert!(decrypt(&key_b, &ciphertext, &nonce).is_err());
    }

    #[test]
    fn decrypt_fails_with_tampered_ciphertext() {
        let key = test_key();
        let (mut ciphertext, nonce) = encrypt(&key, b"secret").unwrap();
        let last = ciphertext.len() - 1;
        ciphertext[last] ^= 0xFF;
        assert!(decrypt(&key, &ciphertext, &nonce).is_err());
    }

    #[test]
    fn decode_key_requires_exact_length() {
        let too_short = base64::engine::general_purpose::STANDARD.encode([1u8; 16]);
        assert!(decode_key(&too_short).is_none());
        let exact = base64::engine::general_purpose::STANDARD.encode([1u8; KEY_LEN]);
        assert!(decode_key(&exact).is_some());
    }

    #[test]
    fn unconfigured_key_state_errors_without_panicking() {
        let state = KeyState::Unconfigured;
        assert!(matches!(state.key(), Err(CryptoError::NotConfigured)));
    }
}
