import 'errors.dart';

/// Runtime configuration, assigned by flags-2-env conventions.
///
/// The key names are the strings flags-2-env emits for every language in the
/// fleet, so a Flutter `--dart-define`, a browser blob and a server environment
/// all spell them identically. The lookup is a parameter rather than an ambient
/// read of `Platform.environment`, which is what lets this run unchanged on
/// web (where there is no such thing).
const String apiBaseKey = 'GHA_INDIE_WORKER_API_BASE';
const String wsBaseKey = 'GHA_INDIE_WORKER_WS_BASE';
const String surfaceKey = 'GHA_INDIE_WORKER_SURFACE';
const String clientTimeoutMsKey = 'GHA_INDIE_WORKER_CLIENT_TIMEOUT_MS';
const String sharedAuthBaseKey = 'SHARED_AUTH_BASE';
const String sharedAuthAudienceKey = 'SHARED_AUTH_AUDIENCE';

const List<String> configKeys = <String>[
  apiBaseKey,
  wsBaseKey,
  surfaceKey,
  clientTimeoutMsKey,
  sharedAuthBaseKey,
  sharedAuthAudienceKey,
];

const int defaultTimeoutMs = 30000;
const int _maxTimeoutMs = 600000;

/// Which `indiebuild.dev` host this client is talking to.
enum Surface {
  app('app'),
  user('user'),
  org('org'),
  mobile('mobile'),
  nativeApp('native_app');

  const Surface(this.wireName);

  final String wireName;

  static Surface? parse(String value) {
    for (final surface in Surface.values) {
      if (surface.wireName == value) {
        return surface;
      }
    }
    return null;
  }

  /// Only the B2B surface shows seat and invitation UI.
  bool get isOrganization => this == Surface.org;
}

/// Immutable runtime configuration.
final class RuntimeConfig {
  const RuntimeConfig({
    required this.apiBase,
    required this.wsBase,
    required this.surface,
    required this.timeoutMs,
    this.sharedAuthBase,
    this.sharedAuthAudience,
  });

  /// Build from any key/value lookup. Throws a [ClientError] on bad input.
  factory RuntimeConfig.fromLookup(String? Function(String key) lookup) {
    final apiBase = _present(lookup(apiBaseKey));
    if (apiBase == null) {
      throw const MissingConfig(apiBaseKey);
    }
    _validateOrigin(apiBaseKey, apiBase);

    final explicitWs = _present(lookup(wsBaseKey));
    if (explicitWs != null &&
        !(explicitWs.startsWith('wss://') || explicitWs.startsWith('ws://'))) {
      throw InvalidConfig(wsBaseKey, explicitWs);
    }
    final wsBase = explicitWs ?? _deriveWsBase(apiBase);

    final rawSurface = _present(lookup(surfaceKey));
    final surface =
        rawSurface == null ? Surface.app : Surface.parse(rawSurface);
    if (surface == null) {
      throw InvalidConfig(surfaceKey, rawSurface ?? '');
    }

    var timeoutMs = defaultTimeoutMs;
    final rawTimeout = _present(lookup(clientTimeoutMsKey));
    if (rawTimeout != null) {
      final parsed = int.tryParse(rawTimeout);
      if (parsed == null || parsed <= 0 || parsed > _maxTimeoutMs) {
        throw InvalidConfig(clientTimeoutMsKey, rawTimeout);
      }
      timeoutMs = parsed;
    }

    return RuntimeConfig(
      apiBase: apiBase,
      wsBase: wsBase,
      surface: surface,
      timeoutMs: timeoutMs,
      sharedAuthBase: _present(lookup(sharedAuthBaseKey)),
      sharedAuthAudience: _present(lookup(sharedAuthAudienceKey)),
    );
  }

  final String apiBase;
  final String wsBase;
  final Surface surface;
  final int timeoutMs;
  final String? sharedAuthBase;
  final String? sharedAuthAudience;

  /// Join a path onto the API origin without doubling or dropping the separator.
  String endpoint(String path) {
    final base = apiBase.replaceAll(RegExp(r'/+$'), '');
    return path.startsWith('/') ? '$base$path' : '$base/$path';
  }

  @override
  String toString() =>
      'RuntimeConfig(apiBase: $apiBase, wsBase: $wsBase, surface: ${surface.wireName})';
}

String? _present(String? value) {
  if (value == null) {
    return null;
  }
  final trimmed = value.trim();
  return trimmed.isEmpty ? null : trimmed;
}

void _validateOrigin(String key, String value) {
  if (!(value.startsWith('https://') || value.startsWith('http://'))) {
    throw InvalidConfig(key, value);
  }
  // A base carrying a query or fragment would corrupt every joined path.
  if (value.contains('?') || value.contains('#')) {
    throw InvalidConfig(key, value);
  }
}

/// `https://` becomes `wss://`; `http://` becomes `ws://`. The security level is
/// preserved rather than upgraded — a browser blocks a plaintext socket from an
/// https page anyway, and silently downgrading would be worse than failing.
String _deriveWsBase(String apiBase) {
  if (apiBase.startsWith('https://')) {
    return 'wss://${apiBase.substring('https://'.length)}';
  }
  if (apiBase.startsWith('http://')) {
    return 'ws://${apiBase.substring('http://'.length)}';
  }
  return apiBase;
}
