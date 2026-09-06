//! Retry policy for public clients.
//!
//! Every SDK we ship — TypeScript in a browser, Dart in the Flutter app, Rust in the CLI and the
//! desktop app — has to decide the same three things when a request fails: *should* I retry, *when*,
//! and *how many times*. Deciding it once here means a browser tab and a runner behave identically,
//! and it means the policy is testable without a network.
//!
//! Two properties this gets right that hand-rolled retry loops usually do not:
//!
//! * **Full jitter, not "exponential plus a little noise".** Every client that retries a failed
//!   deploy at 1s, 2s, 4s retries *together*, and the synchronized second wave is what keeps a
//!   recovering server down. Full jitter draws uniformly from `[0, backoff]`, which spreads the
//!   herd across the whole window.
//! * **`Retry-After` always wins.** When the server says when to come back, guessing is worse than
//!   obeying — including obeying a value *longer* than our own ceiling.

use core::time::Duration;

/// How aggressively a client retries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Policy {
    pub base: Duration,
    pub ceiling: Duration,
    /// Attempts *after* the first. `max_retries: 0` means try once and stop.
    pub max_retries: u32,
    /// Cap on a server-supplied `Retry-After`. A hostile or misconfigured server should not be
    /// able to park a client for an hour; beyond this we give up rather than wait.
    pub max_honored_retry_after: Duration,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            base: Duration::from_millis(250),
            ceiling: Duration::from_secs(30),
            max_retries: 5,
            max_honored_retry_after: Duration::from_secs(120),
        }
    }
}

/// What a client should do with a failed attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Decision {
    /// Wait this long, then try again.
    RetryAfter(Duration),
    /// Stop. Either the failure is not retryable, or the budget is spent.
    GiveUp(Reason),
}

/// Why a client stopped retrying — surfaced to the caller, because "it failed" and "it failed and
/// we stopped trying because the request itself is wrong" are different for a user.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reason {
    /// The status will never succeed on repeat (400, 401, 403, 404, 422…).
    NotRetryable,
    /// Out of attempts.
    Exhausted,
    /// The server asked for a longer wait than this client will honor.
    RetryAfterTooLong,
}

/// The outcome of one attempt, as a client sees it before deciding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// A transport failure: DNS, connect, TLS, reset, timeout. Always retryable — the request may
    /// never have reached us.
    Transport,
    /// An HTTP status, plus `Retry-After` in seconds when the server sent one.
    Status { code: u16, retry_after_seconds: Option<u64> },
}

/// Which statuses are worth repeating.
///
/// 408 and 425 are transient by definition; 429 is explicitly "try again"; 5xx except 501 and 505
/// may be a single bad instance. Everything else — including 400, 401, 403, 404 and 422 — will
/// fail identically on repeat, and retrying it only spends the user's battery.
#[must_use]
pub const fn is_retryable(code: u16) -> bool {
    matches!(code, 408 | 425 | 429) || (code >= 500 && code != 501 && code != 505)
}

/// Decide what to do after `attempt` failures (1 = the first attempt just failed).
///
/// `random_unit` is a value in `[0, 1)` supplied by the caller — the jitter source is an input, so
/// this stays a pure function and the tests can pin both extremes.
#[must_use]
pub fn decide(policy: Policy, attempt: u32, outcome: Outcome, random_unit: f64) -> Decision {
    if let Outcome::Status { code, .. } = outcome {
        if !is_retryable(code) {
            return Decision::GiveUp(Reason::NotRetryable);
        }
    }
    if attempt > policy.max_retries {
        return Decision::GiveUp(Reason::Exhausted);
    }
    // A server that tells us when to come back knows better than our exponential guess.
    if let Outcome::Status { retry_after_seconds: Some(seconds), .. } = outcome {
        let requested = Duration::from_secs(seconds);
        return if requested > policy.max_honored_retry_after {
            Decision::GiveUp(Reason::RetryAfterTooLong)
        } else {
            Decision::RetryAfter(requested)
        };
    }
    Decision::RetryAfter(full_jitter(policy, attempt, random_unit))
}

