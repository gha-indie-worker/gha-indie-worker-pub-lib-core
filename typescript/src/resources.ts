/**
 * Typed API resources.
 *
 * These mirror the contracts in `gha-indie-worker-interfaces`. Once
 * `ores-contracts generate` emits `generated/typescript/types.d.ts` there, this
 * file becomes a re-export of it — see `contracts/README.md`. Until then the
 * shapes are hand-written so client work is not blocked, and
 * `contracts/expected-resources.json` records the field names so the swap is
 * mechanical.
 *
 * The parse functions are total: they either return a resource or throw with
 * the offending field named. A client that silently accepts a malformed run is
 * a client that renders nonsense three screens later.
 */

export const RUN_STATES = [
  "queued",
  "planning",
  "dispatching",
  "running",
  "completed",
  "cancelled",
] as const;
export type RunState = (typeof RUN_STATES)[number];

export const JOB_STATES = [
  "pending",
  "ready",
  "leased",
  "running",
  "completed",
  "cancelled",
] as const;
export type JobState = (typeof JOB_STATES)[number];

export const CONCLUSIONS = [
  "success",
  "failure",
  "cancelled",
  "timed_out",
  "unsupported",
  "skipped",
] as const;
export type Conclusion = (typeof CONCLUSIONS)[number];

export const RUN_LANES = ["arc", "independent"] as const;
export type RunLane = (typeof RUN_LANES)[number];

export interface JobResource {
  readonly id: string;
  readonly name: string;
  readonly ordinal: number;
  readonly profile: string;
  /** A state this build does not recognise is preserved as a raw string. */
  readonly state: JobState | string;
  readonly conclusion: Conclusion | string | null;
  readonly needs: readonly string[];
  /** Byte offset a log tail resumes from. */
  readonly logOffset: number;
}

export interface RunResource {
  readonly id: string;
  readonly repository: string;
  /** Always a full 40-hex commit SHA — never a branch or tag. */
  readonly revision: string;
  readonly workflowPath: string;
  readonly lane: RunLane | string;
  readonly state: RunState | string;
  readonly conclusion: Conclusion | string | null;
  readonly queuedAt: string;
  readonly startedAt: string | null;
  readonly finishedAt: string | null;
  readonly jobs: readonly JobResource[];
}

export interface WorkerResource {
  readonly id: string;
  readonly name: string;
  readonly pool: string;
  readonly architecture: string;
  readonly operatingSystem: string;
  readonly maxConcurrency: number;
  readonly activeLeases: number;
  readonly lastHeartbeatAt: string | null;
}

export class ResourceError extends Error {
  readonly field: string;

  constructor(field: string, reason: string) {
    super(`malformed resource: ${field} ${reason}`);
    this.name = "ResourceError";
    this.field = field;
  }
}

const COMMIT_SHA = /^[0-9a-f]{40}$/;

export function isRunState(value: string): value is RunState {
  return (RUN_STATES as readonly string[]).includes(value);
}

export function isTerminalRunState(value: string): boolean {
  return value === "completed" || value === "cancelled";
}

/**
 * A run is in flight unless it reached a terminal state this build recognises.
 * An unrecognised state counts as in flight, so a client keeps polling rather
 * than declaring a run finished it cannot read.
 */
export function isRunInFlight(run: RunResource): boolean {
  return !isTerminalRunState(run.state);
}

export function freeCapacity(worker: WorkerResource): number {
  return Math.max(0, worker.maxConcurrency - worker.activeLeases);
}

/** Parse one run from a decoded JSON value. Throws `ResourceError` on bad shape. */
export function parseRun(value: unknown): RunResource {
  const raw = asObject(value, "run");
  const revision = asString(raw, "revision");
  if (!COMMIT_SHA.test(revision)) {
    throw new ResourceError("revision", "is not a lowercase 40-hex commit sha");
  }
  const jobsValue = raw["jobs"];
  const jobs =
    jobsValue === undefined || jobsValue === null
      ? []
      : asArray(jobsValue, "jobs").map(parseJob);
  return {
    id: asString(raw, "id"),
    repository: asString(raw, "repository"),
    revision,
    workflowPath: asString(raw, "workflowPath"),
    lane: asString(raw, "lane"),
    state: asString(raw, "state"),
    conclusion: optionalString(raw, "conclusion"),
    queuedAt: asString(raw, "queuedAt"),
    startedAt: optionalString(raw, "startedAt"),
    finishedAt: optionalString(raw, "finishedAt"),
    jobs,
  };
}

export function parseJob(value: unknown): JobResource {
  const raw = asObject(value, "job");
  const needsValue = raw["needs"];
  const needs =
    needsValue === undefined || needsValue === null
      ? []
      : asArray(needsValue, "needs").map((entry, index) => {
          if (typeof entry !== "string") {
            throw new ResourceError(`needs[${index}]`, "is not a string");
          }
          return entry;
        });
  return {
    id: asString(raw, "id"),
    name: asString(raw, "name"),
    ordinal: asNumber(raw, "ordinal"),
    profile: asString(raw, "profile"),
    state: asString(raw, "state"),
    conclusion: optionalString(raw, "conclusion"),
    needs,
    logOffset: raw["logOffset"] === undefined ? 0 : asNumber(raw, "logOffset"),
  };
}

function asObject(value: unknown, field: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new ResourceError(field, "is not an object");
  }
  return value as Record<string, unknown>;
}

function asArray(value: unknown, field: string): unknown[] {
  if (!Array.isArray(value)) {
    throw new ResourceError(field, "is not an array");
  }
  return value;
}

function asString(raw: Record<string, unknown>, field: string): string {
  const value = raw[field];
  if (typeof value !== "string" || value.length === 0) {
    throw new ResourceError(field, "is missing or not a non-empty string");
  }
  return value;
}

function optionalString(raw: Record<string, unknown>, field: string): string | null {
  const value = raw[field];
  if (value === undefined || value === null) {
    return null;
  }
  if (typeof value !== "string") {
    throw new ResourceError(field, "is not a string");
  }
  return value;
}

function asNumber(raw: Record<string, unknown>, field: string): number {
  const value = raw[field];
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new ResourceError(field, "is missing or not a finite number");
  }
  return value;
}
