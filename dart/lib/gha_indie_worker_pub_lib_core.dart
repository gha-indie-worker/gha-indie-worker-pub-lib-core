/// Public client-side core for GHA Indie Worker.
///
/// The Dart port of the same contract the Rust and TypeScript cores implement:
/// the same runtime-configuration keys, the same `GIW1` signing scheme, the
/// same base64url cursor format, and the same onboarding state machines.
///
/// There is no Flutter dependency here on purpose — this package is used by the
/// Flutter app, by `gha-indie-worker-cli`'s Dart surface and by plain Dart
/// tooling. Widgets belong in `gha-indie-worker-flutter`.
library;

export 'src/config.dart';
export 'src/errors.dart';
export 'src/onboarding.dart';
export 'src/pagination.dart';
export 'src/resources.dart';
export 'src/signing.dart';
export 'src/streams.dart';
