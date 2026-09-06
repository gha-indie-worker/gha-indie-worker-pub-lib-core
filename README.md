# gha-indie-worker-pub-lib-core

The **public** client core for GHA Indie Worker: the small amount of logic that every external SDK
needs to get right, written once, in one place, with tests.

This is not `gha-indie-worker-lib-core`. That crate is server-side and shared across the four
servers; this one ships to **browsers, desktops and phones**, so it depends on nothing (std only),
knows nothing about databases or auth issuers, and never sees a secret.

## What is here, and why it is here rather than in each SDK

| module | why it must not be reimplemented per language |
|---|---|
| `endpoint` | Path construction with per-segment percent-encoding and identifier validation. Five SDKs hand-concatenating paths is five chances to forget escaping — and a run id containing `../` turns a request for one resource into a request for another, identically in every language. |
| `backoff` | Retry policy. Full jitter rather than plain exponential, because clients that fail together and retry together are what keeps a recovering server down. `Retry-After` always wins over our own guess, and an absurd one is refused rather than obeyed. |

## What is deliberately *not* here

No HTTP client, no TLS, no async runtime, no JSON codec. Those differ per platform and per host
application, and pinning them here would force a Flutter app and a browser bundle to agree on
things they have no reason to agree on. This crate decides *policy*; the SDK performs the *effect*.

## Consuming it

Through zed-pkg, like everything else in the org:

```toml
[dependencies]
"gha-indie-worker/gha-indie-worker-pub-lib-core" = "^0.1.0"
```

The TypeScript, Dart and Gleam ports live in `gha-indie-worker-clients` and are generated against
the same contracts in `gha-indie-worker-interfaces`; the tests in this crate are the reference
behaviour those ports must reproduce.

## Verification

```sh
cargo test          # 14 tests, no network, no fixtures
```

The tests are written as adversarial cases rather than happy paths: traversal and query injection
through a path segment, a `Retry-After` a hostile server could use to park a client, jitter that is
actually wide, and a NaN reaching the delay calculation.
