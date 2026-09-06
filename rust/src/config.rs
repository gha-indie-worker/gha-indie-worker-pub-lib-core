//! Runtime configuration, assigned by flags-2-env conventions.
//!
//! The fleet declares configuration once in `.cli-flags.toml` and flags-2-env
//! emits the key names for every language. This module is the client half of
//! that: the same keys, a value type per key, and validation.
//!
//! The lookup is a *parameter*, not an ambient effect. A browser passes a
//! closure over a `<script type="application/json">` blob or `import.meta.env`;
//! a Flutter app passes one over `--dart-define`; a native binary uses
//! [`RuntimeConfig::from_env`] behind the `std` feature. The parsing and
//! validation are identical in all three, which is the point.

use alloc::borrow::ToOwned;
use alloc::string::String;

use crate::error::{ClientError, Result};

/// `GHA_INDIE_WORKER_API_BASE` — origin of the JSON API (`api.indiebuild.dev`).
pub const API_BASE: &str = "GHA_INDIE_WORKER_API_BASE";
/// `GHA_INDIE_WORKER_WS_BASE` — origin for `/v1/ws`. Defaults from the API base.
pub const WS_BASE: &str = "GHA_INDIE_WORKER_WS_BASE";
/// `GHA_INDIE_WORKER_SURFACE` — which host this client is running on.
pub const SURFACE: &str = "GHA_INDIE_WORKER_SURFACE";
/// `GHA_INDIE_WORKER_CLIENT_TIMEOUT_MS` — per-request deadline.
pub const CLIENT_TIMEOUT_MS: &str = "GHA_INDIE_WORKER_CLIENT_TIMEOUT_MS";
/// `SHARED_AUTH_BASE` — the federated auth origin (`auth.indiebuild.dev`).
pub const SHARED_AUTH_BASE: &str = "SHARED_AUTH_BASE";
/// `SHARED_AUTH_AUDIENCE` — the audience this client's tokens must carry.
pub const SHARED_AUTH_AUDIENCE: &str = "SHARED_AUTH_AUDIENCE";

/// Every key this crate reads, for a host that wants to validate its own
/// environment up front rather than discovering a gap at first request.
pub const KEYS: &[&str] = &[
    API_BASE,
    WS_BASE,
    SURFACE,
    CLIENT_TIMEOUT_MS,
    SHARED_AUTH_BASE,
    SHARED_AUTH_AUDIENCE,
];

pub const DEFAULT_TIMEOUT_MS: u32 = 30_000;
const MAX_TIMEOUT_MS: u32 = 600_000;

/// Which `indiebuild.dev` host the client is talking to. The surface changes
/// which entry points a UI offers, not which API it calls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// `app.indiebuild.dev` — marketing plus the primary product UI.
    App,
    /// `user.indiebuild.dev` — B2C: individual login and personal dashboards.
    User,
    /// `org.indiebuild.dev` — B2B: org login, multi-seat onboarding, team pages.
    Org,
    /// `m.indiebuild.dev` — mobile web, compact layout.
    Mobile,
    /// A packaged desktop or mobile application, not a browser origin.
    NativeApp,
}

impl Surface {
    pub const ALL: &'static [Self] = &[
        Self::App,
        Self::User,
        Self::Org,
        Self::Mobile,
        Self::NativeApp,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::User => "user",
            Self::Org => "org",
            Self::Mobile => "mobile",
            Self::NativeApp => "native_app",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "app" => Some(Self::App),
            "user" => Some(Self::User),
            "org" => Some(Self::Org),
            "mobile" => Some(Self::Mobile),
            "native_app" => Some(Self::NativeApp),
            _ => None,
        }
    }

    /// The B2B surfaces are the ones that show seat and invitation UI.
    pub const fn is_organization(self) -> bool {
        matches!(self, Self::Org)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub api_base: String,
    pub ws_base: String,
    pub surface: Surface,
    pub timeout_ms: u32,
    pub shared_auth_base: Option<String>,
    pub shared_auth_audience: Option<String>,
}

impl RuntimeConfig {
    /// Build from any key/value lookup. Pure: the caller owns the effect.
    pub fn from_lookup<F>(lookup: F) -> Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let api_base = require(&lookup, API_BASE)?;
        validate_origin(API_BASE, &api_base)?;

