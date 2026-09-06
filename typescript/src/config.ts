/**
 * Runtime configuration, assigned by flags-2-env conventions.
 *
 * The key names are the same strings flags-2-env emits for Rust, Dart and
 * Gleam. The lookup is a parameter rather than an ambient read of
 * `process.env`, so the same function serves a browser bundle (over
 * `import.meta.env` or a `<script type="application/json">` blob), a service
 * worker, and Node.
 */

export const API_BASE = "GHA_INDIE_WORKER_API_BASE" as const;
export const WS_BASE = "GHA_INDIE_WORKER_WS_BASE" as const;
export const SURFACE = "GHA_INDIE_WORKER_SURFACE" as const;
export const CLIENT_TIMEOUT_MS = "GHA_INDIE_WORKER_CLIENT_TIMEOUT_MS" as const;
export const SHARED_AUTH_BASE = "SHARED_AUTH_BASE" as const;
export const SHARED_AUTH_AUDIENCE = "SHARED_AUTH_AUDIENCE" as const;

export const KEYS = [
  API_BASE,
  WS_BASE,
  SURFACE,
  CLIENT_TIMEOUT_MS,
  SHARED_AUTH_BASE,
  SHARED_AUTH_AUDIENCE,
] as const;

export const DEFAULT_TIMEOUT_MS = 30_000;
const MAX_TIMEOUT_MS = 600_000;

/**
 * Which `indiebuild.dev` host this client is running on. A union of string
 * literals rather than a TypeScript `enum`: `enum` is not erasable syntax, and
 * the whole package must run under `node --test` with no build step.
 */
export const SURFACES = ["app", "user", "org", "mobile", "native_app"] as const;
export type Surface = (typeof SURFACES)[number];

export function isSurface(value: string): value is Surface {
  return (SURFACES as readonly string[]).includes(value);
}

/** Only the B2B surface shows seat and invitation UI. */
export function isOrganizationSurface(surface: Surface): boolean {
  return surface === "org";
}

export interface RuntimeConfig {
  readonly apiBase: string;
  readonly wsBase: string;
  readonly surface: Surface;
  readonly timeoutMs: number;
  readonly sharedAuthBase: string | null;
  readonly sharedAuthAudience: string | null;
}

export class ConfigError extends Error {
  readonly key: string;

  constructor(key: string, reason: string) {
    super(`invalid runtime configuration for ${key}: ${reason}`);
    this.name = "ConfigError";
    this.key = key;
  }
}

export type Lookup = (key: string) => string | undefined;

/** Build a config from any key/value lookup. Throws `ConfigError` on bad input. */
export function loadConfig(lookup: Lookup): RuntimeConfig {
  const apiBase = required(lookup, API_BASE);
  validateOrigin(API_BASE, apiBase);

  const explicitWs = present(lookup(WS_BASE));
  if (explicitWs !== null && !/^wss?:\/\//.test(explicitWs)) {
    throw new ConfigError(WS_BASE, explicitWs);
  }
  const wsBase = explicitWs ?? deriveWsBase(apiBase);

  const rawSurface = present(lookup(SURFACE));
  if (rawSurface !== null && !isSurface(rawSurface)) {
    throw new ConfigError(SURFACE, rawSurface);
  }
  const surface: Surface = rawSurface ?? "app";

  const rawTimeout = present(lookup(CLIENT_TIMEOUT_MS));
  let timeoutMs = DEFAULT_TIMEOUT_MS;
  if (rawTimeout !== null) {
    // `Number()` accepts "1e3" and " 12 "; the API contract is integer
    // milliseconds, so anything else is a configuration error, not a coercion.
    if (!/^\d+$/.test(rawTimeout)) {
      throw new ConfigError(CLIENT_TIMEOUT_MS, rawTimeout);
    }
    timeoutMs = Number.parseInt(rawTimeout, 10);
    if (timeoutMs === 0 || timeoutMs > MAX_TIMEOUT_MS) {
      throw new ConfigError(CLIENT_TIMEOUT_MS, rawTimeout);
    }
  }

  return {
    apiBase,
    wsBase,
    surface,
    timeoutMs,
    sharedAuthBase: present(lookup(SHARED_AUTH_BASE)),
    sharedAuthAudience: present(lookup(SHARED_AUTH_AUDIENCE)),
  };
}

/** Join a path onto the API origin without doubling or dropping the separator. */
export function endpoint(config: RuntimeConfig, path: string): string {
  const base = config.apiBase.replace(/\/+$/, "");
  return path.startsWith("/") ? `${base}${path}` : `${base}/${path}`;
}

function present(value: string | undefined): string | null {
  if (value === undefined) {
    return null;
  }
  const trimmed = value.trim();
  return trimmed.length === 0 ? null : trimmed;
}

function required(lookup: Lookup, key: string): string {
  const value = present(lookup(key));
  if (value === null) {
    throw new ConfigError(key, "missing");
  }
  return value;
}

function validateOrigin(key: string, value: string): void {
  if (!/^https?:\/\//.test(value)) {
    throw new ConfigError(key, value);
  }
  if (value.includes("?") || value.includes("#")) {
    throw new ConfigError(key, value);
  }
}

/**
 * `https://` → `wss://`, `http://` → `ws://`. The security level is preserved
 * rather than upgraded: a browser blocks a plaintext socket from an https page
 * anyway, and silently downgrading would be worse than failing.
 */
function deriveWsBase(apiBase: string): string {
  if (apiBase.startsWith("https://")) {
    return `wss://${apiBase.slice("https://".length)}`;
  }
  if (apiBase.startsWith("http://")) {
    return `ws://${apiBase.slice("http://".length)}`;
  }
  return apiBase;
}
