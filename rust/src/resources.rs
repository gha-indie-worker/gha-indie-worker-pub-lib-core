//! Typed API resources.
//!
//! These mirror the contracts in `gha-indie-worker-interfaces`. Once
//! `ores-contracts generate` emits `generated/rust/types.rs` there, this module
//! becomes a thin re-export of it and these definitions are deleted — see
//! `contracts/README.md`. Until then they are hand-written so client work is not
//! blocked, and `contracts/expected-resources.json` records the field names so
//! the swap is mechanical rather than archaeological.
//!
//! Deserialization is deliberately strict about *shape* and lenient about
//! *values a future server may add*: an unknown `state` string is preserved as
//! [`Unknown`] rather than failing the whole response, because a client that
//! refuses to render a run list because one run reached a new state is worse
//! than one that renders it as unrecognised.

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::error::{ClientError, Result};

/// A value from a closed set the server may extend before this client is
/// updated. Round-trips unchanged, so echoing a resource back does not corrupt
/// a field this build did not understand.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Extensible<T> {
    Known(T),
    Unknown(String),
}

impl<T> Extensible<T> {
    pub const fn known(&self) -> Option<&T> {
        match self {
            Self::Known(value) => Some(value),
            Self::Unknown(_) => None,
        }
    }

    pub const fn is_known(&self) -> bool {
        matches!(self, Self::Known(_))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Queued,
    Planning,
    Dispatching,
    Running,
    Completed,
    Cancelled,
}

impl RunState {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Pending,
    Ready,
    Leased,
    Running,
    Completed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Conclusion {
    Success,
    Failure,
    Cancelled,
    TimedOut,
    Unsupported,
    Skipped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunLane {
    /// GitHub's own runner scale sets.
    Arc,
    /// The independent fixed-profile mirror.
    Independent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunResource {
    pub id: String,
    pub repository: String,
    /// Always a full 40-hex commit SHA — never a branch or tag.
    pub revision: String,
    pub workflow_path: String,
    pub lane: Extensible<RunLane>,
    pub state: Extensible<RunState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<Extensible<Conclusion>>,
    pub queued_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default)]
    pub jobs: Vec<JobResource>,
}

impl RunResource {
    /// A run is still moving unless it reached a terminal state this build
    /// recognises. An unknown state is treated as *in flight*, which keeps a
    /// client polling rather than declaring a run finished it cannot read.
    pub fn is_in_flight(&self) -> bool {
        match &self.state {
            Extensible::Known(state) => !state.is_terminal(),
            Extensible::Unknown(_) => true,
        }
    }

    /// Reject a run whose revision is not a commit SHA. The independent lane
    /// refuses branch/tag execution, and a client should not render one either.
    pub fn validate(&self) -> Result<()> {
        if self.revision.len() != 40
            || !self
                .revision
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(ClientError::MalformedResponse {
                reason: alloc::format!("revision {:?} is not a 40-hex commit sha", self.revision),
            });
        }
        if self.id.is_empty() || self.repository.is_empty() {
            return Err(ClientError::MalformedResponse {
                reason: String::from("run is missing an id or repository"),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobResource {
    pub id: String,
    pub name: String,
    pub ordinal: u32,
    pub profile: String,
    pub state: Extensible<JobState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<Extensible<Conclusion>>,
    #[serde(default)]
    pub needs: Vec<String>,
    /// Byte offset a log tail should resume from.
    #[serde(default)]
    pub log_offset: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerResource {
    pub id: String,
    pub name: String,
    pub pool: String,
    pub architecture: String,
    pub operating_system: String,
    pub max_concurrency: u32,
    pub active_leases: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_heartbeat_at: Option<String>,
}

impl WorkerResource {
    pub const fn free_capacity(&self) -> u32 {
        self.max_concurrency.saturating_sub(self.active_leases)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

    fn run_json(state: &str) -> alloc::string::String {
        alloc::format!(
            r#"{{"id":"r1","repository":"gha-indie-worker/x","revision":"{SHA}",
                 "workflow_path":".github/workflows/ci.yml","lane":"independent",
                 "state":"{state}","queued_at":"2026-09-05T12:00:00Z"}}"#
        )
    }

    #[test]
    fn a_known_state_deserializes_into_the_enum() {
        let run: RunResource = serde_json::from_str(&run_json("running")).expect("parse");
        assert_eq!(run.state, Extensible::Known(RunState::Running));
        assert!(run.state.is_known());
        assert!(run.is_in_flight());
        assert!(run.jobs.is_empty());
    }

    #[test]
    fn an_unknown_state_is_preserved_rather_than_failing_the_response() {
        let run: RunResource = serde_json::from_str(&run_json("quarantined")).expect("parse");
        assert_eq!(
            run.state,
            Extensible::Unknown(alloc::string::String::from("quarantined"))
        );
        assert!(run.state.known().is_none());
        // An unrecognised state must keep the client polling.
        assert!(run.is_in_flight());
    }

    #[test]
    fn an_unknown_value_round_trips_unchanged() {
        let run: RunResource = serde_json::from_str(&run_json("quarantined")).expect("parse");
        let re_encoded = serde_json::to_string(&run).expect("serialize");
        assert!(re_encoded.contains("\"quarantined\""));
    }

    #[test]
    fn a_terminal_run_is_not_in_flight() {
        let run: RunResource = serde_json::from_str(&run_json("completed")).expect("parse");
        assert!(!run.is_in_flight());
    }

    #[test]
    fn absent_optional_fields_are_not_re_emitted() {
        let run: RunResource = serde_json::from_str(&run_json("queued")).expect("parse");
        let re_encoded = serde_json::to_string(&run).expect("serialize");
        assert!(!re_encoded.contains("conclusion"));
        assert!(!re_encoded.contains("started_at"));
    }

    #[test]
    fn a_revision_that_is_not_a_commit_sha_is_rejected() {
        let mut run: RunResource = serde_json::from_str(&run_json("queued")).expect("parse");
        run.validate().expect("a 40-hex revision is valid");

        run.revision = alloc::string::String::from("main");
        assert!(run.validate().is_err());

        run.revision = alloc::string::String::from(SHA).to_uppercase();
        assert!(run.validate().is_err(), "hex must be lowercase");

        run.revision = alloc::string::String::from(SHA);
        run.id = alloc::string::String::new();
        assert!(run.validate().is_err());
    }

    #[test]
    fn worker_capacity_never_goes_negative() {
        let worker = WorkerResource {
            id: alloc::string::String::from("w1"),
            name: alloc::string::String::from("arm-1"),
            pool: alloc::string::String::from("shared"),
            architecture: alloc::string::String::from("arm64"),
            operating_system: alloc::string::String::from("linux"),
            max_concurrency: 2,
            active_leases: 5,
            last_heartbeat_at: None,
        };
        assert_eq!(worker.free_capacity(), 0);
    }
}
