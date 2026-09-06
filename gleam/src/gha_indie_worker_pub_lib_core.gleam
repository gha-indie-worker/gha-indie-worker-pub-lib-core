//// `gha_indie_worker_pub_lib_core` — the public client-side core, Gleam port.
////
//// Gleam is a first-class client language in this org alongside Rust,
//// TypeScript and Dart. This package carries the parts of the client contract
//// that are pure and total: the onboarding state machines, the runtime
//// configuration keys and the resource types.
////
//// Transport is the host's. There is no HTTP client here, no database access
//// and no server secret — the same boundary the other three ports keep.

import gha_indie_worker_pub_lib_core/config
import gha_indie_worker_pub_lib_core/onboarding
import gha_indie_worker_pub_lib_core/resources
import gleam/int
import gleam/list

/// Version of this package, for the `User-Agent` and telemetry correlation.
pub const version = "0.1.0"

/// Header every host transport should send.
pub const client_header = "x-giw-client"

/// A one-line description of the contract this build carries. Doubles as a
/// smoke check that every module in the package links.
pub fn summary() -> String {
  "gha_indie_worker_pub_lib_core "
  <> version
  <> ": "
  <> int.to_string(list.length(onboarding.org_states()))
  <> " org states, "
  <> int.to_string(list.length(config.surfaces()))
  <> " surfaces, "
  <> int.to_string(list.length(resources.run_states()))
  <> " run states"
}