/// Exponential backoff with full jitter: uniform over `[0, min(ceiling, base·2^(attempt-1))]`.
#[must_use]
pub fn full_jitter(policy: Policy, attempt: u32, random_unit: f64) -> Duration {
    let exponent = attempt.saturating_sub(1).min(31);
    let scaled = policy.base.saturating_mul(1_u32 << exponent);
    let window = if scaled > policy.ceiling { policy.ceiling } else { scaled };
    let unit = if random_unit.is_finite() { random_unit.clamp(0.0, 1.0) } else { 0.0 };
    Duration::from_secs_f64(window.as_secs_f64() * unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_transient_statuses_are_retried() {
        for code in [408, 425, 429, 500, 502, 503, 504, 507, 599] {
            assert!(is_retryable(code), "{code} should be retryable");
        }
        for code in [200, 201, 301, 400, 401, 403, 404, 409, 422, 451, 501, 505] {
            assert!(!is_retryable(code), "{code} should not be retryable");
        }
    }

    #[test]
    fn a_client_error_stops_immediately_however_much_budget_is_left() {
        let decision = decide(
            Policy::default(),
            1,
            Outcome::Status { code: 403, retry_after_seconds: Some(1) },
            0.5,
        );
        assert_eq!(decision, Decision::GiveUp(Reason::NotRetryable));
    }

    #[test]
    fn transport_failures_are_always_retryable_until_the_budget_runs_out() {
        let policy = Policy { max_retries: 2, ..Policy::default() };
        assert!(matches!(decide(policy, 1, Outcome::Transport, 0.5), Decision::RetryAfter(_)));
        assert!(matches!(decide(policy, 2, Outcome::Transport, 0.5), Decision::RetryAfter(_)));
        assert_eq!(decide(policy, 3, Outcome::Transport, 0.5), Decision::GiveUp(Reason::Exhausted));
    }

    #[test]
    fn retry_after_beats_our_own_guess_even_when_it_is_longer() {
        let policy = Policy { ceiling: Duration::from_secs(2), ..Policy::default() };
        let decision =
            decide(policy, 1, Outcome::Status { code: 429, retry_after_seconds: Some(45) }, 0.0);
        assert_eq!(decision, Decision::RetryAfter(Duration::from_secs(45)));
    }

    #[test]
    fn an_absurd_retry_after_is_refused_rather_than_obeyed() {
        let decision =
            decide(Policy::default(), 1, Outcome::Status { code: 503, retry_after_seconds: Some(3600) }, 0.0);
        assert_eq!(decision, Decision::GiveUp(Reason::RetryAfterTooLong));
    }

    #[test]
    fn full_jitter_spans_the_whole_window_and_respects_the_ceiling() {
        let policy = Policy {
            base: Duration::from_millis(100),
            ceiling: Duration::from_secs(1),
            ..Policy::default()
        };
        // attempt 1 -> window 100ms; the draw covers [0, window].
        assert_eq!(full_jitter(policy, 1, 0.0), Duration::ZERO);
        assert_eq!(full_jitter(policy, 1, 1.0), Duration::from_millis(100));
        // attempt 3 -> 400ms; attempt 8 would be 12.8s but the ceiling holds it at 1s.
        assert_eq!(full_jitter(policy, 3, 1.0), Duration::from_millis(400));
        assert_eq!(full_jitter(policy, 8, 1.0), Duration::from_secs(1));
        assert_eq!(full_jitter(policy, 40, 1.0), Duration::from_secs(1), "no overflow at large attempts");
    }

    #[test]
    fn jitter_is_not_merely_decorative() {
        // The point of full jitter: two clients failing at the same instant must not come back
        // together. Different draws must give different delays across the whole window.
        let policy = Policy { base: Duration::from_secs(1), ..Policy::default() };
        let low = full_jitter(policy, 4, 0.01);
        let high = full_jitter(policy, 4, 0.99);
        assert!(high > low * 10, "expected a wide spread, got {low:?} and {high:?}");
    }

    #[test]
    fn a_nonsense_random_input_cannot_produce_a_nonsense_delay() {
        let policy = Policy::default();
        for unit in [f64::NAN, f64::INFINITY, -5.0, 12.0] {
            let delay = full_jitter(policy, 3, unit);
            assert!(delay <= policy.ceiling, "unit {unit} produced {delay:?}");
        }
    }
}
