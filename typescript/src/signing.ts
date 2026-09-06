/**
 * Request signing — byte-identical to the Rust core's `GIW1` scheme.
 *
 * Uses Web Crypto (`globalThis.crypto.subtle`), which is present in browsers,
 * Deno, Cloudflare Workers and Node 18+. That is the only reason this module
 * has no dependencies and no Node-specific import: the same file runs in a
 * service worker and in `node --test`.
 *
 * The canonical string is six newline-separated lines:
 *
 *     GIW1
 *     <METHOD uppercased>
 *     <path, no query>
 *     <query string, or empty>
 *     <unix seconds>
 *     <lowercase hex sha256 of the body>
 */

export const SCHEME = "GIW1" as const;
export const MAX_SKEW_SECONDS = 300;

export const SIGNATURE_HEADER = "x-giw-signature" as const;
export const TIMESTAMP_HEADER = "x-giw-timestamp" as const;
export const KEY_ID_HEADER = "x-giw-key-id" as const;

export class SigningError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SigningError";
  }
}

export interface SignedRequest {
  readonly keyId: string;
  readonly timestamp: number;
  /** Lowercase hex HMAC-SHA256. */
  readonly signature: string;
  /** Exactly what was signed. Contains no secret. */
  readonly canonical: string;
}

export interface SignInput {
  readonly keyId: string;
  readonly key: Uint8Array;
  readonly method: string;
  readonly path: string;
  readonly query?: string;
  readonly timestamp: number;
  readonly body?: Uint8Array;
}

const EMPTY = new Uint8Array(0);

/**
 * Build the canonical string. Exported because the server implements the same
 * function and the two must agree byte for byte.
 */
export async function canonicalString(
  method: string,
  path: string,
  query: string,
  timestamp: number,
  body: Uint8Array,
): Promise<string> {
  const digest = await sha256Hex(body);
  const normalizedQuery = query.startsWith("?") ? query.slice(1) : query;
  return [
    SCHEME,
    method.toUpperCase(),
    path,
    normalizedQuery,
    String(timestamp),
    digest,
  ].join("\n");
}

export async function signRequest(input: SignInput): Promise<SignedRequest> {
  if (input.key.length === 0 || input.keyId.length === 0) {
    throw new SigningError("signing key and key id must both be non-empty");
  }
  if (!Number.isInteger(input.timestamp)) {
    throw new SigningError("timestamp must be integer unix seconds");
  }
  const query = input.query ?? "";
  for (const field of [input.method, input.path, query, input.keyId]) {
    // A newline in a covered field would let one signature stand for two
    // different requests.
    if (field.includes("\n") || field.includes("\r")) {
      throw new SigningError("a signed field may not contain a newline");
    }
  }
  const canonical = await canonicalString(
    input.method,
    input.path,
    query,
    input.timestamp,
    input.body ?? EMPTY,
  );
  const signature = toHex(await hmacSha256(input.key, canonical));
  return { keyId: input.keyId, timestamp: input.timestamp, signature, canonical };
}

/** The three headers a host transport attaches. */
export function signatureHeaders(signed: SignedRequest): Record<string, string> {
  return {
    [KEY_ID_HEADER]: signed.keyId,
    [TIMESTAMP_HEADER]: String(signed.timestamp),
    [SIGNATURE_HEADER]: signed.signature,
  };
}

/**
 * Verify a signature. The skew window is checked first — an expired signature
 * is not worth a comparison — and the comparison itself is time-independent.
 */
export async function verifySignature(
  key: Uint8Array,
  canonical: string,
  presentedHex: string,
  timestamp: number,
  now: number,
): Promise<boolean> {
  if (Math.abs(now - timestamp) > MAX_SKEW_SECONDS) {
    return false;
  }
  if (key.length === 0) {
    return false;
  }
  const expected = toHex(await hmacSha256(key, canonical));
  return timingSafeEqualHex(expected, presentedHex);
}

/**
 * Compare two hex strings without leaking where they differ.
 *
 * Length is compared first and non-secretly: the expected length is a constant
 * of the scheme, so it reveals nothing an attacker does not already know.
 */
export function timingSafeEqualHex(expected: string, presented: string): boolean {
  if (expected.length !== presented.length) {
    return false;
  }
  let difference = 0;
  for (let index = 0; index < expected.length; index += 1) {
    difference |= expected.charCodeAt(index) ^ presented.charCodeAt(index);
  }
  return difference === 0;
}

async function hmacSha256(key: Uint8Array, message: string): Promise<Uint8Array> {
  const imported = await crypto.subtle.importKey(
    "raw",
    toArrayBuffer(key),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const signature = await crypto.subtle.sign(
    "HMAC",
    imported,
    new TextEncoder().encode(message),
  );
  return new Uint8Array(signature);
}

async function sha256Hex(bytes: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", toArrayBuffer(bytes));
  return toHex(new Uint8Array(digest));
}

/** Web Crypto wants an ArrayBuffer; a Uint8Array view may be a slice of one. */
function toArrayBuffer(bytes: Uint8Array): ArrayBuffer {
  const copy = new Uint8Array(bytes.length);
  copy.set(bytes);
  return copy.buffer;
}

export function toHex(bytes: Uint8Array): string {
  let out = "";
  for (const byte of bytes) {
    out += byte.toString(16).padStart(2, "0");
  }
  return out;
}
