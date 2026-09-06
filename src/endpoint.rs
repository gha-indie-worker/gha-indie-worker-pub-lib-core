//! Typed API endpoints and safe URL construction for the public SDKs.
//!
//! Client SDKs in five languages should not each hand-concatenate paths. They should not, in
//! particular, each *forget to escape* the same identifier: a run id containing `../` or `?` turns
//! a request for one resource into a request for another, and the bug looks identical in every
//! language it is written in.
//!
//! So paths are built here, from a closed [`Endpoint`] set, with percent-encoding applied to every
//! interpolated segment — and identifiers are validated before that as a second line of defence.

use core::fmt;

/// Every path a public client may call. Closed on purpose: an SDK cannot invent a path, and adding
/// one is a reviewable change here rather than a string somewhere in a generated client.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Endpoint {
    Health,
    Ready,
    /// `GET /v1/organizations`
    Organizations,
    /// `GET|POST /v1/organizations/{org}/members`
    Members { organization: String },
    /// `POST /v1/organizations/{org}/invitations`
    Invitations { organization: String },
    /// `DELETE /v1/organizations/{org}/invitations/{id}`
    Invitation { organization: String, invitation: String },
    /// `GET /v1/runs`
    Runs,
    /// `GET /v1/runs/{run}`
    Run { run: String },
    /// `GET /v1/runs/{run}/logs`
    RunLogs { run: String },
    /// `POST /v1/runs/{run}/cancel`
    CancelRun { run: String },
    /// `GET /v1/runners`
    Runners,
    /// `GET|PUT /v1/caches/{key}`
    Cache { key: String },
    /// `GET|POST /v1/artifacts/{run}`
    Artifacts { run: String },
    /// WebSocket upgrade for the live session.
    Session,
}

/// Why an identifier was refused before it ever reached a URL.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdError {
    Empty,
    TooLong,
    /// Contains a character an identifier of ours never contains.
    Illegal,
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdError::Empty => f.write_str("identifier is empty"),
            IdError::TooLong => f.write_str("identifier is too long"),
            IdError::Illegal => f.write_str("identifier contains an illegal character"),
        }
    }
}

/// Validate an identifier: ASCII alphanumerics plus `-`, `_` and `.`, at most 128 bytes.
///
/// Escaping alone would be enough to make a URL safe, but an id that needs escaping is a bug
/// somewhere upstream, and failing loudly beats silently requesting `%2E%2E%2F`.
///
/// # Errors
/// [`IdError`].
pub fn validate_id(value: &str) -> Result<&str, IdError> {
    if value.is_empty() {
        return Err(IdError::Empty);
    }
    if value.len() > 128 {
        return Err(IdError::TooLong);
    }
    if value.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.')) {
        Ok(value)
    } else {
        Err(IdError::Illegal)
    }
}

/// Percent-encode one path segment. Unreserved characters (RFC 3986 §2.3) pass through; everything
/// else — including `/`, `?`, `#`, `%` and every non-ASCII byte — is encoded.
#[must_use]
pub fn encode_segment(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0').to_ascii_uppercase());
            out.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0').to_ascii_uppercase());
        }
    }
    out
}

impl Endpoint {
    /// The path, with every interpolated segment percent-encoded.
    #[must_use]
    pub fn path(&self) -> String {
        match self {
            Endpoint::Health => "/healthz".to_owned(),
            Endpoint::Ready => "/readyz".to_owned(),
            Endpoint::Organizations => "/v1/organizations".to_owned(),
            Endpoint::Members { organization } => {
                format!("/v1/organizations/{}/members", encode_segment(organization))
            }
            Endpoint::Invitations { organization } => {
                format!("/v1/organizations/{}/invitations", encode_segment(organization))
            }
            Endpoint::Invitation { organization, invitation } => format!(
                "/v1/organizations/{}/invitations/{}",
                encode_segment(organization),
                encode_segment(invitation)
            ),
            Endpoint::Runs => "/v1/runs".to_owned(),
            Endpoint::Run { run } => format!("/v1/runs/{}", encode_segment(run)),
            Endpoint::RunLogs { run } => format!("/v1/runs/{}/logs", encode_segment(run)),
            Endpoint::CancelRun { run } => format!("/v1/runs/{}/cancel", encode_segment(run)),
            Endpoint::Runners => "/v1/runners".to_owned(),
            Endpoint::Cache { key } => format!("/v1/caches/{}", encode_segment(key)),
            Endpoint::Artifacts { run } => format!("/v1/artifacts/{}", encode_segment(run)),
            Endpoint::Session => "/v1/session".to_owned(),
        }
    }

