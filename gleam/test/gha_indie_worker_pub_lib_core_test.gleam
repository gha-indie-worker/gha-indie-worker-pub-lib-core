import gha_indie_worker_pub_lib_core
import gha_indie_worker_pub_lib_core/config
import gha_indie_worker_pub_lib_core/onboarding
import gha_indie_worker_pub_lib_core/resources
import gleam/list
import gleam/option.{None, Some}
import gleeunit
import gleeunit/should

pub fn main() {
  gleeunit.main()
}

fn lookup_from(pairs: List(#(String, String))) -> fn(String) -> option.Option(String) {
  fn(key) {
    case list.key_find(pairs, key) {
      Ok(value) -> Some(value)
      Error(_) -> None
    }
  }
}

pub fn summary_links_every_module_test() {
  gha_indie_worker_pub_lib_core.summary()
  |> should.equal(
    "gha_indie_worker_pub_lib_core 0.1.0: 8 org states, 5 surfaces, 6 run states",
  )
}

pub fn minimal_config_derives_the_websocket_origin_test() {
  let assert Ok(loaded) =
    config.load(lookup_from([#(config.api_base_key, "https://api.indiebuild.dev")]))
  loaded.ws_base
  |> should.equal("wss://api.indiebuild.dev")
  loaded.surface
  |> should.equal(config.App)
  loaded.timeout_ms
  |> should.equal(config.default_timeout_ms)
  loaded.shared_auth_base
  |> should.equal(None)
}

pub fn plaintext_base_never_upgrades_test() {
  let assert Ok(loaded) =
    config.load(lookup_from([#(config.api_base_key, "http://127.0.0.1:8080")]))
  loaded.ws_base
  |> should.equal("ws://127.0.0.1:8080")
}

pub fn the_api_base_is_required_test() {
  config.load(lookup_from([]))
  |> should.equal(Error(config.MissingConfig(config.api_base_key)))
}

pub fn a_base_without_a_scheme_is_rejected_test() {
  config.load(lookup_from([#(config.api_base_key, "api.indiebuild.dev")]))
  |> should.be_error
}

pub fn a_base_with_a_query_is_rejected_test() {
  config.load(lookup_from([#(config.api_base_key, "https://api.indiebuild.dev?a=1")]))
  |> should.be_error
}

pub fn an_absurd_timeout_is_rejected_test() {
  config.load(
    lookup_from([
      #(config.api_base_key, "https://api.indiebuild.dev"),
      #(config.client_timeout_ms_key, "0"),
    ]),
  )
  |> should.be_error
}

pub fn endpoint_joins_without_doubling_the_separator_test() {
  let assert Ok(loaded) =
    config.load(lookup_from([#(config.api_base_key, "https://api.indiebuild.dev/")]))
  config.endpoint(loaded, "/v1/runs")
  |> should.equal("https://api.indiebuild.dev/v1/runs")
  config.endpoint(loaded, "v1/runs")
  |> should.equal("https://api.indiebuild.dev/v1/runs")
}

pub fn only_the_org_surface_shows_seat_ui_test() {
  config.surfaces()
  |> list.filter(config.is_organization)
  |> should.equal([config.Org])
}

pub fn every_surface_round_trips_test() {
  config.surfaces()
  |> list.all(fn(surface) {
    config.surface_from_string(config.surface_to_string(surface)) == Ok(surface)
  })
  |> should.be_true
}

pub fn every_org_state_round_trips_test() {
  onboarding.org_states()
  |> list.all(fn(state) {
    onboarding.org_from_string(onboarding.org_to_string(state)) == Ok(state)
  })
  |> should.be_true
}

pub fn an_unknown_state_is_an_error_not_a_default_test() {
  onboarding.org_from_string("nope")
  |> should.equal(Error(onboarding.UnknownState("nope")))
}

pub fn skipping_billing_is_rejected_test() {
  onboarding.org_transition(onboarding.Created, onboarding.OrgActive)
  |> should.equal(
    Error(onboarding.IllegalTransition("org_onboarding", "created", "active")),
  )
}

pub fn the_org_happy_path_is_six_steps_test() {
  walk(onboarding.Created, 0)
  |> should.equal(6)
}

fn walk(state: onboarding.OrgState, steps: Int) -> Int {
  case state == onboarding.OrgActive || steps > 10 {
    True -> steps
    False ->
      case onboarding.org_next_step(state) {
        Error(_) -> steps
        Ok(next) ->
          case onboarding.org_transition(state, next) {
            Ok(moved) -> walk(moved, steps + 1)
            Error(_) -> steps
          }
      }
  }
}

pub fn suspension_is_never_the_primary_next_step_test() {
  onboarding.org_states()
  |> list.all(fn(state) {
    onboarding.org_next_step(state) != Ok(onboarding.Suspended)
  })
  |> should.be_true
}

pub fn progress_is_zero_when_suspended_test() {
  onboarding.org_progress_permille(onboarding.Suspended)
  |> should.equal(0)
  onboarding.org_progress_permille(onboarding.OrgActive)
  |> should.equal(1000)
}

pub fn the_user_machine_forks_at_the_account_kind_choice_test() {
  onboarding.user_is_fork(onboarding.ChoosingAccountKind)
  |> should.be_true
  onboarding.user_successors(onboarding.ChoosingAccountKind)
  |> list.contains(onboarding.JoiningOrg)
  |> should.be_true
  onboarding.user_successors(onboarding.ChoosingAccountKind)
  |> list.contains(onboarding.UserConnectingRepository)
  |> should.be_true
}

pub fn a_revision_must_be_a_lowercase_commit_sha_test() {
  resources.validate_revision("0123456789abcdef0123456789abcdef01234567")
  |> should.be_ok
  resources.validate_revision("main")
  |> should.be_error
  resources.validate_revision("0123456789ABCDEF0123456789ABCDEF01234567")
  |> should.be_error
}

pub fn an_unknown_run_state_still_counts_as_in_flight_test() {
  let run =
    resources.Run(
      id: "r1",
      repository: "gha-indie-worker/x",
      revision: "0123456789abcdef0123456789abcdef01234567",
      workflow_path: ".github/workflows/ci.yml",
      lane: "independent",
      state: "quarantined",
      conclusion: None,
      queued_at: "2026-09-05T12:00:00Z",
      started_at: None,
      finished_at: None,
      jobs: [],
    )
  resources.is_in_flight(run)
  |> should.be_true
  resources.is_known_run_state(run.state)
  |> should.be_false
}

pub fn worker_capacity_never_goes_negative_test() {
  resources.Worker(
    id: "w1",
    name: "arm-1",
    pool: "shared",
    architecture: "arm64",
    operating_system: "linux",
    max_concurrency: 2,
    active_leases: 5,
    last_heartbeat_at: None,
  )
  |> resources.free_capacity
  |> should.equal(0)
}
