//! Request signing for clients that hold their own credential.
//!
//! This is **not** the shared-auth introspection secret — that never leaves a
//! server. It is the per-installation key a desktop agent or a CI plugin is
//! issued, used to prove a request came from that installation.
//!
//! The signature covers a canonical string, not the raw request, so a proxy
//! that reorders headers or re-encodes a body cannot break it and — more
//! importantly — an attacker cannot move a signature onto a different method,
//! path or body:
//!
//! ```text
//! GIW1
//! <METHOD uppercased>
//! <path, no query>
//! <query string, or empty>
//! <unix seconds>
//! <lowercase hex sha256 of the body>
//! ```
//!
//! Every field is length-delimited by the newline and none may contain one, so
//! two different requests cannot produce the same canonical string.

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::error::{ClientError, Result};

type HmacSha256 = Hmac<Sha256>;

/// Signature scheme version, first line of the canonical string. Bumping it
/// invalidates every old signature by construction.
pub const SCHEME: &str = "GIW1";

/// How far a timestamp may be from now. Wide enough for a phone with a lazy
/// clock, narrow enough that a captured request expires quickly.
pub const MAX_SKEW_SECONDS: i64 = 300;

pub const SIGNATURE_HEADER: &str = "x-giw-signature";
pub const TIMESTAMP_HEADER: &str = "x-giw-timestamp";
pub const KEY_ID_HEADER: &str = "x-giw-key-id";

/// What the host transport must attach to the outgoing request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedRequest {
    pub key_id: String,
    pub timestamp: i64,
    /// Lowercase hex HMAC-SHA256.
    pub signature: String,
    /// The exact bytes that were signed. Returned so a caller can log or test
    /// against it; it contains no secret.
    pub canonical: String,
}

impl SignedRequest {
    /// The three headers, ready to attach.
    pub fn headers(&self) -> [(&'static str, String); 3] {
        [
            (KEY_ID_HEADER, self.key_id.clone()),
            (TIMESTAMP_HEADER, format_i64(self.timestamp)),
            (SIGNATURE_HEADER, self.signature.clone()),
        ]
    }
}

/// Build the canonical string. Pure and public, because the server implements
/// the same function and the two must agree byte for byte.
pub fn canonical_string(
    method: &str,
    path: &str,
    query: &str,
    timestamp: i64,
    body: &[u8],
) -> String {
    let mut out =
        String::with_capacity(SCHEME.len() + method.len() + path.len() + query.len() + 96);
    out.push_str(SCHEME);
    out.push('\n');
    for byte in method.bytes() {
        out.push(byte.to_ascii_uppercase() as char);
    }
    out.push('\n');
    out.push_str(path);
    out.push('\n');
    out.push_str(query.strip_prefix('?').unwrap_or(query));
    out.push('\n');
    out.push_str(&format_i64(timestamp));
    out.push('\n');
    out.push_str(&hex_lower(&Sha256::digest(body)));
    out
}

/// Sign a request.
///
/// `key` is raw bytes; an empty key is refused rather than producing a
/// signature anyone could forge. Newlines in `method`, `path` or `query` are
/// refused because they would let a caller forge a different canonical string.
pub fn sign_request(
    key_id: &str,
    key: &[u8],
    method: &str,
    path: &str,
    query: &str,
    timestamp: i64,
    body: &[u8],
) -> Result<SignedRequest> {
    if key.is_empty() || key_id.is_empty() {
        return Err(ClientError::InvalidSigningKey);
    }
    for field in [method, path, query, key_id] {
        if field.contains('\n') || field.contains('\r') {
            return Err(ClientError::InvalidSigningKey);
        }
    }
    let canonical = canonical_string(method, path, query, timestamp, body);
    let signature = hex_lower(&hmac_sha256(key, canonical.as_bytes())?);
    Ok(SignedRequest {
        key_id: key_id.to_owned(),
        timestamp,
        signature,
        canonical,
    })
}

/// Verify a signature in constant time, and reject a timestamp outside the
/// skew window *before* comparing — an expired signature is not worth a
/// comparison, and checking the window first keeps replay cheap to reject.
pub fn verify_signature(
    key: &[u8],
    canonical: &str,
    presented_hex: &str,
    timestamp: i64,
    now: i64,
) -> Result<()> {
    let skew = now.saturating_sub(timestamp);
    if skew.saturating_abs() > MAX_SKEW_SECONDS {
        return Err(ClientError::StaleTimestamp {
            skew_seconds: skew,
            max_seconds: MAX_SKEW_SECONDS,
        });
    }
    if key.is_empty() {
        return Err(ClientError::InvalidSigningKey);
    }
    let expected = hmac_sha256(key, canonical.as_bytes())?;
    let presented = hex_decode(presented_hex).ok_or(ClientError::SignatureMismatch)?;
    if presented.len() != expected.len() {
        return Err(ClientError::SignatureMismatch);
    }
    if bool::from(presented.ct_eq(&expected)) {
        Ok(())
    } else {
        Err(ClientError::SignatureMismatch)
    }
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> Result<Vec<u8>> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(|_| ClientError::InvalidSigningKey)?;
    mac.update(message);
    Ok(mac.finalize().into_bytes().to_vec())
}

/// Lowercase hex. Shared with the body digest, so both sides of the canonical
/// string use the same casing.
pub fn hex_lower(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[usize::from(byte >> 4)] as char);
        out.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    out
}