        let ws_base = match lookup(WS_BASE).filter(|v| !v.trim().is_empty()) {
            Some(explicit) => {
                validate_ws_origin(WS_BASE, &explicit)?;
                explicit
            }
            None => derive_ws_base(&api_base),
        };

        let surface = match lookup(SURFACE).filter(|v| !v.trim().is_empty()) {
            Some(raw) => Surface::parse(raw.trim()).ok_or(ClientError::InvalidConfig {
                key: SURFACE,
                reason: raw,
            })?,
            None => Surface::App,
        };

        let timeout_ms = match lookup(CLIENT_TIMEOUT_MS).filter(|v| !v.trim().is_empty()) {
            Some(raw) => {
                let parsed: u32 = raw.trim().parse().map_err(|_| ClientError::InvalidConfig {
                    key: CLIENT_TIMEOUT_MS,
                    reason: raw.clone(),
                })?;
                if parsed == 0 || parsed > MAX_TIMEOUT_MS {
                    return Err(ClientError::InvalidConfig {
                        key: CLIENT_TIMEOUT_MS,
                        reason: raw,
                    });
                }
                parsed
            }
            None => DEFAULT_TIMEOUT_MS,
        };

        Ok(Self {
            api_base,
            ws_base,
            surface,
            timeout_ms,
            shared_auth_base: lookup(SHARED_AUTH_BASE).filter(|v| !v.trim().is_empty()),
            shared_auth_audience: lookup(SHARED_AUTH_AUDIENCE).filter(|v| !v.trim().is_empty()),
        })
    }

    /// Read from process environment variables. Native hosts only.
    #[cfg(feature = "std")]
    pub fn from_env() -> Result<Self> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Join a path onto the API origin without ever producing a double slash or
    /// silently escaping the origin.
    pub fn endpoint(&self, path: &str) -> String {
        let mut out = self.api_base.trim_end_matches('/').to_owned();
        if !path.starts_with('/') {
            out.push('/');
        }
        out.push_str(path);
        out
    }
}

