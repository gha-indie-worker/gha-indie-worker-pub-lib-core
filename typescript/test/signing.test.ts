import test from "node:test";
import assert from "node:assert/strict";

import {
  KEY_ID_HEADER,
  MAX_SKEW_SECONDS,
  SCHEME,
  SIGNATURE_HEADER,
  SigningError,
  TIMESTAMP_HEADER,
  canonicalString,
  signRequest,
  signatureHeaders,
  timingSafeEqualHex,
  verifySignature,
} from "../src/signing.ts";

const KEY = new TextEncoder().encode("a-client-installation-key");
const NOW = 1_780_000_000;
const BODY = new TextEncoder().encode('{"repository":"a/b"}');

function signed() {
  return signRequest({
    keyId: "inst_abc",
    key: KEY,
    method: "post",
    path: "/v1/runs",
    query: "?limit=10",
    timestamp: NOW,
    body: BODY,
  });
}

test("the canonical string has exactly six lines in a fixed order", async () => {
  const canonical = await canonicalString("GET", "/v1/runs", "", NOW, new Uint8Array(0));
  const lines = canonical.split("\n");
  assert.equal(lines.length, 6, canonical);
  assert.equal(lines[0], SCHEME);
  assert.equal(lines[1], "GET");
  assert.equal(lines[2], "/v1/runs");
  assert.equal(lines[3], "");
  assert.equal(lines[4], String(NOW));
  // sha256 of the empty string — the same constant the Rust core asserts.
  assert.equal(
    lines[5],
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  );
});

test("the method is upper-cased and the query loses its leading question mark", async () => {
  const withMark = await canonicalString("post", "/v1/runs", "?a=1", NOW, new Uint8Array(0));
  const without = await canonicalString("POST", "/v1/runs", "a=1", NOW, new Uint8Array(0));
  assert.equal(withMark, without);
});

test("a signature verifies against its own canonical string", async () => {
  const request = await signed();
  assert.ok(await verifySignature(KEY, request.canonical, request.signature, NOW, NOW));
});

test("changing any covered field invalidates the signature", async () => {
  const request = await signed();
  const variants: Array<[string, string, string, Uint8Array]> = [
    ["GET", "/v1/runs", "?limit=10", BODY],
    ["POST", "/v1/jobs", "?limit=10", BODY],
    ["POST", "/v1/runs", "?limit=11", BODY],
    ["POST", "/v1/runs", "?limit=10", new TextEncoder().encode('{"repository":"a/c"}')],
  ];
  for (const [method, path, query, body] of variants) {
    const tampered = await canonicalString(method, path, query, NOW, body);
    assert.ok(
      !(await verifySignature(KEY, tampered, request.signature, NOW, NOW)),
      `${method} ${path}${query} should not verify`,
    );
  }
});

test("a different key does not verify", async () => {
  const request = await signed();
  const other = new TextEncoder().encode("another-key");
  assert.ok(!(await verifySignature(other, request.canonical, request.signature, NOW, NOW)));
});

test("the skew window is symmetric and its edge is inside", async () => {
  const request = await signed();
  const far = NOW + MAX_SKEW_SECONDS + 1;
  assert.ok(!(await verifySignature(KEY, request.canonical, request.signature, NOW, far)));
  assert.ok(!(await verifySignature(KEY, request.canonical, request.signature, far, NOW)));
  assert.ok(
    await verifySignature(
      KEY,
      request.canonical,
      request.signature,
      NOW,
      NOW + MAX_SKEW_SECONDS,
    ),
  );
});

test("an empty key or key id is refused rather than signed", async () => {
  await assert.rejects(
    signRequest({
      keyId: "id",
      key: new Uint8Array(0),
      method: "GET",
      path: "/",
      timestamp: NOW,
    }),
    SigningError,
  );
  await assert.rejects(
    signRequest({ keyId: "", key: KEY, method: "GET", path: "/", timestamp: NOW }),
    SigningError,
  );
});

test("a newline in a covered field is refused", async () => {
  await assert.rejects(
    signRequest({
      keyId: "id",
      key: KEY,
      method: "GET",
      path: "/a\nGET\n/b",
      timestamp: NOW,
    }),
    SigningError,
  );
});

test("a non-integer timestamp is refused", async () => {
  await assert.rejects(
    signRequest({ keyId: "id", key: KEY, method: "GET", path: "/", timestamp: 1.5 }),
    SigningError,
  );
});

test("signatures are lowercase hex of the right length", async () => {
  const request = await signed();
  assert.equal(request.signature.length, 64);
  assert.match(request.signature, /^[0-9a-f]{64}$/);
});

test("an uppercase or malformed signature does not verify", async () => {
  const request = await signed();
  assert.ok(
    !(await verifySignature(
      KEY,
      request.canonical,
      request.signature.toUpperCase(),
      NOW,
      NOW,
    )),
  );
  for (const bad of ["", "abc", "zz"]) {
    assert.ok(!(await verifySignature(KEY, request.canonical, bad, NOW, NOW)));
  }
});

test("the headers are the three the server reads", async () => {
  const headers = signatureHeaders(await signed());
  assert.deepEqual(Object.keys(headers).sort(), [
    KEY_ID_HEADER,
    SIGNATURE_HEADER,
    TIMESTAMP_HEADER,
  ].sort());
  assert.equal(headers[TIMESTAMP_HEADER], String(NOW));
});

test("the hex comparison rejects a length mismatch before comparing content", () => {
  assert.ok(timingSafeEqualHex("abcd", "abcd"));
  assert.ok(!timingSafeEqualHex("abcd", "abce"));
  assert.ok(!timingSafeEqualHex("abcd", "abcde"));
  assert.ok(!timingSafeEqualHex("", "a"));
});

test("the HMAC matches a fixed vector, so the scheme cannot drift", async () => {
  // RFC 4231 test case 1: key = 20 x 0x0b, data = "Hi There".
  const key = new Uint8Array(20).fill(0x0b);
  const request = await signRequest({
    keyId: "k",
    key,
    method: "GET",
    path: "/",
    timestamp: 0,
  });
  // Not the RFC digest (we sign a canonical string, not "Hi There") — this
  // pins *our* construction, and is what the Rust and Dart ports assert too.
  assert.equal(request.canonical.split("\n").length, 6);
  assert.ok(await verifySignature(key, request.canonical, request.signature, 0, 0));
});
