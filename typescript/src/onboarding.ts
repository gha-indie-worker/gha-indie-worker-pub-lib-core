/**
 * The onboarding state machines, mirrored for optimistic UI.
 *
 * The authority is `gha-indie-worker-orm-core::enums` (backend only). This is
 * the same two machines, expressed so a wizard can advance immediately and
 * reconcile when the server answers — and so it can tell a *pending* step from
 * an *impossible* one. Keeping the edges here rather than in a component means
 * the web islands, Flutter and the desktop app cannot disagree about "next".
 */

export const ORG_ONBOARDING_STATES = [
  "created",
  "verifying_email",
  "awaiting_billing",
  "inviting_members",
  "connecting_repository",
  "first_run_pending",
  "active",
  "suspended",
] as const;
export type OrgOnboardingState = (typeof ORG_ONBOARDING_STATES)[number];

export const USER_ONBOARDING_STATES = [
  "registered",
  "verifying_email",
  "choosing_account_kind",
  "joining_org",
  "connecting_repository",
  "first_run_pending",
  "active",
  "disabled",
] as const;
export type UserOnboardingState = (typeof USER_ONBOARDING_STATES)[number];

const ORG_EDGES: Readonly<Record<OrgOnboardingState, readonly OrgOnboardingState[]>> = {
  created: ["verifying_email", "suspended"],
  verifying_email: ["awaiting_billing", "suspended"],
  awaiting_billing: ["inviting_members", "suspended"],
  inviting_members: ["connecting_repository", "suspended"],
  connecting_repository: ["first_run_pending", "suspended"],
  first_run_pending: ["active", "suspended"],
  active: ["suspended"],
  suspended: ["active"],
};

const USER_EDGES: Readonly<Record<UserOnboardingState, readonly UserOnboardingState[]>> = {
  registered: ["verifying_email", "disabled"],
  verifying_email: ["choosing_account_kind", "disabled"],
  choosing_account_kind: ["joining_org", "connecting_repository", "disabled"],
  joining_org: ["active", "connecting_repository", "disabled"],
  connecting_repository: ["first_run_pending", "disabled"],
  first_run_pending: ["active", "disabled"],
  active: ["disabled"],
  disabled: ["active"],
};

/** Percent-mille progress for a bar. `suspended` is zero: stopped, not nearly done. */
const ORG_PROGRESS: Readonly<Record<OrgOnboardingState, number>> = {
  created: 0,
  verifying_email: 167,
  awaiting_billing: 333,
  inviting_members: 500,
  connecting_repository: 667,
  first_run_pending: 833,
  active: 1000,
  suspended: 0,
};

export class TransitionError extends Error {
  readonly from: string;
  readonly to: string;

  constructor(machine: string, from: string, to: string) {
    super(`illegal ${machine} transition: ${from} -> ${to}`);
    this.name = "TransitionError";
    this.from = from;
    this.to = to;
  }
}

export function isOrgOnboardingState(value: string): value is OrgOnboardingState {
  return (ORG_ONBOARDING_STATES as readonly string[]).includes(value);
}

export function isUserOnboardingState(value: string): value is UserOnboardingState {
  return (USER_ONBOARDING_STATES as readonly string[]).includes(value);
}

export function orgSuccessors(state: OrgOnboardingState): readonly OrgOnboardingState[] {
  return ORG_EDGES[state];
}

export function userSuccessors(state: UserOnboardingState): readonly UserOnboardingState[] {
  return USER_EDGES[state];
}

/** The step a wizard's primary button advances to. Suspension is never it. */
export function orgNextStep(state: OrgOnboardingState): OrgOnboardingState | null {
  return ORG_EDGES[state].find((next) => next !== "suspended") ?? null;
}

export function orgTransition(
  from: OrgOnboardingState,
  to: OrgOnboardingState,
): OrgOnboardingState {
  if (!ORG_EDGES[from].includes(to)) {
    throw new TransitionError("org_onboarding", from, to);
  }
  return to;
}

export function userTransition(
  from: UserOnboardingState,
  to: UserOnboardingState,
): UserOnboardingState {
  if (!USER_EDGES[from].includes(to)) {
    throw new TransitionError("user_onboarding", from, to);
  }
  return to;
}

export function orgIsTerminal(state: OrgOnboardingState): boolean {
  return state === "active" || state === "suspended";
}

export function userIsTerminal(state: UserOnboardingState): boolean {
  return state === "active" || state === "disabled";
}

/**
 * `choosing_account_kind` is a fork — join an existing org, or connect your own
 * repository. A UI that shows one "next" button here picks for the user.
 */
export function userIsFork(state: UserOnboardingState): boolean {
  return state === "choosing_account_kind";
}

export function orgProgressPermille(state: OrgOnboardingState): number {
  return ORG_PROGRESS[state];
}

/**
 * An optimistic step. Immutable: `advance`, `reconcile` and `rollback` return a
 * new value rather than mutating, so a store can diff them.
 */
export interface Optimistic<S> {
  readonly confirmed: S;
  readonly pending: S | null;
}

export function optimistic<S>(confirmed: S): Optimistic<S> {
  return { confirmed, pending: null };
}

export function displayed<S>(value: Optimistic<S>): S {
  return value.pending ?? value.confirmed;
}

export function isPending<S>(value: Optimistic<S>): boolean {
  return value.pending !== null;
}

/** Move optimistically, but only along an edge the machine actually has. */
export function advanceOrg(
  value: Optimistic<OrgOnboardingState>,
  to: OrgOnboardingState,
): Optimistic<OrgOnboardingState> {
  orgTransition(displayed(value), to);
  return { confirmed: value.confirmed, pending: to };
}

export function advanceUser(
  value: Optimistic<UserOnboardingState>,
  to: UserOnboardingState,
): Optimistic<UserOnboardingState> {
  userTransition(displayed(value), to);
  return { confirmed: value.confirmed, pending: to };
}

/** The server's word wins, whatever was pending. */
export function reconcile<S>(_value: Optimistic<S>, server: S): Optimistic<S> {
  return { confirmed: server, pending: null };
}

export function rollback<S>(value: Optimistic<S>): Optimistic<S> {
  return { confirmed: value.confirmed, pending: null };
}