fn require<F>(lookup: &F, key: &'static str) -> Result<String>
where
    F: Fn(&str) -> Option<String>,
{
    lookup(key)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or(ClientError::MissingConfig { key })
}

fn validate_origin(key: &'static str, value: &str) -> Result<()> {
    if !(value.starts_with("https://") || value.starts_with("http://")) {
        return Err(ClientError::InvalidConfig {
            key,
            reason: value.to_owned(),
        });
    }
    // A base that already carries a query or fragment would corrupt every path
    // joined onto it.
    if value.contains('?') || value.contains('#') {
        return Err(ClientError::InvalidConfig {
            key,
            reason: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_ws_origin(key: &'static str, value: &str) -> Result<()> {
    if !(value.starts_with("wss://") || value.starts_with("ws://")) {
        return Err(ClientError::InvalidConfig {
            key,
            reason: value.to_owned(),
        });
    }
    Ok(())
}

/// `https://api.…` → `wss://api.…`, `http://…` → `ws://…`. Keeping the scheme's
/// security level is the whole point: a plaintext websocket from an https page
/// is blocked by the browser anyway, and silently downgrading would be worse.
fn derive_ws_base(api_base: &str) -> String {
    if let Some(rest) = api_base.strip_prefix("https://") {
        let mut out = String::from("wss://");
        out.push_str(rest);
        out
    } else if let Some(rest) = api_base.strip_prefix("http://") {
        let mut out = String::from("ws://");
        out.push_str(rest);
        out
    } else {
        api_base.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;
    use alloc::vec::Vec;

    use super::*;

    fn lookup_from(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + '_ {
        move |key| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_string())
        }
    }

    #[test]
    fn a_minimal_configuration_derives_the_websocket_origin() {
        let config =
            RuntimeConfig::from_lookup(lookup_from(&[(API_BASE, "https://api.indiebuild.dev")]))
                .expect("valid");
        assert_eq!(config.ws_base, "wss://api.indiebuild.dev");
        assert_eq!(config.surface, Surface::App);
        assert_eq!(config.timeout_ms, DEFAULT_TIMEOUT_MS);
        assert!(config.shared_auth_base.is_none());
    }

    #[test]
    fn a_plaintext_api_base_derives_a_plaintext_websocket_and_never_upgrades() {
        let config =
            RuntimeConfig::from_lookup(lookup_from(&[(API_BASE, "http://127.0.0.1:8080")]))
                .expect("valid");
        assert_eq!(config.ws_base, "ws://127.0.0.1:8080");
    }

    #[test]
    fn the_api_base_is_required() {
        assert_eq!(
            RuntimeConfig::from_lookup(lookup_from(&[])),
            Err(ClientError::MissingConfig { key: API_BASE })
        );
        assert_eq!(
            RuntimeConfig::from_lookup(lookup_from(&[(API_BASE, "   ")])),
            Err(ClientError::MissingConfig { key: API_BASE })
        );
    }

    #[test]
    fn a_base_without_a_scheme_or_with_a_query_is_rejected() {
        for bad in [
            "api.indiebuild.dev",
            "ftp://api.indiebuild.dev",
            "https://api.indiebuild.dev?tenant=1",
            "https://api.indiebuild.dev#x",
        ] {
            assert!(
                RuntimeConfig::from_lookup(lookup_from(&[(API_BASE, bad)])).is_err(),
                "{bad} should be rejected"
            );
        }
    }

    #[test]
    fn an_explicit_websocket_base_must_be_a_websocket_scheme() {
        assert!(RuntimeConfig::from_lookup(lookup_from(&[
            (API_BASE, "https://api.indiebuild.dev"),
            (WS_BASE, "https://api.indiebuild.dev"),
        ]))
        .is_err());

        let config = RuntimeConfig::from_lookup(lookup_from(&[
            (API_BASE, "https://api.indiebuild.dev"),
            (WS_BASE, "wss://ws.indiebuild.dev"),
        ]))
        .expect("valid");
        assert_eq!(config.ws_base, "wss://ws.indiebuild.dev");
    }

    #[test]
    fn every_surface_round_trips_and_unknown_surfaces_are_rejected() {
        for surface in Surface::ALL {
            assert_eq!(Surface::parse(surface.as_str()), Some(*surface));
        }
        assert_eq!(Surface::parse("admin"), None);
        assert!(RuntimeConfig::from_lookup(lookup_from(&[
            (API_BASE, "https://api.indiebuild.dev"),
            (SURFACE, "admin"),
        ]))
        .is_err());
    }

    #[test]
    fn only_the_org_surface_shows_seat_ui() {
        let organization: Vec<Surface> = Surface::ALL
            .iter()
            .copied()
            .filter(|s| s.is_organization())
            .collect();
        assert_eq!(organization, alloc::vec![Surface::Org]);
    }

    #[test]
    fn a_zero_or_absurd_timeout_is_rejected() {
        for bad in ["0", "600001", "not-a-number", "-5"] {
            assert!(
                RuntimeConfig::from_lookup(lookup_from(&[
                    (API_BASE, "https://api.indiebuild.dev"),
                    (CLIENT_TIMEOUT_MS, bad),
                ]))
                .is_err(),
                "{bad} should be rejected"
            );
        }
    }

    #[test]
    fn endpoint_joins_without_doubling_the_separator() {
        let config =
            RuntimeConfig::from_lookup(lookup_from(&[(API_BASE, "https://api.indiebuild.dev/")]))
                .expect("valid");
        assert_eq!(
            config.endpoint("/v1/runs"),
            "https://api.indiebuild.dev/v1/runs"
        );
        assert_eq!(
            config.endpoint("v1/runs"),
            "https://api.indiebuild.dev/v1/runs"
        );
    }

    #[test]
    fn the_declared_key_list_covers_every_key_the_module_reads() {
        for key in [
            API_BASE,
            WS_BASE,
            SURFACE,
            CLIENT_TIMEOUT_MS,
            SHARED_AUTH_BASE,
            SHARED_AUTH_AUDIENCE,
        ] {
            assert!(KEYS.contains(&key), "{key} is missing from KEYS");
        }
        assert_eq!(KEYS.len(), 6);
    }
}
