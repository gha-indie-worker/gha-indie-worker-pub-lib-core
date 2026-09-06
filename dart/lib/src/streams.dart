import 'dart:async';

import 'resources.dart';

/// RxDart-friendly stream helpers.
///
/// RxDart is deliberately **not** a dependency: everything here is
/// `dart:async`, so a consumer that wants `BehaviorSubject`, `switchMap` or
/// `debounceTime` composes them on top, and a consumer that does not want RxDart
/// pays nothing. Every function below is a plain `Stream` transformer, which is
/// exactly what RxDart operators accept.

/// Collapse a stream of run snapshots to one event per *meaningful* change.
///
/// A run poller emits the same run many times. Rendering each one repaints a
/// list for nothing, and — worse — makes it impossible to tell "the run moved"
/// from "we polled again". Two snapshots are the same when the run's state,
/// conclusion and per-job states all match.
Stream<RunResource> distinctRunChanges(Stream<RunResource> source) {
  return source.distinct((previous, next) => _runFingerprint(previous) == _runFingerprint(next));
}

String _runFingerprint(RunResource run) {
  final jobs = run.jobs
      .map((job) => '${job.id}:${job.state}:${job.conclusion ?? ""}:${job.logOffset}')
      .join(',');
  return '${run.id}|${run.state}|${run.conclusion ?? ""}|$jobs';
}

/// Stop a run stream once the run reaches a terminal state, emitting that final
/// snapshot first.
///
/// `takeWhile` would drop the terminal event, which is the one the UI most needs
/// — so this emits it and *then* closes.
Stream<RunResource> untilTerminal(Stream<RunResource> source) async* {
  await for (final run in source) {
    yield run;
    if (!run.isInFlight) {
      return;
    }
  }
}

/// Re-emit the latest value at most once per [window].
///
/// Log tails arrive faster than a UI can paint. This keeps the newest value and
/// drops the ones in between rather than queuing them, so a slow consumer falls
/// behind in *staleness*, never in memory.
Stream<T> sampleLatest<T>(Stream<T> source, Duration window) {
  late StreamController<T> controller;
  StreamSubscription<T>? subscription;
  Timer? timer;
  T? pending;
  var hasPending = false;
  var sourceDone = false;

  void flush() {
    if (hasPending) {
      controller.add(pending as T);
      hasPending = false;
      pending = null;
    }
    if (sourceDone) {
      timer?.cancel();
      timer = null;
      unawaited(controller.close());
    }
  }

  controller = StreamController<T>(
    onListen: () {
      subscription = source.listen(
        (value) {
          pending = value;
          hasPending = true;
        },
        onError: controller.addError,
        onDone: () {
          sourceDone = true;
          flush();
        },
      );
      timer = Timer.periodic(window, (_) => flush());
    },
    onCancel: () async {
      timer?.cancel();
      timer = null;
      await subscription?.cancel();
      subscription = null;
    },
  );

  return controller.stream;
}

/// Accumulate a log tail's byte offset, emitting `(offset, chunk)` pairs so a
/// consumer can resume from the last offset it actually rendered.
Stream<({int offset, String chunk})> withLogOffsets(
  Stream<String> chunks, {
  int startOffset = 0,
}) async* {
  var offset = startOffset;
  await for (final chunk in chunks) {
    yield (offset: offset, chunk: chunk);
    offset += chunk.length;
  }
}
