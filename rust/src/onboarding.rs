//! A mirror of the server's onboarding state machines, for optimistic UI.
//!
//! The authority is `gha-indie-worker-orm-core::enums`, which is backend-only.
//! This is the same two machines expressed for a client, so a wizard can advance
//! its own step immediately and only reconcile when the server answers — and,
//! crucially, so it can tell a *pending* step from an *impossible* one. A UI
//! that offers a button for a transition the server will reject is a bug the
//! user pays for.
//!
//! Keeping the transitions here rather than in the UI means Flutter, the web
//! islands and the desktop app cannot disagree about what "next" is.

use serde::{Deserialize, Serialize};

use crate::error::{ClientError, Result};

/// Organization (B2B) onboarding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrgOnboardingState {
    Created,
    VerifyingEmail,
    AwaitingBilling,
    InvitingMembers,
    ConnectingRepository,
    FirstRunPending,
    Active,
    Suspended,
}

/// Individual (B2C) onboarding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserOnboardingState {
    Registered,
    VerifyingEmail,
    ChoosingAccountKind,
    JoiningOrg,
    ConnectingRepository,
    FirstRunPending,
    Active,
    Disabled,
}

impl OrgOnboardingState {
    pub const ALL: &'static [Self] = &[
        Self::Created,
        Self::VerifyingEmail,
        Self::AwaitingBilling,
        Self::InvitingMembers,
        Self::ConnectingRepository,
        Self::FirstRunPending,
        Self::Active,
        Self::Suspended,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::VerifyingEmail => "verifying_email",
            Self::AwaitingBilling => "awaiting_billing",
            Self::InvitingMembers => "inviting_members",
            Self::ConnectingRepository => "connecting_repository",
            Self::FirstRunPending => "first_run_pending",
            Self::Active => "active",
            Self::Suspended => "suspended",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|s| s.as_str() == value)
    }

    pub const fn successors(self) -> &'static [Self] {
        match self {
            Self::Created => &[Self::VerifyingEmail, Self::Suspended],
            Self::VerifyingEmail => &[Self::AwaitingBilling, Self::Suspended],
            Self::AwaitingBilling => &[Self::InvitingMembers, Self::Suspended],
            Self::InvitingMembers => &[Self::ConnectingRepository, Self::Suspended],
            Self::ConnectingRepository => &[Self::FirstRunPending, Self::Suspended],
            Self::FirstRunPending => &[Self::Active, Self::Suspended],
            Self::Active => &[Self::Suspended],
            Self::Suspended => &[Self::Active],
        }
    }

    /// The step a wizard's primary button advances to, if there is one.
    /// `Suspended` is reachable but is never the *primary* next step.
    pub fn next_step(self) -> Option<Self> {
        self.successors()
            .iter()
            .copied()
            .find(|state| !matches!(state, Self::Suspended))
    }

    pub fn transition(self, to: Self) -> Result<Self> {
        if self.successors().contains(&to) {
            Ok(to)
        } else {
            Err(ClientError::IllegalTransition {
                machine: "org_onboarding",
                from: self.as_str(),
                to: to.as_str(),
            })
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Active | Self::Suspended)
    }

    /// 0..=1 for a progress bar. `Suspended` is deliberately zero: it is not
    /// "almost done", it is stopped.
    pub fn progress_permille(self) -> u16 {
        match self {
            Self::Suspended => 0,
            Self::Created => 0,
            Self::VerifyingEmail => 167,
            Self::AwaitingBilling => 333,
            Self::InvitingMembers => 500,
            Self::ConnectingRepository => 667,
            Self::FirstRunPending => 833,
            Self::Active => 1000,
        }
    }
}

