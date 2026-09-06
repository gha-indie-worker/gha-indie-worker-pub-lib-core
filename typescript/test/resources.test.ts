import test from "node:test";
import assert from "node:assert/strict";

import {
  RUN_STATES,
  ResourceError,
  freeCapacity,
  isRunInFlight,
  isRunState,
  isTerminalRunState,
  parseJob,
  parseRun,
} from "../src/resources.ts";

const SHA = "0123456789abcdef0123456789abcdef01234567";

function runJson(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    id: "r1",
    repository: "gha-indie-worker/x",
    revision: SHA,
    workflowPath: ".github/workflows/ci.yml",
    lane: "independent",
    state: "running",
    queuedAt: "2026-09-05T12:00:00Z",
    ...overrides,
  };
}

test("a well-formed run parses and defaults its optional fields", () => {
  const run = parseRun(runJson());
  assert.equal(run.state, "running");
  assert.equal(run.conclusion, null);
  assert.equal(run.startedAt, null);
  assert.deepEqual(run.jobs, []);
  assert.ok(isRunInFlight(run));
});

test("an unknown state is preserved rather than failing the response", () => {
  const run = parseRun(runJson({ state: "quarantined" }));
  assert.equal(run.state, "quarantined");
  assert.ok(!isRunState(run.state));
  // An unrecognised state must keep the client polling.
  assert.ok(isRunInFlight(run));
});

test("a terminal run is not in flight", () => {
  const run = parseRun(runJson({ state: "completed", conclusion: "success" }));
  assert.ok(!isRunInFlight(run));
  assert.equal(run.conclusion, "success");
  assert.ok(isTerminalRunState("cancelled"));
  assert.ok(!isTerminalRunState("running"));
});

test("a revision that is not a lowercase commit sha is rejected", () => {
  for (const bad of ["main", SHA.toUpperCase(), SHA.slice(0, 39), `${SHA}0`]) {
    assert.throws(() => parseRun(runJson({ revision: bad })), ResourceError, bad);
  }
});

test("a missing required field names the field it is missing", () => {
  try {
    parseRun(runJson({ repository: undefined }));
    assert.fail("expected a ResourceError");
  } catch (error) {
    assert.ok(error instanceof ResourceError);
    assert.equal(error.field, "repository");
  }
});

test("a non-object, an array and null are all rejected", () => {
  for (const bad of [null, 42, "run", [], undefined]) {
    assert.throws(() => parseRun(bad), ResourceError);
  }
});

test("nested jobs parse, and a bad job fails the whole run", () => {
  const run = parseRun(
    runJson({
      jobs: [
        {
          id: "j1",
          name: "test",
          ordinal: 0,
          profile: "rust-verify",
          state: "running",
          needs: ["build"],
          logOffset: 4096,
        },
      ],
    }),
  );
  assert.equal(run.jobs.length, 1);
  assert.equal(run.jobs[0]?.logOffset, 4096);
  assert.deepEqual(run.jobs[0]?.needs, ["build"]);

  assert.throws(() => parseRun(runJson({ jobs: [{ id: "j1" }] })), ResourceError);
  assert.throws(() => parseRun(runJson({ jobs: "not-an-array" })), ResourceError);
});

test("a job defaults its log offset and its dependency list", () => {
  const job = parseJob({
    id: "j1",
    name: "test",
    ordinal: 0,
    profile: "rust-verify",
    state: "pending",
  });
  assert.equal(job.logOffset, 0);
  assert.deepEqual(job.needs, []);
});

test("a non-string entry in needs is rejected with its index", () => {
  try {
    parseJob({
      id: "j1",
      name: "test",
      ordinal: 0,
      profile: "rust-verify",
      state: "pending",
      needs: ["build", 7],
    });
    assert.fail("expected a ResourceError");
  } catch (error) {
    assert.ok(error instanceof ResourceError);
    assert.equal(error.field, "needs[1]");
  }
});

test("worker capacity never goes negative", () => {
  assert.equal(
    freeCapacity({
      id: "w1",
      name: "arm-1",
      pool: "shared",
      architecture: "arm64",
      operatingSystem: "linux",
      maxConcurrency: 2,
      activeLeases: 5,
      lastHeartbeatAt: null,
    }),
    0,
  );
});

test("the declared run states are unique", () => {
  assert.equal(new Set(RUN_STATES).size, RUN_STATES.length);
});
