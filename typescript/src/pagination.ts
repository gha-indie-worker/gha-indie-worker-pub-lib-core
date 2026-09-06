/**
 * Cursor pagination — the same wire format the Rust and Dart cores produce.
 *
 * An offset is wrong for a feed that grows while you read it. The cursor
 * encodes the position of the last row returned, base64url of
 * `v1:<sort-key>:<id>`, so the next page continues from a fixed point.
 */

export const MAX_PAGE_LIMIT = 100;
export const DEFAULT_PAGE_LIMIT = 25;

const CURSOR_VERSION = "v1";

export class PaginationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "PaginationError";
  }
}

export interface Cursor {
  readonly sortKey: string;
  readonly id: string;
}

export interface PageRequest {
  readonly limit: number;
  readonly after: Cursor | null;
}

export interface Page<T> {
  readonly items: readonly T[];
  readonly next: Cursor | null;
}

/** Build a cursor. A `:` in either field would make the encoding ambiguous. */
export function cursor(sortKey: string, id: string): Cursor {
  if (sortKey.length === 0 || id.length === 0) {
    throw new PaginationError("a cursor needs both a sort key and an id");
  }
  if (sortKey.includes(":") || id.includes(":")) {
    throw new PaginationError("a cursor field may not contain ':'");
  }
  return { sortKey, id };
}

export function encodeCursor(value: Cursor): string {
  return base64UrlEncode(
    new TextEncoder().encode(`${CURSOR_VERSION}:${value.sortKey}:${value.id}`),
  );
}

/** Parse a cursor this API issued. Anything else throws rather than guessing. */
export function decodeCursor(encoded: string): Cursor {
  const text = new TextDecoder("utf-8", { fatal: true }).decode(
    base64UrlDecode(encoded),
  );
  // `split` with a limit drops the tail; slice the remainder back on instead so
  // a legitimate id can never be silently truncated.
  const parts = text.split(":");
  if (parts.length < 3) {
    throw new PaginationError("cursor is not a cursor this API issued");
  }
  const [version, sortKey, ...rest] = parts;
  if (version !== CURSOR_VERSION) {
    throw new PaginationError("cursor is not a cursor this API issued");
  }
  return cursor(sortKey ?? "", rest.join(":"));
}

export function pageRequest(
  limit: number = DEFAULT_PAGE_LIMIT,
  after: Cursor | null = null,
): PageRequest {
  if (!Number.isInteger(limit) || limit < 1 || limit > MAX_PAGE_LIMIT) {
    throw new PaginationError(`page limit ${limit} is outside 1..=${MAX_PAGE_LIMIT}`);
  }
  return { limit, after };
}

/** Query-string fragment, with no leading `?` so a caller can compose it. */
export function pageQuery(request: PageRequest): string {
  const parts = [`limit=${request.limit}`];
  if (request.after !== null) {
    parts.push(`after=${encodeCursor(request.after)}`);
  }
  return parts.join("&");
}

export function isLastPage<T>(page: Page<T>): boolean {
  return page.next === null;
}

const ALPHABET =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/** Unpadded base64url (RFC 4648 §5). No dependency, works in every runtime. */
export function base64UrlEncode(bytes: Uint8Array): string {
  let out = "";
  for (let index = 0; index < bytes.length; index += 3) {
    const b0 = bytes[index] ?? 0;
    const b1 = bytes[index + 1];
    const b2 = bytes[index + 2];
    const triple = (b0 << 16) | ((b1 ?? 0) << 8) | (b2 ?? 0);
    out += ALPHABET[(triple >> 18) & 0x3f];
    out += ALPHABET[(triple >> 12) & 0x3f];
    if (b1 !== undefined) {
      out += ALPHABET[(triple >> 6) & 0x3f];
    }
    if (b2 !== undefined) {
      out += ALPHABET[triple & 0x3f];
    }
  }
  return out;
}

/** Rejects padding, whitespace and foreign characters rather than skipping them. */
export function base64UrlDecode(input: string): Uint8Array {
  if (input.length === 0) {
    throw new PaginationError("empty base64url input");
  }
  const out: number[] = [];
  let buffer = 0;
  let bits = 0;
  for (const character of input) {
    const value = ALPHABET.indexOf(character);
    if (value < 0) {
      throw new PaginationError(`invalid base64url character ${JSON.stringify(character)}`);
    }
    buffer = (buffer << 6) | value;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      out.push((buffer >> bits) & 0xff);
    }
  }
  if (bits >= 6 || (buffer & ((1 << bits) - 1)) !== 0) {
    throw new PaginationError("truncated base64url input");
  }
  return Uint8Array.from(out);
}
