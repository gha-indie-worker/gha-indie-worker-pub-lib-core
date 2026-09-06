import 'dart:convert';

import 'errors.dart';

/// Cursor pagination — the same wire format the Rust and TypeScript cores use.
const int maxPageLimit = 100;
const int defaultPageLimit = 25;

const String _cursorVersion = 'v1';
const String _alphabet =
    'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_';

/// An opaque continuation token: base64url of `v1:<sortKey>:<id>`.
final class Cursor {
  Cursor(this.sortKey, this.id) {
    if (sortKey.isEmpty || id.isEmpty) {
      throw const InvalidCursor('a cursor needs both a sort key and an id');
    }
    if (sortKey.contains(':') || id.contains(':')) {
      throw const InvalidCursor("a cursor field may not contain ':'");
    }
  }

  /// Parse a cursor this API issued. Anything else throws.
  factory Cursor.decode(String encoded) {
    final bytes = base64UrlDecodeStrict(encoded);
    final String text;
    try {
      text = const Utf8Decoder().convert(bytes);
    } on FormatException {
      throw const InvalidCursor('payload is not valid UTF-8');
    }
    final parts = text.split(':');
    if (parts.length < 3 || parts[0] != _cursorVersion) {
      throw const InvalidCursor('unrecognised payload');
    }
    return Cursor(parts[1], parts.sublist(2).join(':'));
  }

  final String sortKey;
  final String id;

  String encode() =>
      base64UrlEncodeUnpadded(utf8.encode('$_cursorVersion:$sortKey:$id'));

  @override
  bool operator ==(Object other) =>
      other is Cursor && other.sortKey == sortKey && other.id == id;

  @override
  int get hashCode => Object.hash(sortKey, id);

  @override
  String toString() => 'Cursor($sortKey, $id)';
}

/// A request for one page.
final class PageRequest {
  PageRequest({this.limit = defaultPageLimit, this.after}) {
    if (limit < 1 || limit > maxPageLimit) {
      throw PageLimitOutOfRange(limit, maxPageLimit);
    }
  }

  final int limit;
  final Cursor? after;

  /// Query-string fragment with no leading `?`, so a caller can compose it.
  String toQuery() {
    final cursor = after;
    if (cursor == null) {
      return 'limit=$limit';
    }
    return 'limit=$limit&after=${cursor.encode()}';
  }
}

/// One page of results plus the cursor that continues it.
final class Page<T> {
  const Page(this.items, this.next);

  final List<T> items;
  final Cursor? next;

  bool get isLast => next == null;
  bool get isEmpty => items.isEmpty;
  int get length => items.length;
}

/// Unpadded base64url (RFC 4648 §5).
///
/// Dart's `base64Url` codec pads; the wire format does not, and a padded cursor
/// would not match the Rust or TypeScript encoders.
String base64UrlEncodeUnpadded(List<int> input) {
  final buffer = StringBuffer();
  for (var index = 0; index < input.length; index += 3) {
    final b0 = input[index];
    final b1 = index + 1 < input.length ? input[index + 1] : null;
    final b2 = index + 2 < input.length ? input[index + 2] : null;
    final triple = (b0 << 16) | ((b1 ?? 0) << 8) | (b2 ?? 0);
    buffer.write(_alphabet[(triple >> 18) & 0x3f]);
    buffer.write(_alphabet[(triple >> 12) & 0x3f]);
    if (b1 != null) {
      buffer.write(_alphabet[(triple >> 6) & 0x3f]);
    }
    if (b2 != null) {
      buffer.write(_alphabet[triple & 0x3f]);
    }
  }
  return buffer.toString();
}

/// Rejects padding, whitespace and foreign characters rather than skipping them.
List<int> base64UrlDecodeStrict(String input) {
  if (input.isEmpty) {
    throw const InvalidCursor('empty base64url input');
  }
  final out = <int>[];
  var buffer = 0;
  var bits = 0;
  for (final unit in input.codeUnits) {
    final value = _alphabet.codeUnits.indexOf(unit);
    if (value < 0) {
      throw const InvalidCursor('invalid base64url character');
    }
    buffer = (buffer << 6) | value;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      out.add((buffer >> bits) & 0xff);
    }
  }
  if (bits >= 6 || (buffer & ((1 << bits) - 1)) != 0) {
    throw const InvalidCursor('truncated base64url input');
  }
  return out;
}
