//// Runtime configuration, assigned by flags-2-env conventions.
////
//// The same key strings the Rust, TypeScript and Dart ports read. The lookup is
//// a function parameter rather than an ambient environment read, so this module
//// stays pure and testable and works on both the Erlang and JavaScript targets.

import gleam/int
import gleam/list
import gleam/option.{type Option, None, Some}
import gleam/string

pub const api_base_key = "GHA_INDIE_WORKER_API_BASE"

pub const ws_base_key = "GHA_INDIE_WORKER_WS_BASE"

pub const surface_key = "GHA_INDIE_WORKER_SURFACE"

pub const client_timeout_ms_key = "GHA_INDIE_WORKER_CLIENT_TIMEOUT_MS"

pub const shared_auth_base_key = "SHARED_AUTH_BASE"

pub const shared_auth_audience_key = "SHARED_AUTH_AUDIENCE"

pub const default_timeout_ms = 30_000

const max_timeout_ms = 600_000

pub fn keys() -> List(String) {
  [
    api_base_key,
    ws_base_key,
    surface_key,
    client_timeout_ms_key,
    shared_auth_base_key,
    shared_auth_audience_key,
  ]
}

/// Which `indiebuild.dev` host this client is talking to.
pub type Surface {
  App
  User
  Org
  Mobile
  NativeApp
}

pub type ConfigError {
  MissingConfig(key: String)
  InvalidConfig(key: String, reason: String)
}

pub type RuntimeConfig {
  RuntimeConfig(
    api_base: String,
    ws_base: String,
    surface: Surface,
    timeout_ms: Int,
    shared_auth_base: Option(String),
    shared_auth_audience: Option(String),
  )
}

pub fn surfaces() -> List(Surface) {
  [App, User, Org, Mobile, NativeApp]
}

pub fn surface_to_string(surface: Surface) -> String {
  case surface {
    App -> "app"
    User -> "user"
    Org -> "org"
    Mobile -> "mobile"
    NativeApp -> "native_app"
  }
}

pub fn surface_from_string(value: String) -> Result(Surface, Nil) {
  list.find(surfaces(), fn(surface) { surface_to_string(surface) == value })
}

/// Only the B2B surface shows seat and invitation UI.
pub fn is_organization(surface: Surface) -> Bool {
  surface == Org
}

/// Build a configuration from any key lookup.
pub fn load(
  lookup: fn(String) -> Option(String),
) -> Result(RuntimeConfig, ConfigError) {
  case present(lookup(api_base_key)) {
    None -> Error(MissingConfig(api_base_key))
    Some(api_base) ->
      case validate_origin(api_base) {
        False -> Error(InvalidConfig(api_base_key, api_base))
        True -> build(lookup, api_base)
      }
  }
}

fn build(
  lookup: fn(String) -> Option(String),
  api_base: String,
) -> Result(RuntimeConfig, ConfigError) {
  case resolve_ws_base(lookup, api_base) {
    Error(error) -> Error(error)
    Ok(ws_base) ->
      case resolve_surface(lookup) {
        Error(error) -> Error(error)
        Ok(surface) ->
          case resolve_timeout(lookup) {
            Error(error) -> Error(error)
            Ok(timeout_ms) ->
              Ok(RuntimeConfig(
                api_base: api_base,
                ws_base: ws_base,
                surface: surface,
                timeout_ms: timeout_ms,
                shared_auth_base: present(lookup(shared_auth_base_key)),
                shared_auth_audience: present(lookup(shared_auth_audience_key)),
              ))
          }
      }
  }
}

fn resolve_ws_base(
  lookup: fn(String) -> Option(String),
  api_base: String,
) -> Result(String, ConfigError) {
  case present(lookup(ws_base_key)) {
    None -> Ok(derive_ws_base(api_base))
    Some(explicit) ->
      case
        string.starts_with(explicit, "wss://")
        || string.starts_with(explicit, "ws://")
      {
        True -> Ok(explicit)
        False -> Error(InvalidConfig(ws_base_key, explicit))
      }
  }
}

fn resolve_surface(
  lookup: fn(String) -> Option(String),
) -> Result(Surface, ConfigError) {
  case present(lookup(surface_key)) {
    None -> Ok(App)
    Some(raw) ->
      case surface_from_string(raw) {
        Ok(surface) -> Ok(surface)
        Error(_) -> Error(InvalidConfig(surface_key, raw))
      }
  }
}

fn resolve_timeout(
  lookup: fn(String) -> Option(String),
) -> Result(Int, ConfigError) {
  case present(lookup(client_timeout_ms_key)) {
    None -> Ok(default_timeout_ms)
    Some(raw) ->
      case int.parse(raw) {
        Error(_) -> Error(InvalidConfig(client_timeout_ms_key, raw))
        Ok(parsed) ->
          case parsed > 0 && parsed <= max_timeout_ms {
            True -> Ok(parsed)
            False -> Error(InvalidConfig(client_timeout_ms_key, raw))
          }
      }
  }
}

/// Join a path onto the API origin without doubling or dropping the separator.
pub fn endpoint(config: RuntimeConfig, path: String) -> String {
  let base = trim_trailing_slash(config.api_base)
  case string.starts_with(path, "/") {
    True -> base <> path
    False -> base <> "/" <> path
  }
}

fn trim_trailing_slash(value: String) -> String {
  case string.ends_with(value, "/") {
    True -> trim_trailing_slash(string.drop_end(value, 1))
    False -> value
  }
}

fn present(value: Option(String)) -> Option(String) {
  case value {
    None -> None
    Some(raw) ->
      case string.trim(raw) {
        "" -> None
        trimmed -> Some(trimmed)
      }
  }
}

fn validate_origin(value: String) -> Bool {
  let has_scheme =
    string.starts_with(value, "https://") || string.starts_with(value, "http://")
  let clean =
    !string.contains(value, "?")
    && !string.contains(value, "#")
  has_scheme && clean
}

/// `https://` becomes `wss://`; `http://` becomes `ws://`. The security level is
/// preserved rather than upgraded.
fn derive_ws_base(api_base: String) -> String {
  case string.starts_with(api_base, "https://") {
    True -> "wss://" <> string.drop_start(api_base, 8)
    False ->
      case string.starts_with(api_base, "http://") {
        True -> "ws://" <> string.drop_start(api_base, 7)
        False -> api_base
      }
  }
}