fn hex_decode(input: &str) -> Option<Vec<u8>> {
    if input.len() % 2 != 0 || input.is_empty() {
        return None;
    }
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks(2) {
        let high = hex_value(pair[0])?;
        let low = hex_value(pair[1])?;
        out.push((high << 4) | low);
    }
    Some(out)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        // Uppercase is rejected: one canonical casing means one signature.
        _ => None,
    }
}

fn format_i64(mut value: i64) -> String {
    if value == 0 {
        return String::from("0");
    }
    let negative = value < 0;
    let mut digits = [0u8; 20];
    let mut index = digits.len();
    while value != 0 {
        index -= 1;
        let digit = (value % 10).unsigned_abs() as u8;
        digits[index] = b'0' + digit;
        value /= 10;
    }
    let mut out = String::with_capacity(digits.len() - index + 1);
    if negative {
        out.push('-');
    }
    for digit in &digits[index..] {
        out.push(*digit as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &[u8] = b"a-client-installation-key";
    const NOW: i64 = 1_780_000_000;

    fn signed() -> SignedRequest {
        sign_request(
            "inst_abc",
            KEY,
            "post",
            "/v1/runs",
            "?limit=10",
            NOW,
            b"{\"repository\":\"a/b\"}",
        )
        .expect("valid")
    }

    #[test]
    fn the_canonical_string_has_exactly_six_lines_in_a_fixed_order() {
        let canonical = canonical_string("GET", "/v1/runs", "", NOW, b"");
        let lines: alloc::vec::Vec<&str> = canonical.split('\n').collect();
        assert_eq!(lines.len(), 6, "{canonical}");
        assert_eq!(lines[0], SCHEME);
        assert_eq!(lines[1], "GET");
        assert_eq!(lines[2], "/v1/runs");
        assert_eq!(lines[3], "");
        assert_eq!(lines[4], format_i64(NOW));
        // sha256 of the empty string.
        assert_eq!(
            lines[5],
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn the_method_is_upper_cased_and_the_query_loses_its_leading_question_mark() {
        let with = canonical_string("post", "/v1/runs", "?a=1", NOW, b"");
        let without = canonical_string("POST", "/v1/runs", "a=1", NOW, b"");
        assert_eq!(with, without);
    }

    #[test]
    fn a_signature_verifies_against_its_own_canonical_string() {
        let request = signed();
        verify_signature(KEY, &request.canonical, &request.signature, NOW, NOW)
            .expect("fresh signature verifies");
    }

    #[test]
    fn changing_any_covered_field_invalidates_the_signature() {
        let request = signed();
        for (method, path, query, body) in [
            (
                "GET",
                "/v1/runs",
                "?limit=10",
                &b"{\"repository\":\"a/b\"}"[..],
            ),
            (
                "POST",
                "/v1/jobs",
                "?limit=10",
                &b"{\"repository\":\"a/b\"}"[..],
            ),
            (
                "POST",
                "/v1/runs",
                "?limit=11",
                &b"{\"repository\":\"a/b\"}"[..],
            ),
            (
                "POST",
                "/v1/runs",
                "?limit=10",
                &b"{\"repository\":\"a/c\"}"[..],
            ),
        ] {
            let tampered = canonical_string(method, path, query, NOW, body);
            assert_eq!(
                verify_signature(KEY, &tampered, &request.signature, NOW, NOW),
                Err(ClientError::SignatureMismatch)
            );
        }
    }

    #[test]
    fn a_different_key_does_not_verify() {
        let request = signed();
        assert_eq!(
            verify_signature(
                b"another-key",
                &request.canonical,
                &request.signature,
                NOW,
                NOW
            ),
            Err(ClientError::SignatureMismatch)
        );
    }

    #[test]
    fn the_skew_window_is_checked_before_the_comparison() {
        let request = signed();
        let far = NOW + MAX_SKEW_SECONDS + 1;
        assert!(matches!(
            verify_signature(KEY, &request.canonical, &request.signature, NOW, far),
            Err(ClientError::StaleTimestamp { .. })
        ));
        // Symmetric: a client clock ahead of the server is just as stale.
        assert!(matches!(
            verify_signature(KEY, &request.canonical, &request.signature, far, NOW),
            Err(ClientError::StaleTimestamp { .. })
        ));
        // The boundary itself is inside the window.
        verify_signature(
            KEY,
            &request.canonical,
            &request.signature,
            NOW,
            NOW + MAX_SKEW_SECONDS,
        )
        .expect("the edge of the window is still valid");
    }

    #[test]
    fn an_empty_key_or_key_id_is_refused_rather_than_signed() {
        assert_eq!(
            sign_request("id", b"", "GET", "/", "", NOW, b""),
            Err(ClientError::InvalidSigningKey)
        );
        assert_eq!(
            sign_request("", KEY, "GET", "/", "", NOW, b""),
            Err(ClientError::InvalidSigningKey)
        );
    }

    #[test]
    fn a_newline_in_a_covered_field_is_refused() {
        // Without this, `path = "/a\nGET\n/b"` would let one signature stand for
        // two different requests.
        assert_eq!(
            sign_request("id", KEY, "GET", "/a\nGET\n/b", "", NOW, b""),
            Err(ClientError::InvalidSigningKey)
        );
        assert!(sign_request("id", KEY, "GET\r", "/a", "", NOW, b"").is_err());
    }

    #[test]
    fn signatures_are_lowercase_hex_of_the_right_length() {
        let request = signed();
        assert_eq!(request.signature.len(), 64);
        assert!(request
            .signature
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn an_uppercase_or_malformed_signature_does_not_verify() {
        let request = signed();
        let upper = request.signature.to_ascii_uppercase();
        assert_eq!(
            verify_signature(KEY, &request.canonical, &upper, NOW, NOW),
            Err(ClientError::SignatureMismatch)
        );
        for bad in ["", "abc", "zz"] {
            assert_eq!(
                verify_signature(KEY, &request.canonical, bad, NOW, NOW),
                Err(ClientError::SignatureMismatch)
            );
        }
    }

    #[test]
    fn the_headers_are_the_three_the_server_reads() {
        let headers = signed().headers();
        let names: alloc::vec::Vec<&str> = headers.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            names,
            alloc::vec![KEY_ID_HEADER, TIMESTAMP_HEADER, SIGNATURE_HEADER]
        );
    }

    #[test]
    fn integers_render_including_the_negative_and_extreme_cases() {
        assert_eq!(format_i64(0), "0");
        assert_eq!(format_i64(NOW), "1780000000");
        assert_eq!(format_i64(-42), "-42");
        assert_eq!(format_i64(i64::MIN), "-9223372036854775808");
        assert_eq!(format_i64(i64::MAX), "9223372036854775807");
    }

    #[test]
    fn hex_round_trips() {
        let bytes = [0x00u8, 0x0f, 0xa5, 0xff];
        assert_eq!(hex_lower(&bytes), "000fa5ff");
        assert_eq!(hex_decode("000fa5ff").expect("decode"), bytes.to_vec());
        assert!(hex_decode("000fa5f").is_none());
    }
}
