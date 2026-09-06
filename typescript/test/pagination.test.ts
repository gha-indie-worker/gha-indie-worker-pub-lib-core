import test from "node:test";
import assert from "node:assert/strict";

import {
  DEFAULT_PAGE_LIMIT,
  MAX_PAGE_LIMIT,
  PaginationError,
  base64UrlDecode,
  base64UrlEncode,
  cursor,
  decodeCursor,
  encodeCursor,
  isLastPage,
  pageQuery,
  pageRequest,
} from "../src/pagination.ts";

const encoder = new TextEncoder();

test("base64url round trips every length class", () => {
  for (const text of ["f", "fo", "foo", "foob", "fooba", "foobar"]) {
    const bytes = encoder.encode(text);
    const encoded = base64UrlEncode(bytes);
    assert.deepEqual(base64UrlDecode(encoded), bytes, text);
  }
});

test("base64url uses the url-safe alphabet and no padding", () => {
  const encoded = base64UrlEncode(Uint8Array.from([0xfb, 0xff, 0xbf]));
  assert.ok(!encoded.includes("="));
  assert.ok(!encoded.includes("+"));
  assert.ok(!encoded.includes("/"));
});

test("base64url rejects padding, whitespace and foreign characters", () => {
  for (const bad of ["Zm9v=", "Zm 9v", "Zm+v", "Zm/v", ""]) {
    assert.throws(() => base64UrlDecode(bad), PaginationError, JSON.stringify(bad));
  }
});

test("base64url matches the encoding the Rust core produces", () => {
  // Fixed vectors, so the two implementations cannot drift apart silently.
  assert.equal(base64UrlEncode(encoder.encode("foobar")), "Zm9vYmFy");
  assert.equal(base64UrlEncode(encoder.encode("f")), "Zg");
  assert.equal(base64UrlEncode(encoder.encode("fo")), "Zm8");
});

test("a cursor round trips through its wire form", () => {
  const value = cursor("2026-09-05T12-00-00Z", "run-42");
  const decoded = decodeCursor(encodeCursor(value));
  assert.deepEqual(decoded, value);
});

test("a cursor refuses fields that would break its own encoding", () => {
  assert.throws(() => cursor("a:b", "id"), PaginationError);
  assert.throws(() => cursor("a", "i:d"), PaginationError);
  assert.throws(() => cursor("", "id"), PaginationError);
});

test("a hand-written cursor does not decode", () => {
  assert.throws(() => decodeCursor("not-base64!!"), PaginationError);
  assert.throws(
    () => decodeCursor(base64UrlEncode(encoder.encode("v2:key:id"))),
    PaginationError,
  );
  assert.throws(
    () => decodeCursor(base64UrlEncode(encoder.encode("onlyonepart"))),
    PaginationError,
  );
});

test("page requests are bounded at both ends", () => {
  assert.throws(() => pageRequest(0), PaginationError);
  assert.throws(() => pageRequest(MAX_PAGE_LIMIT + 1), PaginationError);
  assert.throws(() => pageRequest(1.5), PaginationError);
  assert.equal(pageRequest(1).limit, 1);
  assert.equal(pageRequest().limit, DEFAULT_PAGE_LIMIT);
  assert.ok(DEFAULT_PAGE_LIMIT <= MAX_PAGE_LIMIT);
});

test("the query string carries the cursor verbatim", () => {
  const value = cursor("2026", "abc");
  const request = pageRequest(10, value);
  assert.equal(pageQuery(request), `limit=10&after=${encodeCursor(value)}`);
  assert.ok(!pageQuery(pageRequest(10)).includes("after"));
});

test("a page knows whether it is the last one", () => {
  assert.ok(!isLastPage({ items: [1, 2], next: cursor("k", "i") }));
  assert.ok(isLastPage({ items: [], next: null }));
});
