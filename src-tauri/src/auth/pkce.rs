//! PKCE (RFC 7636) verifier/challenge generation, and the OAuth `state` CSRF nonce.
//! Mirrors skillshome-app's `lib/authUtils.ts` `generateCodeVerifier`/`generateCodeChallenge`
//! (same sizes, same base64url encoding, same S256 method) so the two sides agree on format
//! even though they never call each other directly for this step.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::Rng;
use sha2::{Digest, Sha256};

/// A 32-byte random value, base64url-encoded (43 chars) — well within RFC 7636's
/// 43-128 char requirement for the code verifier.
pub fn generate_code_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Derives the S256 code challenge from a verifier: base64url(sha256(verifier)).
pub fn generate_code_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

/// A random CSRF nonce for the OAuth `state` parameter — same size/encoding as the
/// verifier, but semantically distinct (never sent as a code_verifier).
pub fn generate_state() -> String {
    generate_code_verifier()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifier_is_43_to_128_chars() {
        let v = generate_code_verifier();
        assert!(v.len() >= 43 && v.len() <= 128, "verifier length {} out of RFC 7636 range", v.len());
    }

    #[test]
    fn verifier_is_url_safe_no_padding() {
        let v = generate_code_verifier();
        assert!(!v.contains('+') && !v.contains('/') && !v.contains('='));
    }

    #[test]
    fn challenge_is_deterministic_for_same_verifier() {
        let verifier = "a-fixed-test-verifier-value-for-this-unit-test-1234";
        assert_eq!(generate_code_challenge(verifier), generate_code_challenge(verifier));
    }

    #[test]
    fn challenge_differs_from_verifier() {
        let verifier = generate_code_verifier();
        assert_ne!(generate_code_challenge(&verifier), verifier);
    }

    #[test]
    fn known_rfc7636_test_vector() {
        // RFC 7636 Appendix B example verifier/challenge pair.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let expected_challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert_eq!(generate_code_challenge(verifier), expected_challenge);
    }

    #[test]
    fn two_generated_verifiers_are_different() {
        assert_ne!(generate_code_verifier(), generate_code_verifier());
    }
}
