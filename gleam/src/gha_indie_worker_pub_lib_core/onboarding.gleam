//// The onboarding state machines, mirrored for client use.
////
//// The authority is `gha-indie-worker-orm-core` (backend only). This is the
//// same two machines expressed in Gleam, so a Gleam client cannot disagree with
//// the Rust, TypeScript and Dart ports about which edges exist.
////
//// Every function here is total: `transition` returns a `Result` rather than
//// crashing, and `parse` returns a `Result` rather than a default.

import gleam/list

/// Organization (B2B) onboarding.
pub type OrgState {
  Created
  OrgVerifyingEmail
  AwaitingBilling
  InvitingMembers
  OrgConnectingRepository
  OrgFirstRunPending
  OrgActive
  Suspended
}

/// Individual (B2C) onboarding.
pub type UserState {
  Registered
  UserVerifyingEmail
  ChoosingAccountKind
  JoiningOrg
  UserConnectingRepository
  UserFirstRunPending
  UserActive
  Disabled
}

pub type TransitionError {
  IllegalTransition(machine: String, from: String, to: String)
  UnknownState(value: String)
}

pub fn org_states() -> List(OrgState) {
  [
    Created,
    OrgVerifyingEmail,
    AwaitingBilling,
    InvitingMembers,
    OrgConnectingRepository,
    OrgFirstRunPending,
    OrgActive,
    Suspended,
  ]
}

pub fn user_states() -> List(UserState) {
  [
    Registered,
    UserVerifyingEmail,
    ChoosingAccountKind,
    JoiningOrg,
    UserConnectingRepository,
    UserFirstRunPending,
    UserActive,
    Disabled,
  ]
}

pub fn org_to_string(state: OrgState) -> String {
  case state {
    Created -> "created"
    OrgVerifyingEmail -> "verifying_email"
    AwaitingBilling -> "awaiting_billing"
    InvitingMembers -> "inviting_members"
    OrgConnectingRepository -> "connecting_repository"
    OrgFirstRunPending -> "first_run_pending"
    OrgActive -> "active"
    Suspended -> "suspended"
  }
}

pub fn user_to_string(state: UserState) -> String {
  case state {
    Registered -> "registered"
    UserVerifyingEmail -> "verifying_email"
    ChoosingAccountKind -> "choosing_account_kind"
    JoiningOrg -> "joining_org"
    UserConnectingRepository -> "connecting_repository"
    UserFirstRunPending -> "first_run_pending"
    UserActive -> "active"
    Disabled -> "disabled"
  }
}

pub fn org_from_string(value: String) -> Result(OrgState, TransitionError) {
  case list.find(org_states(), fn(state) { org_to_string(state) == value }) {
    Ok(state) -> Ok(state)
    Error(_) -> Error(UnknownState(value))
  }
}

pub fn user_from_string(value: String) -> Result(UserState, TransitionError) {
  case list.find(user_states(), fn(state) { user_to_string(state) == value }) {
    Ok(state) -> Ok(state)
    Error(_) -> Error(UnknownState(value))
  }
}

pub fn org_successors(state: OrgState) -> List(OrgState) {
  case state {
    Created -> [OrgVerifyingEmail, Suspended]
    OrgVerifyingEmail -> [AwaitingBilling, Suspended]
    AwaitingBilling -> [InvitingMembers, Suspended]
    InvitingMembers -> [OrgConnectingRepository, Suspended]
    OrgConnectingRepository -> [OrgFirstRunPending, Suspended]
    OrgFirstRunPending -> [OrgActive, Suspended]
    OrgActive -> [Suspended]
    Suspended -> [OrgActive]
  }
}

pub fn user_successors(state: UserState) -> List(UserState) {
  case state {
    Registered -> [UserVerifyingEmail, Disabled]
    UserVerifyingEmail -> [ChoosingAccountKind, Disabled]
    ChoosingAccountKind -> [JoiningOrg, UserConnectingRepository, Disabled]
    JoiningOrg -> [UserActive, UserConnectingRepository, Disabled]
    UserConnectingRepository -> [UserFirstRunPending, Disabled]
    UserFirstRunPending -> [UserActive, Disabled]
    UserActive -> [Disabled]
    Disabled -> [UserActive]
  }
}

/// The step a wizard's primary button advances to. Suspension is reachable but
/// is never the primary next step.
pub fn org_next_step(state: OrgState) -> Result(OrgState, Nil) {
  list.find(org_successors(state), fn(next) { next != Suspended })
}

pub fn org_transition(
  from: OrgState,
  to: OrgState,
) -> Result(OrgState, TransitionError) {
  case list.contains(org_successors(from), to) {
    True -> Ok(to)
    False ->
      Error(IllegalTransition(
        "org_onboarding",
        org_to_string(from),
        org_to_string(to),
      ))
  }
}

pub fn user_transition(
  from: UserState,
  to: UserState,
) -> Result(UserState, TransitionError) {
  case list.contains(user_successors(from), to) {
    True -> Ok(to)
    False ->
      Error(IllegalTransition(
        "user_onboarding",
        user_to_string(from),
        user_to_string(to),
      ))
  }
}

pub fn org_is_terminal(state: OrgState) -> Bool {
  state == OrgActive || state == Suspended
}

pub fn user_is_terminal(state: UserState) -> Bool {
  state == UserActive || state == Disabled
}

/// `ChoosingAccountKind` is a fork — join an existing org, or connect your own
/// repository. A UI that shows one "next" button here picks for the user.
pub fn user_is_fork(state: UserState) -> Bool {
  state == ChoosingAccountKind
}

/// Per-mille progress for a bar. `Suspended` is zero: stopped, not nearly done.
pub fn org_progress_permille(state: OrgState) -> Int {
  case state {
    Created -> 0
    OrgVerifyingEmail -> 167
    AwaitingBilling -> 333
    InvitingMembers -> 500
    OrgConnectingRepository -> 667
    OrgFirstRunPending -> 833
    OrgActive -> 1000
    Suspended -> 0
  }
}