impl UserOnboardingState {
    pub const ALL: &'static [Self] = &[
        Self::Registered,
        Self::VerifyingEmail,
        Self::ChoosingAccountKind,
        Self::JoiningOrg,
        Self::ConnectingRepository,
        Self::FirstRunPending,
        Self::Active,
        Self::Disabled,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Registered => "registered",
            Self::VerifyingEmail => "verifying_email",
            Self::ChoosingAccountKind => "choosing_account_kind",
            Self::JoiningOrg => "joining_org",
            Self::ConnectingRepository => "connecting_repository",
            Self::FirstRunPending => "first_run_pending",
            Self::Active => "active",
            Self::Disabled => "disabled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|s| s.as_str() == value)
    }

    pub const fn successors(self) -> &'static [Self] {
        match self {
            Self::Registered => &[Self::VerifyingEmail, Self::Disabled],
            Self::VerifyingEmail => &[Self::ChoosingAccountKind, Self::Disabled],
            Self::ChoosingAccountKind => {
                &[Self::JoiningOrg, Self::ConnectingRepository, Self::Disabled]
            }
            Self::JoiningOrg => &[Self::Active, Self::ConnectingRepository, Self::Disabled],
            Self::ConnectingRepository => &[Self::FirstRunPending, Self::Disabled],
            Self::FirstRunPending => &[Self::Active, Self::Disabled],
            Self::Active => &[Self::Disabled],
            Self::Disabled => &[Self::Active],
        }
    }

    /// `ChoosingAccountKind` is the fork: a user either joins an existing org
    /// (B2B) or connects a repository of their own (B2C). There is no single
    /// "next", and a UI that pretends otherwise picks for the user.
    pub const fn is_fork(self) -> bool {
        matches!(self, Self::ChoosingAccountKind)
    }

    pub fn transition(self, to: Self) -> Result<Self> {
        if self.successors().contains(&to) {
            Ok(to)
        } else {
            Err(ClientError::IllegalTransition {
                machine: "user_onboarding",
                from: self.as_str(),
                to: to.as_str(),
            })
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Active | Self::Disabled)
    }
}

/// An optimistic step: the UI moves now, and reconciles when the server answers.
///
/// The invariant this type enforces is that an optimistic move is only ever
/// made along an edge the machine actually has. A server response that
/// contradicts the optimistic state wins — [`Optimistic::reconcile`] takes the
/// server's word without asking whether the edge existed, because by then it
/// demonstrably did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Optimistic<S> {
    confirmed: S,
    pending: Option<S>,
}

impl<S: Copy + PartialEq> Optimistic<S> {
    pub fn new(confirmed: S) -> Self {
        Self {
            confirmed,
            pending: None,
        }
    }

    /// What the UI should render: the optimistic state if one is in flight.
    pub fn displayed(&self) -> S {
        self.pending.unwrap_or(self.confirmed)
    }

    pub fn confirmed(&self) -> S {
        self.confirmed
    }

    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Accept the server's state. Clears any pending move, whether or not it
    /// was the one that happened.
    pub fn reconcile(&mut self, server: S) {
        self.confirmed = server;
        self.pending = None;
    }

    /// Abandon a pending move — a failed request, or a cancelled dialog.
    pub fn rollback(&mut self) {
        self.pending = None;
    }
}

impl Optimistic<OrgOnboardingState> {
    /// Move optimistically, but only along a real edge.
    pub fn advance(&mut self, to: OrgOnboardingState) -> Result<()> {
        self.displayed().transition(to)?;
        self.pending = Some(to);
        Ok(())
    }
}

