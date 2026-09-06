//! `gha-indie-worker-pub-lib-core` — the **public** client-side core for
//! GHA Indie Worker.
//!
//! This crate is the half of the shared code that is safe to ship to a browser,
//! a desktop app or a phone. It contains typed API resources, request signing,
//! cursor pagination, and a mirror of the onboarding state machine so a UI can
//! move optimistically and still be correct.
//!
//! What it deliberately does **not** contain:
//!
//! * no HTTP client. The transport is the host's — `fetch` in a browser,
//!   `reqwest` on a desktop, `dart:io` under Flutter. This crate describes
//!   *what* to send; the host decides *how*;
//! * no database access, no table names, no SQL. That is
//!   `gha-indie-worker-orm-core`, which is backend-only;
//! * no server-side secrets. [`signing`] signs with a credential the *client*
//!   legitimately holds; it is not the server's introspection secret.
//!
//! ## no_std
//!
//! With `default-features = false` the crate is `no_std` + `alloc`, which is
//! what makes it usable from a `wasm32-unknown-unknown` island without dragging
//! in a runtime. The `std` feature adds exactly one thing: reading the runtime
//! configuration from process environment variables. Everything else is
//! available either way.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod config;
pub mod error;
pub mod onboarding;
pub mod pagination;
pub mod resources;
pub mod signing;

pub use config::{RuntimeConfig, Surface};
pub use error::{ClientError, Result};
pub use onboarding::{OrgOnboardingState, UserOnboardingState};
pub use pagination::{Cursor, Page, PageRequest};
pub use resources::{JobResource, RunResource, WorkerResource};
pub use signing::{sign_request, verify_signature, SignedRequest};

/// Version of this crate, for the `User-Agent` and for telemetry correlation.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The `x-giw-client` header value every host transport should send.
pub const CLIENT_HEADER: &str = "x-giw-client";
