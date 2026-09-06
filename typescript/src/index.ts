/**
 * `@gha-indie-worker/pub-lib-core` — the public client-side core.
 *
 * ESM, zero runtime dependencies, no framework. It describes *what* to send;
 * the host decides *how* — `fetch` in a browser, whatever the desktop shell
 * prefers, `dart:io` under Flutter through the Dart port of this package.
 *
 * It contains no database access and no server secrets. Those live in
 * `gha-indie-worker-orm-core` (backend only) and never reach a client bundle.
 */

export const VERSION = "0.1.0" as const;
export const CLIENT_HEADER = "x-giw-client" as const;

export {
  API_BASE,
  CLIENT_TIMEOUT_MS,
  ConfigError,
  DEFAULT_TIMEOUT_MS,
  KEYS,
  SHARED_AUTH_AUDIENCE,
  SHARED_AUTH_BASE,
  SURFACE,
  SURFACES,
  WS_BASE,
  endpoint,
  isOrganizationSurface,
  isSurface,
  loadConfig,
} from "./config.ts";
export type { Lookup, RuntimeConfig, Surface } from "./config.ts";

export {
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
} from "./pagination.ts";
export type { Cursor, Page, PageRequest } from "./pagination.ts";

export {
  CONCLUSIONS,
  JOB_STATES,
  RUN_LANES,
  RUN_STATES,
  ResourceError,
  freeCapacity,
  isRunInFlight,
  isRunState,
  isTerminalRunState,
  parseJob,
  parseRun,
} from "./resources.ts";
export type {
  Conclusion,
  JobResource,
  JobState,
  RunLane,
  RunResource,
  RunState,
  WorkerResource,
} from "./resources.ts";

export {
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
  toHex,
  verifySignature,
} from "./signing.ts";
export type { SignInput, SignedRequest } from "./signing.ts";

export {
  ORG_ONBOARDING_STATES,
  TransitionError,
  USER_ONBOARDING_STATES,
  advanceOrg,
  advanceUser,
  displayed,
  isOrgOnboardingState,
  isPending,
  isUserOnboardingState,
  optimistic,
  orgIsTerminal,
  orgNextStep,
  orgProgressPermille,
  orgSuccessors,
  orgTransition,
  reconcile,
  rollback,
  userIsFork,
  userIsTerminal,
  userSuccessors,
  userTransition,
} from "./onboarding.ts";
export type {
  Optimistic,
  OrgOnboardingState,
  UserOnboardingState,
} from "./onboarding.ts";
