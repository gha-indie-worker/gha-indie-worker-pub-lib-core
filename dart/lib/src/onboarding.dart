import 'errors.dart';

/// The onboarding state machines, mirrored for optimistic UI.
///
/// The authority is `gha-indie-worker-orm-core::enums` (backend only). Keeping
/// the edges here rather than in a widget means the Flutter app, the web
/// islands and the desktop app cannot disagree about what "next" is.
enum OrgOnboardingState {
  created('created'),
  verifyingEmail('verifying_email'),
  awaitingBilling('awaiting_billing'),
  invitingMembers('inviting_members'),
  connectingRepository('connecting_repository'),
  firstRunPending('first_run_pending'),
  active('active'),
  suspended('suspended');

  const OrgOnboardingState(this.wireName);

  final String wireName;

  static OrgOnboardingState? parse(String value) {
    for (final state in OrgOnboardingState.values) {
      if (state.wireName == value) {
        return state;
      }
    }
    return null;
  }

  List<OrgOnboardingState> get successors => switch (this) {
        OrgOnboardingState.created => const <OrgOnboardingState>[
            OrgOnboardingState.verifyingEmail,
            OrgOnboardingState.suspended,
          ],
        OrgOnboardingState.verifyingEmail => const <OrgOnboardingState>[
            OrgOnboardingState.awaitingBilling,
            OrgOnboardingState.suspended,
          ],
        OrgOnboardingState.awaitingBilling => const <OrgOnboardingState>[
            OrgOnboardingState.invitingMembers,
            OrgOnboardingState.suspended,
          ],
        OrgOnboardingState.invitingMembers => const <OrgOnboardingState>[
            OrgOnboardingState.connectingRepository,
            OrgOnboardingState.suspended,
          ],
        OrgOnboardingState.connectingRepository => const <OrgOnboardingState>[
            OrgOnboardingState.firstRunPending,
            OrgOnboardingState.suspended,
          ],
        OrgOnboardingState.firstRunPending => const <OrgOnboardingState>[
            OrgOnboardingState.active,
            OrgOnboardingState.suspended,
          ],
        OrgOnboardingState.active => const <OrgOnboardingState>[
            OrgOnboardingState.suspended,
          ],
        OrgOnboardingState.suspended => const <OrgOnboardingState>[
            OrgOnboardingState.active,
          ],
      };

  /// The step a wizard's primary button advances to. Suspension is reachable
  /// but is never the primary next step.
  OrgOnboardingState? get nextStep {
    for (final state in successors) {
      if (state != OrgOnboardingState.suspended) {
        return state;
      }
    }
    return null;
  }

  bool get isTerminal =>
      this == OrgOnboardingState.active || this == OrgOnboardingState.suspended;

  /// Per-mille progress for a bar. `suspended` is zero: stopped, not nearly done.
  int get progressPermille => switch (this) {
        OrgOnboardingState.created => 0,
        OrgOnboardingState.verifyingEmail => 167,
        OrgOnboardingState.awaitingBilling => 333,
        OrgOnboardingState.invitingMembers => 500,
        OrgOnboardingState.connectingRepository => 667,
        OrgOnboardingState.firstRunPending => 833,
        OrgOnboardingState.active => 1000,
        OrgOnboardingState.suspended => 0,
      };

  OrgOnboardingState transition(OrgOnboardingState to) {
    if (!successors.contains(to)) {
      throw IllegalTransition('org_onboarding', wireName, to.wireName);
    }
    return to;
  }
}

enum UserOnboardingState {
  registered('registered'),
  verifyingEmail('verifying_email'),
  choosingAccountKind('choosing_account_kind'),
  joiningOrg('joining_org'),
  connectingRepository('connecting_repository'),
  firstRunPending('first_run_pending'),
  active('active'),
  disabled('disabled');

  const UserOnboardingState(this.wireName);

  final String wireName;

  static UserOnboardingState? parse(String value) {
    for (final state in UserOnboardingState.values) {
      if (state.wireName == value) {
        return state;
      }
    }
    return null;
  }

  List<UserOnboardingState> get successors => switch (this) {
        UserOnboardingState.registered => const <UserOnboardingState>[
            UserOnboardingState.verifyingEmail,
            UserOnboardingState.disabled,
          ],
        UserOnboardingState.verifyingEmail => const <UserOnboardingState>[
            UserOnboardingState.choosingAccountKind,
            UserOnboardingState.disabled,
          ],
        UserOnboardingState.choosingAccountKind => const <UserOnboardingState>[
            UserOnboardingState.joiningOrg,
            UserOnboardingState.connectingRepository,
            UserOnboardingState.disabled,
          ],
        UserOnboardingState.joiningOrg => const <UserOnboardingState>[
            UserOnboardingState.active,
            UserOnboardingState.connectingRepository,
            UserOnboardingState.disabled,
          ],
        UserOnboardingState.connectingRepository => const <UserOnboardingState>[
            UserOnboardingState.firstRunPending,
            UserOnboardingState.disabled,
          ],
        UserOnboardingState.firstRunPending => const <UserOnboardingState>[
            UserOnboardingState.active,
            UserOnboardingState.disabled,
          ],
        UserOnboardingState.active => const <UserOnboardingState>[
            UserOnboardingState.disabled,
          ],
        UserOnboardingState.disabled => const <UserOnboardingState>[
            UserOnboardingState.active,
          ],
      };

  /// `choosingAccountKind` is a fork — join an existing org, or connect your
  /// own repository. A UI that shows one "next" button here picks for the user.
  bool get isFork => this == UserOnboardingState.choosingAccountKind;

  bool get isTerminal =>
      this == UserOnboardingState.active || this == UserOnboardingState.disabled;

  UserOnboardingState transition(UserOnboardingState to) {
    if (!successors.contains(to)) {
      throw IllegalTransition('user_onboarding', wireName, to.wireName);
    }
    return to;
  }
}

/// An optimistic step: the UI moves now and reconciles when the server answers.
/// Immutable, so a store can diff two values.
final class Optimistic<S> {
  const Optimistic(this.confirmed, [this.pending]);

  final S confirmed;
  final S? pending;

  S get displayed => pending ?? confirmed;
  bool get isPending => pending != null;

  /// The server's word wins, whatever was pending.
  Optimistic<S> reconcile(S server) => Optimistic<S>(server);

  Optimistic<S> rollback() => Optimistic<S>(confirmed);

  @override
  bool operator ==(Object other) =>
      other is Optimistic<S> &&
      other.confirmed == confirmed &&
      other.pending == pending;

  @override
  int get hashCode => Object.hash(confirmed, pending);
}

/// Move optimistically, but only along an edge the machine actually has.
Optimistic<OrgOnboardingState> advanceOrg(
  Optimistic<OrgOnboardingState> value,
  OrgOnboardingState to,
) {
  value.displayed.transition(to);
  return Optimistic<OrgOnboardingState>(value.confirmed, to);
}

Optimistic<UserOnboardingState> advanceUser(
  Optimistic<UserOnboardingState> value,
  UserOnboardingState to,
) {
  value.displayed.transition(to);
  return Optimistic<UserOnboardingState>(value.confirmed, to);
}
