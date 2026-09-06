//// Typed API resources.
////
//// These mirror the contracts in `gha-indie-worker-interfaces`. Once
//// `ores-contracts generate` emits a Gleam lane there, this module becomes a
//// re-export of it — see `contracts/README.md`.
////
//// `state` and `conclusion` stay `String` on purpose: the server may add a
//// state before this build is updated, and a client that refuses to render a
//// run list because one run reached a new state is worse than one that renders
//// it as unrecognised. `is_in_flight` treats an unknown state as still moving,
//// which keeps a client polling.

import gleam/list
import gleam/option.{type Option}
import gleam/string

pub type RunLane {
  /// GitHub's own runner scale sets.
  Arc
  /// The independent fixed-profile mirror.
  Independent
}

pub type Job {
  Job(
    id: String,
    name: String,
    ordinal: Int,
    profile: String,
    state: String,
    conclusion: Option(String),
    needs: List(String),
    /// Byte offset a log tail resumes from.
    log_offset: Int,
  )
}

pub type Run {
  Run(
    id: String,
    repository: String,
    /// Always a full 40-hex commit SHA — never a branch or tag.
    revision: String,
    workflow_path: String,
    lane: String,
    state: String,
    conclusion: Option(String),
    queued_at: String,
    started_at: Option(String),
    finished_at: Option(String),
    jobs: List(Job),
  )
}

pub type Worker {
  Worker(
    id: String,
    name: String,
    pool: String,
    architecture: String,
    operating_system: String,
    max_concurrency: Int,
    active_leases: Int,
    last_heartbeat_at: Option(String),
  )
}

pub type ResourceError {
  MalformedResource(field: String, reason: String)
}

pub fn run_states() -> List(String) {
  ["queued", "planning", "dispatching", "running", "completed", "cancelled"]
}

pub fn job_states() -> List(String) {
  ["pending", "ready", "leased", "running", "completed", "cancelled"]
}

pub fn conclusions() -> List(String) {
  ["success", "failure", "cancelled", "timed_out", "unsupported", "skipped"]
}

pub fn lane_to_string(lane: RunLane) -> String {
  case lane {
    Arc -> "arc"
    Independent -> "independent"
  }
}

pub fn is_known_run_state(state: String) -> Bool {
  list.contains(run_states(), state)
}

pub fn is_terminal_run_state(state: String) -> Bool {
  state == "completed" || state == "cancelled"
}

/// A run is in flight unless it reached a terminal state this build recognises.
pub fn is_in_flight(run: Run) -> Bool {
  !is_terminal_run_state(run.state)
}

pub fn free_capacity(worker: Worker) -> Int {
  case worker.max_concurrency - worker.active_leases {
    free if free > 0 -> free
    _ -> 0
  }
}

/// The independent lane refuses branch or tag execution, so a client must not
/// render a run whose revision is not a lowercase 40-hex commit SHA.
pub fn validate_revision(revision: String) -> Result(String, ResourceError) {
  let is_hex =
    string.to_graphemes(revision)
    |> list.all(fn(character) {
      list.contains(
        [
          "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "a", "b", "c", "d",
          "e", "f",
        ],
        character,
      )
    })
  case string.length(revision) == 40 && is_hex {
    True -> Ok(revision)
    False ->
      Error(MalformedResource(
        "revision",
        "is not a lowercase 40-hex commit sha",
      ))
  }
}
