import 'dart:convert';

import 'package:crypto/crypto.dart';

import 'errors.dart';

/// Request signing — byte-identical to the Rust and TypeScript cores' `GIW1`
/// scheme.
///
/// The canonical string is six newline-separated lines:
///
/// ```text
/// GIW1
/// <METHOD uppercased>
/// <path, no query>
/// <query string, or empty>
/// <unix seconds>
/// <lowercase hex sha256 of the body>
/// ```
///
/// None of the covered fields may contain a newline, so two different requests
/// cannot produce the same canonical string.
const String signingScheme = 'GIW1';
const int maxSkewSeconds = 300;

const String signatureHeader = 'x-giw-signature';
const String timestampHeader = 'x-giw-timestamp';
const String keyIdHeader = 'x-giw-key-id';

/// What the host transport attaches to the outgoing request.
final class SignedRequest {
  const SignedRequest({
    required this.keyId,
    required this.timestamp,
    required this.signature,
    required this.canonical,
  });

  final String keyId;
  final int timestamp;

  /// Lowercase hex HMAC-SHA256.
  final String signature;

  /// Exactly what was signed. Contains no secret.
  final String canonical;

  Map<String, String> get headers => <String, String>{
        keyIdHeader: keyId,
        timestampHeader: '$timestamp',
        signatureHeader: signature,
      };
}

/// Build the canonical string. Public because the server implements the same
/// function and the two must agree byte for byte.
String canonicalString({
  required String method,
  required String path,
  required String query,
  required int timestamp,
  required List<int> body,
}) {
  final normalizedQuery = query.startsWith('?') ? query.substring(1) : query;
  final digest = sha256.convert(body).toString();
  return <String>[
    signingScheme,
    method.toUpperCase(),
    path,
    normalizedQuery,
    '$timestamp',
    digest,
  ].join('\n');
}

SignedRequest signRequest({
  required String keyId,
  required List<int> key,
  required String method,
  required String path,
  required int timestamp,
  String query = '',
  List<int> body = const <int>[],
}) {
  if (key.isEmpty || keyId.isEmpty) {
    throw const InvalidSigningKey('key and key id must both be non-empty');
  }
  for (final field in <String>[method, path, query, keyId]) {
    if (field.contains('\n') || field.contains('\r')) {
      throw const InvalidSigningKey('a signed field may not contain a newline');
    }
  }
  final canonical = canonicalString(
    method: method,
    path: path,
    query: query,
    timestamp: timestamp,
    body: body,
  );
  final signature =
      Hmac(sha256, key).convert(utf8.encode(canonical)).toString();
  return SignedRequest(
    keyId: keyId,
    timestamp: timestamp,
    signature: signature,
    canonical: canonical,
  );
}

/// Verify a signature. The skew window is checked first — an expired signature
/// is not worth comparing — and the comparison itself is time-independent.
bool verifySignature({
  required List<int> key,
  required String canonical,
  required String presented,
  required int timestamp,
  required int now,
}) {
  if ((now - timestamp).abs() > maxSkewSeconds) {
    return false;
  }
  if (key.isEmpty) {
    return false;
  }
  final expected = Hmac(sha256, key).convert(utf8.encode(canonical)).toString();
  return timingSafeEqualHex(expected, presented);
}

/// Compare two hex strings without leaking where they differ. The length check
/// is non-secret: the expected length is a constant of the scheme.
bool timingSafeEqualHex(String expected, String presented) {
  if (expected.length != presented.length) {
    return false;
  }
  var difference = 0;
  for (var index = 0; index < expected.length; index += 1) {
    difference |= expected.codeUnitAt(index) ^ presented.codeUnitAt(index);
  }
  return difference == 0;
}
