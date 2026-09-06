#!/usr/bin/env bash
# check-contract-parity.sh — the four ports must agree on one contract.
#
# `contracts/expected-resources.json` is the record of what the Rust,
# TypeScript, Dart and Gleam ports promise while the resource types are still
# hand-written (see `contracts/README.md`). This script proves the four have not
# drifted from it — offline, with no toolchain beyond python3, so it runs even
# when a language's SDK is absent from the machine.
#
# It is a *presence* check, not a type check: each language's own compiler is
# what checks types. What it catches is a field added to one port and forgotten
# in the other three, which is the failure mode that actually happens.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$here"

python3 - <<'PY'
import json
import pathlib
import re
import sys

root = pathlib.Path.cwd()
contract = json.loads((root / "contracts" / "expected-resources.json").read_text())

# One source file per language that must mention every field of every resource.
PORTS = {
    "rust": root / "rust/src/resources.rs",
    "typescript": root / "typescript/src/resources.ts",
    "dart": root / "dart/lib/src/resources.dart",
    "gleam": root / "gleam/src/gha_indie_worker_pub_lib_core/resources.gleam",
}

CASE = {
    "rust": lambda name: re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower(),
    "gleam": lambda name: re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower(),
    "typescript": lambda name: name,
    "dart": lambda name: name,
}

failures = []

for language, path in PORTS.items():
    if not path.exists():
        failures.append(f"{language}: {path.relative_to(root)} is missing")
        continue
    text = path.read_text()
    for resource, definition in contract["resources"].items():
        for field in definition["fields"]:
            expected = CASE[language](field)
            if expected not in text:
                failures.append(
                    f"{language}: {resource}.{field} (expected identifier "
                    f"{expected!r}) is not in {path.relative_to(root)}"
                )

# The runtime configuration keys are literal strings and must appear verbatim in
# every port, whatever the language's naming convention.
CONFIG_PORTS = {
    "rust": root / "rust/src/config.rs",
    "typescript": root / "typescript/src/config.ts",
    "dart": root / "dart/lib/src/config.dart",
    "gleam": root / "gleam/src/gha_indie_worker_pub_lib_core/config.gleam",
}
for language, path in CONFIG_PORTS.items():
    if not path.exists():
        failures.append(f"{language}: {path.relative_to(root)} is missing")
        continue
    text = path.read_text()
    for key in contract["runtimeConfigKeys"]:
        if key not in text:
            failures.append(f"{language}: config key {key} is missing from {path.relative_to(root)}")

# The onboarding wire strings are literal too.
MACHINE_PORTS = {
    "rust": root / "rust/src/onboarding.rs",
    "typescript": root / "typescript/src/onboarding.ts",
    "dart": root / "dart/lib/src/onboarding.dart",
    "gleam": root / "gleam/src/gha_indie_worker_pub_lib_core/onboarding.gleam",
}
for language, path in MACHINE_PORTS.items():
    if not path.exists():
        failures.append(f"{language}: {path.relative_to(root)} is missing")
        continue
    text = path.read_text()
    for machine, states in contract["stateMachines"].items():
        for state in states:
            if f'"{state}"' not in text and f"'{state}'" not in text:
                failures.append(
                    f"{language}: {machine} state {state!r} is missing from "
                    f"{path.relative_to(root)}"
                )

# The signing scheme is one constant and one skew window, in all four.
SIGNING_PORTS = {
    "rust": root / "rust/src/signing.rs",
    "typescript": root / "typescript/src/signing.ts",
    "dart": root / "dart/lib/src/signing.dart",
}
scheme = contract["signing"]["scheme"]
skew = str(contract["signing"]["maxSkewSeconds"])
for language, path in SIGNING_PORTS.items():
    if not path.exists():
        failures.append(f"{language}: {path.relative_to(root)} is missing")
        continue
    text = path.read_text()
    if scheme not in text:
        failures.append(f"{language}: signing scheme {scheme} is missing")
    if skew not in text:
        failures.append(f"{language}: max skew {skew}s is missing")
    for header in contract["signing"]["headers"]:
        if header not in text:
            failures.append(f"{language}: signing header {header} is missing")

if failures:
    print("[contract] the ports have drifted from contracts/expected-resources.json:", file=sys.stderr)
    for failure in failures:
        print(f"  - {failure}", file=sys.stderr)
    sys.exit(1)

resources = len(contract["resources"])
fields = sum(len(d["fields"]) for d in contract["resources"].values())
print(
    f"[contract] ok: {resources} resources / {fields} fields, "
    f"{len(contract['runtimeConfigKeys'])} config keys and the GIW1 signing scheme "
    "agree across every port"
)
PY
