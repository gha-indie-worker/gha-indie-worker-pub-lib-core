#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

pub mod backoff;
pub mod endpoint;

/// Contract version this SDK core implements. Bumped with the API's major version.
pub const CONTRACT_VERSION: &str = "1.0.0";
