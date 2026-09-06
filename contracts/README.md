# contracts/ — a pointer, not a copy

The contracts this package implements are **owned by
[`gha-indie-worker-interfaces`](https://github.com/gha-indie-worker/gha-indie-worker-interfaces)**.
Nothing in this repository is an authority. This directory exists so the pointer
is written down rather than remembered.

## Where the authorities live

`gha-indie-worker-interfaces` carries two independent, human-authored
authorities and generates from both (the ores-contracts toolkit):

```text
contracts/typespec/main.tsp        ──parse──▶ IR_T ──emit──▶ artifacts
contracts/json-schema/*.json       ──parse──▶ IR_J ──emit──▶ artifacts
                                                 ║ byte parity ║
```

Neither is generated from the other. A discrepancy is a finding with a stable
fingerprint, and `generate` refuses to write `generated/` until a human changes
an authored source.

## What this package consumes

| lane | consumes from `-interfaces` | lands in |
|---|---|---|
| rust | `generated/rust/types.rs` | `rust/src/resources.rs` |
| typescript | `generated/typescript/types.d.ts` | `typescript/src/resources.ts` |
| dart | `generated/dart/models.dart` | `dart/lib/src/resources.dart` |
| gleam | *(no Gleam emitter yet — see below)* | `gleam/src/.../resources.gleam` |

Today each of those four files is **hand-written**, because `-interfaces` has
not yet published a `generated/` tree for these resources. That is a deliberate,
temporary state, and `expected-resources.json` in this directory records the
exact field names and types the four ports agree on, so the swap to generated
types is a mechanical diff rather than an archaeological dig.

`scripts/check-contract-parity.sh` compares the four hand-written ports against
`expected-resources.json` on every CI run, which is what stops them drifting
apart while they are still hand-written.

## Adding a field

1. Add it to **both** authorities in `-interfaces` and run
   `npx ores-contracts check` there.
2. Add it to `expected-resources.json` here.
3. Add it to all four ports. CI fails until all four match.

The order matters: the contract changes first, and the clients follow. A field
that exists in a client and not in the contract is a field the server does not
promise.

## What is deliberately not here

* **No `.tsp` or `.schema.json` files.** Copying an authority is how two
  authorities are created.
* **No `generated/` directory.** Generated artifacts are committed in
  `-interfaces`, and consumed from there through zed-pkg.
* **No SQL, no SeaORM, no Diesel.** Those artifacts go to
  `gha-indie-worker-orm-core`, which is backend-only.
