# gha-indie-worker-pub-lib-core

The **public** client-side core for [GHA Indie Worker](https://github.com/gha-indie-worker),
in four languages: Rust (wasm32-capable), TypeScript, Dart and Gleam.

## `-pub-lib-core` versus `-lib-core` versus `-orm-core`

These are three repositories with three different audiences, and the boundary is
the point of having three:

| repo | who imports it | may contain |
|---|---|---|
| **`-pub-lib-core`** (this one) | **browser, desktop, mobile** — anything a user runs | typed resources, request signing with a *client's own* credential, pagination, the onboarding state machine, runtime config |
| `-lib-core` | client **and** server — the shared middle | shared runtime config, JSON Schema validators, the lock/lease helpers from `ores-locks-and-leases` |
| `-orm-core` | **servers only** | SeaORM entities, query builders, `schema.sql` |

Read the table as a one-way street. Anything in `-pub-lib-core` is, by
construction, safe to ship to a device you do not control. That is why this
repository contains:

* **no HTTP client.** The transport is the host's — `fetch` in a browser,
  whatever the desktop shell prefers, `dart:io` under Flutter. This package
  describes *what* to send; the host decides *how*, and gets to keep its own
  retry, cancellation and offline behaviour;
* **no database access, no table names, no SQL.** Those are in `-orm-core`,
  which links `sqlx-postgres` through SeaORM and is never compiled for a client;
* **no server secrets.** [`signing`](rust/src/signing.rs) signs with the
  per-installation credential a client legitimately holds. The shared-auth
  introspection secret never leaves a server;
* **no framework.** No React, no JSX, no widget library. RxDart and RxJS are
  *allowed* for a consumer, and `dart/lib/src/streams.dart` is written to
  compose with RxDart — but neither is a dependency here.

`-lib-core` may depend on this package. This package depends on nothing in the
org except `-interfaces` contracts.

## The four ports are one contract

Every port implements the same thing, and CI proves it before any of them
compile:

| | Rust | TypeScript | Dart | Gleam |
|---|---|---|---|---|
| runtime config (flags-2-env keys) | ✓ | ✓ | ✓ | ✓ |
| cursor pagination (`base64url(v1:…)`) | ✓ | ✓ | ✓ | — |
| `GIW1` request signing | ✓ | ✓ | ✓ | — |
| onboarding state machines | ✓ | ✓ | ✓ | ✓ |
| resource types | ✓ | ✓ | ✓ | ✓ |
| optimistic-UI wrapper | ✓ | ✓ | ✓ | — |

`scripts/check-contract-parity.sh` reads `contracts/expected-resources.json` and
fails if a field, a config key, an onboarding state or a signing header exists in
one port and not the others. It needs nothing but `python3`, so it runs even on a
machine with no Dart or Gleam toolchain — which is exactly when drift creeps in.

The `GIW1` canonical string is asserted against the same fixed vectors in all
three signing ports (including the SHA-256 of the empty string), so the three
cannot quietly diverge.

## Contracts

`contracts/` is a **pointer**, not a copy — see
[`contracts/README.md`](contracts/README.md). The authorities are the TypeSpec
and JSON Schema files in `gha-indie-worker-interfaces`, and the resource types
here are hand-written only until that repository publishes a `generated/` tree
for them. `contracts/expected-resources.json` records the field names so the swap
is a mechanical diff.

## Layout

```
rust/        crate gha-indie-worker-pub-lib-core — no_std + alloc by default
typescript/  ESM, zero runtime dependencies, node --test with type stripping
dart/        package gha_indie_worker_pub_lib_core — no Flutter dependency
gleam/       package gha_indie_worker_pub_lib_core — pure, total functions
contracts/   pointer to -interfaces + the field-level parity record
scripts/     check-contract-parity.sh
```

### Rust

`default-features = false` gives `no_std` + `alloc`, which is what makes the
crate usable from a `wasm32-unknown-unknown` island without a runtime. The `std`
feature adds exactly one capability: reading configuration from process
environment variables. CI builds both, plus the wasm32 target.

### TypeScript

Node 22.6+ strips the types, so `node --test` runs the `.ts` sources with **no
build step and no dependencies** — not even a test framework. The source is
written in erasable syntax only (no `enum`, no `namespace`, no parameter
properties) so that stays true.

### Dart

No Flutter dependency: the package is used by `gha-indie-worker-flutter`, by the
CLI's Dart surface, and by plain Dart tooling. `streams.dart` is `dart:async`
only and composes with RxDart rather than requiring it.

### Gleam

Total functions, `Result` everywhere, no exceptions. The state machines and the
config parser are the parts worth having in a fourth language, because they are
the parts where a client can be *wrong* rather than merely broken.

## Gates

```sh
scripts/check-contract-parity.sh                       # all four ports, no toolchain
cd rust        && cargo fmt --all -- --check \
                  && cargo clippy --all-targets --all-features -- -D warnings \
                  && cargo test --all-targets --all-features \
                  && cargo check --no-default-features \
                  && cargo check --target wasm32-unknown-unknown --no-default-features
cd typescript  && node --test "test/**/*.test.ts"
cd dart        && dart analyze --fatal-infos && dart test    # skipped if dart is absent
cd gleam       && gleam check && gleam test                  # skipped if gleam is absent
```
