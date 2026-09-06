/// Typed errors for the client core.
///
/// Sealed so a caller's `switch` is exhaustive and a new failure mode cannot be
/// added without every handler being told about it.
sealed class ClientError implements Exception {
  const ClientError();

  String get message;

  @override
  String toString() => '$runtimeType: $message';
}

final class MissingConfig extends ClientError {
  const MissingConfig(this.key);

  final String key;

  @override
  String get message => 'missing runtime configuration key $key';
}

final class InvalidConfig extends ClientError {
  const InvalidConfig(this.key, this.reason);

  final String key;
  final String reason;

  @override
  String get message => 'invalid runtime configuration for $key: $reason';
}

final class InvalidCursor extends ClientError {
  const InvalidCursor(this.reason);

  final String reason;

  @override
  String get message => 'cursor is not a cursor this API issued: $reason';
}

final class PageLimitOutOfRange extends ClientError {
  const PageLimitOutOfRange(this.got, this.max);

  final int got;
  final int max;

  @override
  String get message => 'page limit $got is outside 1..=$max';
}

final class InvalidSigningKey extends ClientError {
  const InvalidSigningKey(this.reason);

  final String reason;

  @override
  String get message => 'signing key is unusable: $reason';
}

/// Carries nothing about *why*. Explaining a signature failure is how signature
/// oracles are built.
final class SignatureMismatch extends ClientError {
  const SignatureMismatch();

  @override
  String get message => 'signature did not verify';
}

final class IllegalTransition extends ClientError {
  const IllegalTransition(this.machine, this.from, this.to);

  final String machine;
  final String from;
  final String to;

  @override
  String get message => 'illegal $machine transition: $from -> $to';
}

final class MalformedResponse extends ClientError {
  const MalformedResponse(this.field, this.reason);

  final String field;
  final String reason;

  @override
  String get message => 'malformed response: $field $reason';
}
