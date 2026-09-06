import 'errors.dart';

/// Typed API resources.
///
/// These mirror the contracts in `gha-indie-worker-interfaces`. Once
/// `ores-contracts generate` emits `generated/dart/models.dart` there, this file
/// becomes a re-export of it — see `contracts/README.md`.
///
/// `state` and `conclusion` are kept as `String` rather than enums on purpose:
/// the server may add a state before this build is updated, and a client that
/// refuses to render a run list because one run reached a new state is worse
/// than one that renders it as unrecognised. [RunResource.isInFlight] treats an
/// unknown state as still moving, which keeps a client polling.
const List<String> runStates = <String>[
  'queued',
  'planning',
  'dispatching',
  'running',
  'completed',
  'cancelled',
];

const List<String> jobStates = <String>[
  'pending',
  'ready',
  'leased',
  'running',
  'completed',
  'cancelled',
];

const List<String> conclusions = <String>[
  'success',
  'failure',
  'cancelled',
  'timed_out',
  'unsupported',
  'skipped',
];

final RegExp _commitSha = RegExp(r'^[0-9a-f]{40}$');

bool isTerminalRunState(String state) =>
    state == 'completed' || state == 'cancelled';

final class JobResource {
  const JobResource({
    required this.id,
    required this.name,
    required this.ordinal,
    required this.profile,
    required this.state,
    required this.needs,
    required this.logOffset,
    this.conclusion,
  });

  factory JobResource.fromJson(Object? value) {
    final raw = _asObject(value, 'job');
    final needsValue = raw['needs'];
    final needs = <String>[];
    if (needsValue != null) {
      if (needsValue is! List) {
        throw const MalformedResponse('needs', 'is not an array');
      }
      for (var index = 0; index < needsValue.length; index += 1) {
        final entry = needsValue[index];
        if (entry is! String) {
          throw MalformedResponse('needs[$index]', 'is not a string');
        }
        needs.add(entry);
      }
    }
    return JobResource(
      id: _asString(raw, 'id'),
      name: _asString(raw, 'name'),
      ordinal: _asInt(raw, 'ordinal'),
      profile: _asString(raw, 'profile'),
      state: _asString(raw, 'state'),
      conclusion: _optionalString(raw, 'conclusion'),
      needs: needs,
      logOffset: raw['logOffset'] == null ? 0 : _asInt(raw, 'logOffset'),
    );
  }

  final String id;
  final String name;
  final int ordinal;
  final String profile;
  final String state;
  final String? conclusion;
  final List<String> needs;

  /// Byte offset a log tail resumes from.
  final int logOffset;
}

final class RunResource {
  const RunResource({
    required this.id,
    required this.repository,
    required this.revision,
    required this.workflowPath,
    required this.lane,
    required this.state,
    required this.queuedAt,
    required this.jobs,
    this.conclusion,
    this.startedAt,
    this.finishedAt,
  });

  factory RunResource.fromJson(Object? value) {
    final raw = _asObject(value, 'run');
    final revision = _asString(raw, 'revision');
    if (!_commitSha.hasMatch(revision)) {
      throw const MalformedResponse(
        'revision',
        'is not a lowercase 40-hex commit sha',
      );
    }
    final jobsValue = raw['jobs'];
    final jobs = <JobResource>[];
    if (jobsValue != null) {
      if (jobsValue is! List) {
        throw const MalformedResponse('jobs', 'is not an array');
      }
      for (final entry in jobsValue) {
        jobs.add(JobResource.fromJson(entry));
      }
    }
    return RunResource(
      id: _asString(raw, 'id'),
      repository: _asString(raw, 'repository'),
      revision: revision,
      workflowPath: _asString(raw, 'workflowPath'),
      lane: _asString(raw, 'lane'),
      state: _asString(raw, 'state'),
      conclusion: _optionalString(raw, 'conclusion'),
      queuedAt: _asString(raw, 'queuedAt'),
      startedAt: _optionalString(raw, 'startedAt'),
      finishedAt: _optionalString(raw, 'finishedAt'),
      jobs: jobs,
    );
  }

  final String id;
  final String repository;

  /// Always a full 40-hex commit SHA — never a branch or tag.
  final String revision;
  final String workflowPath;
  final String lane;
  final String state;
  final String? conclusion;
  final String queuedAt;
  final String? startedAt;
  final String? finishedAt;
  final List<JobResource> jobs;

  bool get isInFlight => !isTerminalRunState(state);
}

final class WorkerResource {
  const WorkerResource({
    required this.id,
    required this.name,
    required this.pool,
    required this.architecture,
    required this.operatingSystem,
    required this.maxConcurrency,
    required this.activeLeases,
    this.lastHeartbeatAt,
  });

  final String id;
  final String name;
  final String pool;
  final String architecture;
  final String operatingSystem;
  final int maxConcurrency;
  final int activeLeases;
  final String? lastHeartbeatAt;

  int get freeCapacity {
    final free = maxConcurrency - activeLeases;
    return free < 0 ? 0 : free;
  }
}

Map<String, Object?> _asObject(Object? value, String field) {
  if (value is! Map<String, Object?>) {
    throw MalformedResponse(field, 'is not an object');
  }
  return value;
}

String _asString(Map<String, Object?> raw, String field) {
  final value = raw[field];
  if (value is! String || value.isEmpty) {
    throw MalformedResponse(field, 'is missing or not a non-empty string');
  }
  return value;
}

String? _optionalString(Map<String, Object?> raw, String field) {
  final value = raw[field];
  if (value == null) {
    return null;
  }
  if (value is! String) {
    throw MalformedResponse(field, 'is not a string');
  }
  return value;
}

int _asInt(Map<String, Object?> raw, String field) {
  final value = raw[field];
  if (value is! int) {
    throw MalformedResponse(field, 'is missing or not an integer');
  }
  return value;
}