    /// Whether this endpoint needs a bearer token. Health and readiness deliberately do not: a
    /// probe that requires a credential is a probe that reports "unhealthy" when auth breaks.
    #[must_use]
    pub const fn requires_authentication(&self) -> bool {
        !matches!(self, Endpoint::Health | Endpoint::Ready)
    }

    /// Whether a failed call may be repeated without changing the world. `POST /cancel` is
    /// idempotent in effect (cancelling twice cancels once), so it is included deliberately.
    #[must_use]
    pub const fn is_idempotent(&self) -> bool {
        !matches!(self, Endpoint::Invitations { .. })
    }
}

/// Join a base URL and an endpoint without producing a double slash or dropping a base path.
#[must_use]
pub fn url(base: &str, endpoint: &Endpoint) -> String {
    format!("{}{}", base.trim_end_matches('/'), endpoint.path())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_that_could_change_the_target_are_refused() {
        assert!(validate_id("run_01H8XYZ").is_ok());
        assert!(validate_id("a.b-c_d").is_ok());
        assert_eq!(validate_id(""), Err(IdError::Empty));
        assert_eq!(validate_id(&"a".repeat(129)), Err(IdError::TooLong));
        for hostile in ["../admin", "a/b", "a?b", "a#b", "a%2f", "a b", "a\nb", "é"] {
            assert_eq!(validate_id(hostile), Err(IdError::Illegal), "input {hostile:?}");
        }
    }

    #[test]
    fn traversal_and_query_injection_cannot_escape_a_path_segment() {
        // Even if validation were bypassed, encoding must contain the damage.
        let run = "../../v1/organizations?x=1#y";
        let path = Endpoint::Run { run: run.to_owned() }.path();
        assert_eq!(path, "/v1/runs/..%2F..%2Fv1%2Forganizations%3Fx%3D1%23y");
        assert!(!path.contains("/organizations"), "path escaped its segment: {path}");
        assert_eq!(path.matches('/').count(), 3, "no extra path separators: {path}");
    }

    #[test]
    fn encoding_follows_rfc_3986_unreserved() {
        assert_eq!(encode_segment("aZ0-_.~"), "aZ0-_.~");
        assert_eq!(encode_segment("a/b"), "a%2Fb");
        assert_eq!(encode_segment(" "), "%20");
        assert_eq!(encode_segment("%"), "%25");
        assert_eq!(encode_segment("é"), "%C3%A9", "multi-byte UTF-8 is encoded per byte");
    }

    #[test]
    fn urls_join_cleanly_whatever_the_base_looks_like() {
        for base in ["https://api.indiebuild.dev", "https://api.indiebuild.dev/"] {
            assert_eq!(url(base, &Endpoint::Runs), "https://api.indiebuild.dev/v1/runs");
        }
        assert_eq!(
            url("https://example.test/gateway/", &Endpoint::Health),
            "https://example.test/gateway/healthz",
            "a base path is preserved"
        );
    }

    #[test]
    fn probes_do_not_require_a_credential() {
        assert!(!Endpoint::Health.requires_authentication());
        assert!(!Endpoint::Ready.requires_authentication());
        assert!(Endpoint::Runs.requires_authentication());
        assert!(Endpoint::Session.requires_authentication());
    }

    #[test]
    fn creating_an_invitation_is_the_one_call_a_client_must_not_blindly_repeat() {
        assert!(!Endpoint::Invitations { organization: "acme".into() }.is_idempotent());
        assert!(Endpoint::CancelRun { run: "r1".into() }.is_idempotent());
        assert!(Endpoint::Runs.is_idempotent());
    }
}
