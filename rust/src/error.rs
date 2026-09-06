//! Typed client errors. Hand-written rather than derived so the crate keeps a
//! dependency list a browser bundle can afford.

use alloc::string::String;
use core::fmt;

pub type Result<T> = core::result::Result<T, ClientError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientError {
    /// A required runtime configuration key was absent or empty.
    MissingConfig { key: &'static str },
    /// A configuration value was present but unusable.
    InvalidConfig { key: &'static str, reason: String },
    /// A cursor did not decode, or was not produced by this API.
    InvalidCursor,
    /// A page size outside the server's accepted range.
    PageLimitOutOfRange { got: u32, max: u32 },
    /// The signing key was rejected — wrong length, or empty.
    InvalidSigningKey,
    /// A signature did not verify. Carries nothing: telling a caller *why* a
    /// signature failed is how signature oracles are built.
    SignatureMismatch,
    /// A timestamp outside the accepted skew window.
    StaleTimestamp { skew_seconds: i64, max_seconds: i64 },
    /// An onboarding transition the state machine does not allow.
    IllegalTransition {
        machine: &'static str,
        from: &'static str,
        to: &'static str,
    },
    /// A server response that did not match the contract.
    MalformedResponse { reason: String },
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingConfig { key } => write!(f, "missing runtime configuration key {key}"),
            Self::InvalidConfig { key, reason } => {
                write!(f, "invalid runtime configuration for {key}: {reason}")
            }
            Self::InvalidCursor => f.write_str("cursor is not a cursor this API issued"),
            Self::PageLimitOutOfRange { got, max } => {
                write!(f, "page limit {got} is outside 1..={max}")
            }
            Self::InvalidSigningKey => f.write_str("signing key is empty or the wrong length"),
            Self::SignatureMismatch => f.write_str("signature did not verify"),
            Self::StaleTimestamp {
                skew_seconds,
                max_seconds,
            } => write!(
                f,
                "timestamp is {skew_seconds}s from now; the window is ±{max_seconds}s"
            ),
            Self::IllegalTransition { machine, from, to } => {
                write!(f, "illegal {machine} transition: {from} -> {to}")
            }
            Self::MalformedResponse { reason } => write!(f, "malformed response: {reason}"),
        }
    }
}

impl core::error::Error for ClientError {}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn a_signature_mismatch_says_nothing_about_why() {
        let rendered = ClientError::SignatureMismatch.to_string();
        assert_eq!(rendered, "signature did not verify");
        assert!(!rendered.contains("expected"));
        assert!(!rendered.contains("got"));
    }

    #[test]
    fn errors_render_the_offending_key() {
        assert!(ClientError::MissingConfig {
            key: "GHA_INDIE_WORKER_API_BASE"
        }
        .to_string()
        .contains("GHA_INDIE_WORKER_API_BASE"));
    }
}
