import test from "node:test";
import assert from "node:assert/strict";

import {
  API_BASE,
  CLIENT_TIMEOUT_MS,
  ConfigError,
  DEFAULT_TIMEOUT_MS,
  KEYS,
  SURFACE,
  SURFACES,
  WS_BASE,
  endpoint,
  isOrganizationSurface,
  isSurface,
  loadConfig,
} from "../src/config.ts";
import type { Lookup } from "../src/config.ts";

function lookupFrom(pairs: Record<string, string>): Lookup {
  return (key) => pairs[key];
}

test("a minimal configuration derives the websocket origin", () => {
  const config = loadConfig(lookupFrom({ [API_BASE]: "https://api.indiebuild.dev" }));
  assert.equal(config.wsBase, "wss://api.indiebuild.dev");
  assert.equal(config.surface, "app");
  assert.equal(config.timeoutMs, DEFAULT_TIMEOUT_MS);
  assert.equal(config.sharedAuthBase, null);
});

test("a plaintext api base derives a plaintext socket and never upgrades", () => {
  const config = loadConfig(lookupFrom({ [API_BASE]: "http://127.0.0.1:8080" }));
  assert.equal(config.wsBase, "ws://127.0.0.1:8080");
});

test("the api base is required and whitespace does not count as present", () => {
  assert.throws(() => loadConfig(lookupFrom({})), ConfigError);
  assert.throws(() => loadConfig(lookupFrom({ [API_BASE]: "   " })), ConfigError);
});

test("a base without a scheme, or carrying a query, is rejected", () => {
  for (const bad of [
    "api.indiebuild.dev",
    "ftp://api.indiebuild.dev",
    "https://api.indiebuild.dev?tenant=1",
    "https://api.indiebuild.dev#x",
  ]) {
    assert.throws(() => loadConfig(lookupFrom({ [API_BASE]: bad })), ConfigError, bad);
  }
});

test("an explicit websocket base must use a websocket scheme", () => {
  assert.throws(
    () =>
      loadConfig(
        lookupFrom({
          [API_BASE]: "https://api.indiebuild.dev",
          [WS_BASE]: "https://api.indiebuild.dev",
        }),
      ),
    ConfigError,
  );
  const config = loadConfig(
    lookupFrom({
      [API_BASE]: "https://api.indiebuild.dev",
      [WS_BASE]: "wss://ws.indiebuild.dev",
    }),
  );
  assert.equal(config.wsBase, "wss://ws.indiebuild.dev");
});

test("every surface is recognised and unknown surfaces are rejected", () => {
  for (const surface of SURFACES) {
    assert.ok(isSurface(surface));
    const config = loadConfig(
      lookupFrom({ [API_BASE]: "https://api.indiebuild.dev", [SURFACE]: surface }),
    );
    assert.equal(config.surface, surface);
  }
  assert.ok(!isSurface("admin"));
  assert.throws(
    () =>
      loadConfig(
        lookupFrom({ [API_BASE]: "https://api.indiebuild.dev", [SURFACE]: "admin" }),
      ),
    ConfigError,
  );
});

test("only the org surface shows seat ui", () => {
  assert.deepEqual(SURFACES.filter(isOrganizationSurface), ["org"]);
});

test("a zero, absurd or non-integer timeout is rejected", () => {
  for (const bad of ["0", "600001", "not-a-number", "-5", "1e3", "12.5"]) {
    assert.throws(
      () =>
        loadConfig(
          lookupFrom({
            [API_BASE]: "https://api.indiebuild.dev",
            [CLIENT_TIMEOUT_MS]: bad,
          }),
        ),
      ConfigError,
      bad,
    );
  }
  const config = loadConfig(
    lookupFrom({ [API_BASE]: "https://api.indiebuild.dev", [CLIENT_TIMEOUT_MS]: "1500" }),
  );
  assert.equal(config.timeoutMs, 1500);
});

test("endpoint joins without doubling the separator", () => {
  const config = loadConfig(lookupFrom({ [API_BASE]: "https://api.indiebuild.dev/" }));
  assert.equal(endpoint(config, "/v1/runs"), "https://api.indiebuild.dev/v1/runs");
  assert.equal(endpoint(config, "v1/runs"), "https://api.indiebuild.dev/v1/runs");
});

test("the declared key list matches the keys the module reads", () => {
  assert.equal(KEYS.length, 6);
  assert.ok(KEYS.includes(API_BASE));
  assert.ok(KEYS.includes(CLIENT_TIMEOUT_MS));
  assert.equal(new Set(KEYS).size, KEYS.length, "keys must be unique");
});