impl Optimistic<UserOnboardingState> {
    pub fn advance(&mut self, to: UserOnboardingState) -> Result<()> {
        self.displayed().transition(to)?;
        self.pending = Some(to);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_org_state_round_trips_through_its_wire_string() {
        for state in OrgOnboardingState::ALL {
            assert_eq!(OrgOnboardingState::parse(state.as_str()), Some(*state));
            let json = serde_json::to_string(state).expect("serialize");
            assert_eq!(json, alloc::format!("\"{}\"", state.as_str()));
        }
        assert_eq!(OrgOnboardingState::parse("nope"), None);
    }

    #[test]
    fn every_user_state_round_trips_through_its_wire_string() {
        for state in UserOnboardingState::ALL {
            assert_eq!(UserOnboardingState::parse(state.as_str()), Some(*state));
            let json = serde_json::to_string(state).expect("serialize");
            assert_eq!(json, alloc::format!("\"{}\"", state.as_str()));
        }
    }

    #[test]
    fn the_org_happy_path_is_reachable_one_step_at_a_time() {
        let mut state = OrgOnboardingState::Created;
        let mut steps = 0;
        while let Some(next) = state.next_step() {
            state = state.transition(next).expect("legal");
            steps += 1;
            assert!(steps < 10, "the machine has a cycle");
            if state == OrgOnboardingState::Active {
                break;
            }
        }
        assert_eq!(state, OrgOnboardingState::Active);
        assert_eq!(steps, 6);
    }

    #[test]
    fn skipping_billing_is_rejected() {
        assert_eq!(
            OrgOnboardingState::Created.transition(OrgOnboardingState::Active),
            Err(ClientError::IllegalTransition {
                machine: "org_onboarding",
                from: "created",
                to: "active",
            })
        );
    }

    #[test]
    fn suspension_is_never_the_primary_next_step() {
        for state in OrgOnboardingState::ALL {
            assert_ne!(state.next_step(), Some(OrgOnboardingState::Suspended));
        }
        // …but it is always reachable, except from itself.
        for state in OrgOnboardingState::ALL {
            if *state == OrgOnboardingState::Suspended {
                continue;
            }
            assert!(state.successors().contains(&OrgOnboardingState::Suspended));
        }
    }

    #[test]
    fn progress_is_monotonic_along_the_happy_path_and_zero_when_suspended() {
        let path = [
            OrgOnboardingState::Created,
            OrgOnboardingState::VerifyingEmail,
            OrgOnboardingState::AwaitingBilling,
            OrgOnboardingState::InvitingMembers,
            OrgOnboardingState::ConnectingRepository,
            OrgOnboardingState::FirstRunPending,
            OrgOnboardingState::Active,
        ];
        for pair in path.windows(2) {
            assert!(
                pair[0].progress_permille() < pair[1].progress_permille(),
                "{:?} -> {:?} did not advance",
                pair[0],
                pair[1]
            );
        }
        assert_eq!(OrgOnboardingState::Active.progress_permille(), 1000);
        assert_eq!(OrgOnboardingState::Suspended.progress_permille(), 0);
    }

    #[test]
    fn the_user_machine_forks_at_the_account_kind_choice() {
        let fork = UserOnboardingState::ChoosingAccountKind;
        assert!(fork.is_fork());
        assert!(fork.successors().contains(&UserOnboardingState::JoiningOrg));
        assert!(fork
            .successors()
            .contains(&UserOnboardingState::ConnectingRepository));
        assert!(!UserOnboardingState::Registered.is_fork());
    }

    #[test]
    fn terminal_states_have_only_the_reactivation_edge() {
        assert_eq!(
            OrgOnboardingState::Active.successors(),
            [OrgOnboardingState::Suspended].as_slice()
        );
        assert_eq!(
            UserOnboardingState::Disabled.successors(),
            [UserOnboardingState::Active].as_slice()
        );
        assert!(OrgOnboardingState::Active.is_terminal());
        assert!(UserOnboardingState::Disabled.is_terminal());
    }

    #[test]
    fn an_optimistic_move_renders_immediately_and_keeps_the_confirmed_state() {
        let mut wizard = Optimistic::new(OrgOnboardingState::Created);
        assert!(!wizard.is_pending());
        wizard
            .advance(OrgOnboardingState::VerifyingEmail)
            .expect("legal edge");
        assert!(wizard.is_pending());
        assert_eq!(wizard.displayed(), OrgOnboardingState::VerifyingEmail);
        assert_eq!(wizard.confirmed(), OrgOnboardingState::Created);
    }

    #[test]
    fn an_optimistic_move_along_a_missing_edge_is_refused_and_changes_nothing() {
        let mut wizard = Optimistic::new(OrgOnboardingState::Created);
        assert!(wizard.advance(OrgOnboardingState::Active).is_err());
        assert!(!wizard.is_pending());
        assert_eq!(wizard.displayed(), OrgOnboardingState::Created);
    }

    #[test]
    fn reconciling_takes_the_servers_word_even_when_it_disagrees() {
        let mut wizard = Optimistic::new(OrgOnboardingState::Created);
        wizard
            .advance(OrgOnboardingState::VerifyingEmail)
            .expect("legal edge");
        wizard.reconcile(OrgOnboardingState::Suspended);
        assert_eq!(wizard.displayed(), OrgOnboardingState::Suspended);
        assert!(!wizard.is_pending());
    }

    #[test]
    fn rolling_back_returns_to_the_confirmed_state() {
        let mut wizard = Optimistic::new(UserOnboardingState::Registered);
        wizard
            .advance(UserOnboardingState::VerifyingEmail)
            .expect("legal edge");
        wizard.rollback();
        assert_eq!(wizard.displayed(), UserOnboardingState::Registered);
        assert!(!wizard.is_pending());
    }

    #[test]
    fn a_pending_move_chains_from_the_displayed_state_not_the_confirmed_one() {
        let mut wizard = Optimistic::new(OrgOnboardingState::Created);
        wizard
            .advance(OrgOnboardingState::VerifyingEmail)
            .expect("legal");
        // Legal from `verifying_email`, illegal from `created`.
        wizard
            .advance(OrgOnboardingState::AwaitingBilling)
            .expect("chains from the displayed state");
        assert_eq!(wizard.displayed(), OrgOnboardingState::AwaitingBilling);
    }
}
