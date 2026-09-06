# GHA Indie Worker — pub-lib-core

## Parent / root agent contract

The fleet-wide parent lives at:

- GitHub: https://github.com/oresoftware/my-ai/AGENTS.md
- Canonical disk path: `~/codes/oresoftware/my-ai/AGENTS.md`
- `~/codes/AGENTS.md` is a symlink to `~/codes/oresoftware/my-ai/AGENTS.md` (installed by `~/codes/oresoftware/my-ai/setup-final.sh`)

When this file and the parent disagree: follow this file for this repository's
local layout and tools; follow the parent for org-wide conventions and the
functional programming rules.

Canonical `pub-lib-core` repository for [`gha-indie-worker`](https://github.com/gha-indie-worker).

- Internal runtimes: Rust, TypeScript, Dart. Gleam is a first-class client language here too.
- Contracts: TypeSpec + JSON Schema in `gha-indie-worker-interfaces`.
- Auth: github.com/shared-auth.
- Sync: github.com/opto-sync.
- Telemetry: github.com/ores-otel.
- Flags: github.com/flags-2-env.
- Packages: github.com/zed-pkg.
- Never use React/JSX or webviews.
- Resolve git conflicts semantically; never rebase, stash, or reset.

## Hard rules for this repository

1. **PUBLIC and client-side.** Everything here ships to a device the org does
   not control. Nothing may assume it does not.
2. **No transport.** No HTTP client, no websocket client, no retry loop. This
   package says what to send; the host sends it. A dependency on `reqwest`,
   `http`, `dio` or `httpx` is a bug, not an improvement.
3. **No secrets that are not the client's own.** The signing key here is the
   per-installation credential a client holds. `SHARED_AUTH_INTROSPECT_SECRET`
   and anything like it never appears in this repository.
4. **No database.** No SeaORM, no Diesel, no table name, no SQL string. That is
   `gha-indie-worker-orm-core`, and it is backend-only.
5. **Four ports, one contract.** A change to a resource, a config key, an
   onboarding state or the signing scheme lands in **all four** ports and in
   `contracts/expected-resources.json` in the same commit.
   `scripts/check-contract-parity.sh` fails otherwise, and it needs no toolchain.
6. **The Rust crate must stay `no_std`-capable.** `cargo check
   --no-default-features` and the `wasm32-unknown-unknown` target are gates. A
   dependency that is std-only does not belong here.
7. **The TypeScript package must stay zero-dependency and build-step-free.**
   Erasable syntax only — no `enum`, no `namespace`, no parameter properties —
   so `node --test` runs the sources directly. CI asserts `dependencies` is
   empty.
8. **The Dart package must not depend on Flutter.** Widgets belong in
   `gha-indie-worker-flutter`. `streams.dart` composes with RxDart; it does not
   import it.
9. **`contracts/` is a pointer.** No `.tsp` and no `.schema.json` files here —
   copying an authority is how two authorities are created.

## Layout

```
rust/        crate, no_std + alloc by default, `std` feature adds env reading
typescript/  ESM, zero deps, node --test with type stripping
dart/        package gha_indie_worker_pub_lib_core, dart:async only
gleam/       package gha_indie_worker_pub_lib_core, total functions
contracts/   README (pointer to -interfaces) + expected-resources.json
scripts/     check-contract-parity.sh
```

## Code style and coding patterns

remember to modularize the rust, typescript and dart - not everything belongs in
main.rs, main.ts and main.dart; also follow functional coding principles - fewer
side-effects (use pure functions more), more immutability (immutable variables);
but for stateful apps like the client or stateful servers like websockets or tcp
connections, sometimes classes and oop make more sense than functional
programming perse, but we can still adhere to functional programming more than
usual. Favor exhaustive pattern matching and use formal methods checking too.
Favor composability and re-use, so basically create more utility functions and
routines for shared use. You can follow a medium level of D.R.Y. (don't repeat
yourself) - in other words you can repeat yourself at medium amount (not too much
not too little). Some chaining is totally fine, so either method-chaining
(immutable sometimes although with classes can be mutable too for performance),
and chaining via the pipe operator is ok in languages like gleamlang.

Functional programming is mostly the following:

+ explicit inputs
+ explicit outputs
+ immutable values
+ pure transformations
+ typed errors
+ explicit state transitions
+ composition
+ effects pushed outward
+ illegal states excluded by types
