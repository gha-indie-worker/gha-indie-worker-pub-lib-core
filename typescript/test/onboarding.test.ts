import test from "node:test";
import assert from "node:assert/strict";

import {
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
} from "../src/onboarding.ts";
import type { OrgOnboardingState } from "../src/onboarding.ts";

test("every declared state is recognised and nothing else is", () => {
  for (const state of ORG_ONBOARDING_STATES) {
    assert.ok(isOrgOnboardingState(state));
  }
  for (const state of USER_ONBOARDING_STATES) {
    assert.ok(isUserOnboardingState(state));
  }
  assert.ok(!isOrgOnboardingState("nope"));
  assert.ok(!isUserOnboardingState("nope"));
});

test("the org happy path is reachable one step at a time", () => {
  let state: OrgOnboardingState = "created";
  let steps = 0;
  for (;;) {
    const next = orgNextStep(state);
    if (next === null) {
      break;
    }
    state = orgTransition(state, next);
    steps += 1;
    assert.ok(steps < 10, "the machine has a cycle");
    if (state === "active") {
      break;
    }
  }
  assert.equal(state, "active");
  assert.equal(steps, 6);
});

test("skipping billing is rejected", () => {
  assert.throws(() => orgTransition("created", "active"), TransitionError);
  assert.throws(() => userTransition("registered", "active"), TransitionError);
});

test("suspension is never the primary next step but is always reachable", () => {
  for (const state of ORG_ONBOARDING_STATES) {
    assert.notEqual(orgNextStep(state), "suspended");
    if (state !== "suspended") {
      assert.ok(orgSuccessors(state).includes("suspended"), state);
    }
  }
});

test("progress is monotonic along the happy path and zero when suspended", () => {
  const path: OrgOnboardingState[] = [
    "created",
    "verifying_email",
    "awaiting_billing",
    "inviting_members",
    "connecting_repository",
    "first_run_pending",
    "active",
  ];
  for (let index = 1; index < path.length; index += 1) {
    const previous = path[index - 1];
    const current = path[index];
    assert.ok(previous !== undefined && current !== undefined);
    assert.ok(
      orgProgressPermille(previous) < orgProgressPermille(current),
      `${previous} -> ${current} did not advance`,
    );
  }
  assert.equal(orgProgressPermille("active"), 1000);
  assert.equal(orgProgressPermille("suspended"), 0);
});

test("the user machine forks at the account-kind choice", () => {
  assert.ok(userIsFork("choosing_account_kind"));
  assert.ok(!userIsFork("registered"));
  const successors = userSuccessors("choosing_account_kind");
  assert.ok(successors.includes("joining_org"));
  assert.ok(successors.includes("connecting_repository"));
});

test("terminal states keep only the reactivation edge", () => {
  assert.deepEqual(orgSuccessors("active"), ["suspended"]);
  assert.deepEqual(userSuccessors("disabled"), ["active"]);
  assert.ok(orgIsTerminal("active"));
  assert.ok(userIsTerminal("disabled"));
});

test("an optimistic move renders immediately and keeps the confirmed state", () => {
  const start = optimistic<OrgOnboardingState>("created");
  assert.ok(!isPending(start));
  const moved = advanceOrg(start, "verifying_email");
  assert.ok(isPending(moved));
  assert.equal(displayed(moved), "verifying_email");
  assert.equal(moved.confirmed, "created");
  // The original value is untouched: these are immutable transitions.
  assert.equal(displayed(start), "created");
});

test("an optimistic move along a missing edge throws and changes nothing", () => {
  const start = optimistic<OrgOnboardingState>("created");
  assert.throws(() => advanceOrg(start, "active"), TransitionError);
  assert.ok(!isPending(start));
});

test("reconciling takes the server's word even when it disagrees", () => {
  const moved = advanceOrg(optimistic<OrgOnboardingState>("created"), "verifying_email");
  const settled = reconcile(moved, "suspended");
  assert.equal(displayed(settled), "suspended");
  assert.ok(!isPending(settled));
});

test("rolling back returns to the confirmed state", () => {
  const moved = advanceUser(optimistic("registered"), "verifying_email");
  const rolled = rollback(moved);
  assert.equal(displayed(rolled), "registered");
  assert.ok(!isPending(rolled));
});

test("a pending move chains from the displayed state, not the confirmed one", () => {
  const first = advanceOrg(optimistic<OrgOnboardingState>("created"), "verifying_email");
  const second = advanceOrg(first, "awaiting_billing");
  assert.equal(displayed(second), "awaiting_billing");
  assert.equal(second.confirmed, "created");
});
